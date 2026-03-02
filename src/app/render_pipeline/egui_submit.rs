//! egui frame rendering for the GPU submit pipeline.
//!
//! `render_egui_frame` runs all egui dialogs and overlay panels for the current
//! frame (phase 3 of the render cycle). It is split from `gpu_submit.rs` to keep
//! that module within the 500-line target while grouping the egui-specific logic
//! in one place.

use super::egui_overlays;
use super::types::PostRenderActions;
use crate::app::window_state::WindowState;
use crate::badge::render_badge;
use crate::progress_bar::{ProgressBarSnapshot, render_progress_bars};

/// Parameters for [`WindowState::render_egui_frame`], bundled to stay within
/// the clippy `too_many_arguments` limit.
pub(super) struct RenderEguiParams<'a> {
    pub(super) actions: &'a mut PostRenderActions,
    pub(super) hovered_mark: &'a Option<crate::scrollback_metadata::ScrollbackMark>,
    pub(super) window_size_for_badge: Option<&'a winit::dpi::PhysicalSize<u32>>,
    pub(super) progress_snapshot: &'a Option<ProgressBarSnapshot>,
    pub(super) visible_lines: usize,
    pub(super) scrollback_len: usize,
    pub(super) any_modal_visible: bool,
}

impl WindowState {
    /// Run all egui dialogs and overlay panels for the current frame (phase 3).
    ///
    /// Takes pre-captured state values and updates `actions` with deferred UI responses.
    /// Returns the egui output (`FullOutput` + `Context`) needed by the wgpu render call,
    /// or `None` if the egui context/window is not yet initialised for this window.
    pub(super) fn render_egui_frame(
        &mut self,
        params: RenderEguiParams<'_>,
    ) -> Option<(egui::FullOutput, egui::Context)> {
        let RenderEguiParams {
            actions,
            hovered_mark,
            window_size_for_badge,
            progress_snapshot,
            visible_lines,
            scrollback_len,
            any_modal_visible,
        } = params;
        let egui_start = std::time::Instant::now();

        // Capture values for FPS overlay before closure
        let show_fps = self.debug.show_fps_overlay;
        let fps_value = self.debug.fps_value;
        let frame_time_ms = if !self.debug.frame_times.is_empty() {
            let avg = self.debug.frame_times.iter().sum::<std::time::Duration>()
                / self.debug.frame_times.len() as u32;
            avg.as_secs_f64() * 1000.0
        } else {
            0.0
        };

        // Capture badge state for closure
        let badge_enabled = self.badge_state.enabled;
        let badge_state = if badge_enabled {
            if self.badge_state.is_dirty() {
                self.badge_state.interpolate();
            }
            Some(self.badge_state.clone())
        } else {
            None
        };

        // Capture session variables for status bar rendering (skip if bar is hidden)
        let status_bar_session_vars = if self.config.status_bar.status_bar_enabled
            && !self
                .status_bar_ui
                .should_hide(&self.config, self.is_fullscreen)
        {
            Some(self.badge_state.variables.read().clone())
        } else {
            None
        };

        // Collect pane bounds for identify overlay (before egui borrow)
        let pane_identify_bounds: Vec<(usize, crate::pane::PaneBounds)> =
            if self.overlay_state.pane_identify_hide_time.is_some() {
                self.tab_manager
                    .active_tab()
                    .and_then(|tab| tab.pane_manager())
                    .map(|pm| {
                        pm.all_panes()
                            .iter()
                            .enumerate()
                            .map(|(i, pane)| (i, pane.bounds))
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

        let result =
            if let Some(window) = self.window.as_ref() {
                if let (Some(egui_ctx), Some(egui_state)) = (&self.egui.ctx, &mut self.egui.state) {
                    let mut raw_input = egui_state.take_egui_input(window);

                    // Inject pending events from menu accelerators (Cmd+V/C/A intercepted by muda)
                    raw_input.events.append(&mut self.egui.pending_events);

                    // When no modal UI overlay is visible, filter out Tab key events to prevent
                    // egui's default focus navigation from stealing Tab/Shift+Tab from the terminal.
                    if !any_modal_visible {
                        raw_input.events.retain(|e| {
                            !matches!(
                                e,
                                egui::Event::Key {
                                    key: egui::Key::Tab,
                                    ..
                                }
                            )
                        });
                    }

                    let egui_output = egui_ctx.run(raw_input, |ctx| {
                    // FPS overlay (top-right corner)
                    egui_overlays::render_fps_overlay(ctx, show_fps, fps_value, frame_time_ms);

                    // Resize overlay (centered)
                    egui_overlays::render_resize_overlay(
                        ctx,
                        self.overlay_state.resize_overlay_visible,
                        self.overlay_state.resize_dimensions,
                    );

                    // Copy mode status bar overlay (bottom-left)
                    {
                        let mode_text = if self.copy_mode.is_searching {
                            "SEARCH"
                        } else {
                            match self.copy_mode.visual_mode {
                                crate::copy_mode::VisualMode::None => "COPY",
                                crate::copy_mode::VisualMode::Char => "VISUAL",
                                crate::copy_mode::VisualMode::Line => "V-LINE",
                                crate::copy_mode::VisualMode::Block => "V-BLOCK",
                            }
                        };
                        let status = self.copy_mode.status_text();
                        egui_overlays::render_copy_mode_status_bar(
                            ctx,
                            self.copy_mode.active,
                            self.config.copy_mode.copy_mode_show_status,
                            self.copy_mode.is_searching,
                            self.copy_mode.visual_mode,
                            mode_text,
                            &status,
                        );
                    }

                    // Toast notification (top-center)
                    egui_overlays::render_toast_overlay(
                        ctx,
                        self.overlay_state.toast_message.as_deref(),
                    );

                    // Scrollbar mark tooltip (near mouse pointer)
                    egui_overlays::render_scrollbar_mark_tooltip(ctx, hovered_mark.as_ref());

                    // Render tab bar if visible (action handled after closure)
                    let tab_bar_right_reserved = if self.overlay_ui.ai_inspector.open {
                        self.overlay_ui.ai_inspector.consumed_width()
                    } else {
                        0.0
                    };
                    actions.tab_action = self.tab_bar_ui.render(
                        ctx,
                        &self.tab_manager,
                        &self.config,
                        &self.overlay_ui.profile_manager,
                        tab_bar_right_reserved,
                    );

                    // Render tmux status bar if connected
                    self.overlay_ui.tmux_status_bar_ui.render(
                        ctx,
                        &self.config,
                        self.tmux_state.tmux_session.as_ref(),
                        self.tmux_state.tmux_session_name.as_deref(),
                    );

                    // Render custom status bar
                    if let Some(ref session_vars) = status_bar_session_vars {
                        let (_bar_height, status_bar_action) = self.status_bar_ui.render(
                            ctx,
                            &self.config,
                            session_vars,
                            self.is_fullscreen,
                        );
                        if status_bar_action
                            == Some(crate::status_bar::StatusBarAction::ShowUpdateDialog)
                        {
                            self.update_state.show_dialog = true;
                        }
                    }

                    // Show help UI
                    self.overlay_ui.help_ui.show(ctx);

                    // Show clipboard history UI and collect action
                    actions.clipboard = self.overlay_ui.clipboard_history_ui.show(ctx);

                    // Show command history UI and collect action
                    actions.command_history = self.overlay_ui.command_history_ui.show(ctx);

                    // Show paste special UI and collect action
                    actions.paste_special = self.overlay_ui.paste_special_ui.show(ctx);

                    // Show search UI and collect action
                    actions.search =
                        self.overlay_ui.search_ui.show(ctx, visible_lines, scrollback_len);

                    // Show AI Inspector panel and collect action
                    actions.inspector = self
                        .overlay_ui
                        .ai_inspector
                        .show(ctx, &self.agent_state.available_agents);

                    // Show tmux session picker UI and collect action
                    let tmux_path = self.config.resolve_tmux_path();
                    actions.session_picker =
                        self.overlay_ui.tmux_session_picker_ui.show(ctx, &tmux_path);

                    // Show shader install dialog if visible
                    actions.shader_install = self.overlay_ui.shader_install_ui.show(ctx);

                    // Show integrations welcome dialog if visible
                    actions.integrations = self.overlay_ui.integrations_ui.show(ctx);

                    // Show close confirmation dialog if visible
                    actions.close_confirm = self.overlay_ui.close_confirmation_ui.show(ctx);

                    // Show quit confirmation dialog if visible
                    actions.quit_confirm = self.overlay_ui.quit_confirmation_ui.show(ctx);

                    // Show remote shell install dialog if visible
                    actions.remote_install = self.overlay_ui.remote_shell_install_ui.show(ctx);

                    // Show SSH Quick Connect dialog if visible
                    actions.ssh_connect = self.overlay_ui.ssh_connect_ui.show(ctx);

                    // Render update dialog overlay
                    if self.update_state.show_dialog {
                        // Poll for update install completion
                        if let Some(ref rx) = self.update_state.install_receiver
                            && let Ok(result) = rx.try_recv()
                        {
                            match result {
                                Ok(update_result) => {
                                    self.update_state.install_status = Some(format!(
                                        "Updated to v{}! Restart par-term to use the new version.",
                                        update_result.new_version
                                    ));
                                    self.update_state.installing = false;
                                    self.status_bar_ui.update_available_version = None;
                                }
                                Err(e) => {
                                    self.update_state.install_status =
                                        Some(format!("Update failed: {}", e));
                                    self.update_state.installing = false;
                                }
                            }
                            self.update_state.install_receiver = None;
                        }

                        if let Some(ref update_result) = self.update_state.last_result {
                            let dialog_action = crate::update_dialog::render(
                                ctx,
                                update_result,
                                env!("CARGO_PKG_VERSION"),
                                self.update_state.installation_type,
                                self.update_state.installing,
                                self.update_state.install_status.as_deref(),
                            );
                            match dialog_action {
                                crate::update_dialog::UpdateDialogAction::Dismiss => {
                                    if !self.update_state.installing {
                                        self.update_state.show_dialog = false;
                                        self.update_state.install_status = None;
                                    }
                                }
                                crate::update_dialog::UpdateDialogAction::SkipVersion(v) => {
                                    self.config.updates.skipped_version = Some(v);
                                    self.update_state.show_dialog = false;
                                    self.status_bar_ui.update_available_version = None;
                                    self.update_state.install_status = None;
                                    actions.save_config = true;
                                }
                                crate::update_dialog::UpdateDialogAction::InstallUpdate(v) => {
                                    if !self.update_state.installing {
                                        self.update_state.installing = true;
                                        self.update_state.install_status =
                                            Some("Downloading update...".to_string());
                                        let (tx, rx) = std::sync::mpsc::channel();
                                        self.update_state.install_receiver = Some(rx);
                                        let version = v.clone();
                                        let current_version = crate::VERSION.to_string();
                                        std::thread::spawn(move || {
                                            let result = crate::self_updater::perform_update(
                                                &version,
                                                &current_version,
                                            );
                                            let _ = tx.send(result);
                                        });
                                    }
                                    // Don't close dialog while installing
                                }
                                crate::update_dialog::UpdateDialogAction::None => {}
                            }
                        } else {
                            self.update_state.show_dialog = false;
                        }
                    }

                    // Render profile drawer (right side panel)
                    actions.profile_drawer = self.overlay_ui.profile_drawer_ui.render(
                        ctx,
                        &self.overlay_ui.profile_manager,
                        &self.config,
                        false, // profile modal is no longer in the terminal window
                    );

                    // Render progress bar overlay
                    if let (Some(snap), Some(size)) = (progress_snapshot, window_size_for_badge) {
                        let tab_count = self.tab_manager.tab_count();
                        let tb_height = self.tab_bar_ui.get_height(tab_count, &self.config);
                        let (top_inset, bottom_inset) = match self.config.tab_bar_position {
                            par_term_config::TabBarPosition::Top => (tb_height, 0.0),
                            par_term_config::TabBarPosition::Bottom => (0.0, tb_height),
                            par_term_config::TabBarPosition::Left => (0.0, 0.0),
                        };
                        render_progress_bars(
                            ctx,
                            snap,
                            &self.config,
                            size.width as f32,
                            size.height as f32,
                            top_inset,
                            bottom_inset,
                        );
                    }

                    // Pane identify overlay (large index numbers centered on each pane)
                    egui_overlays::render_pane_identify_overlay(ctx, &pane_identify_bounds);

                    // Render file transfer progress overlay (bottom-right corner)
                    crate::app::file_transfers::render_file_transfer_overlay(
                        &self.file_transfer_state,
                        ctx,
                    );

                    // Render badge overlay (top-right corner)
                    if let (Some(badge), Some(size)) = (&badge_state, window_size_for_badge) {
                        render_badge(ctx, badge, size.width as f32, size.height as f32);
                    }
                });

                    // Handle egui platform output (clipboard, cursor changes, etc.)
                    egui_state.handle_platform_output(window, egui_output.platform_output.clone());

                    Some((egui_output, egui_ctx.clone()))
                } else {
                    // egui context/state not yet initialised for this window.
                    None
                }
            } else {
                // Window not yet created; skip egui rendering this frame.
                crate::debug_error!("RENDER", "egui render skipped: window is None");
                None
            };

        // Mark egui as initialized after first ctx.run() - makes is_using_pointer() reliable
        if !self.egui.initialized && result.is_some() {
            self.egui.initialized = true;
        }

        let debug_egui_time = egui_start.elapsed();
        self.debug.last_egui_time = debug_egui_time;

        result
    }
}

/// Helper to get the current scroll offset from the active tab.
pub(super) fn scroll_offset_from_tab(tab_manager: &crate::tab::TabManager) -> usize {
    tab_manager
        .active_tab()
        .map(|t| t.active_scroll_state().offset)
        .unwrap_or(0)
}

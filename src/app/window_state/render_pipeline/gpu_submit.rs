//! GPU frame submission for the render pipeline.
//!
//! `submit_gpu_frame` drives the full egui + wgpu render pass for one frame:
//! - Prettifier cell substitution
//! - egui overlay rendering (FPS, toast, tab bar, all dialogs)
//! - Cell / cursor / progress / scrollbar / graphics upload to the GPU
//! - Split-pane or single-pane wgpu render call
//!
//! Returns a `PostRenderActions` bundle that the caller dispatches after the
//! renderer borrow is released (`update_post_render_state` in `post_render.rs`).
//!
//! GPU state upload (phases 1–2) lives in `renderer_ops.rs`.
//! Standalone egui overlay free functions live in `egui_overlays.rs`.

use super::egui_overlays;
use super::pane_render;
use super::prettifier_cells;
use super::renderer_ops::{GpuStateUpdateParams, update_gpu_renderer_state};
use super::types::{FrameRenderData, PostRenderActions};
use crate::app::window_state::WindowState;
use crate::badge::render_badge;
use crate::progress_bar::{ProgressBarSnapshot, render_progress_bars};
use crate::ui_constants::VISUAL_BELL_FLASH_DURATION_MS;
use wgpu::SurfaceError;

/// Parameters for [`WindowState::render_egui_frame`], bundled to stay within
/// the clippy `too_many_arguments` limit.
struct RenderEguiParams<'a> {
    actions: &'a mut PostRenderActions,
    hovered_mark: &'a Option<crate::scrollback_metadata::ScrollbackMark>,
    window_size_for_badge: Option<&'a winit::dpi::PhysicalSize<u32>>,
    progress_snapshot: &'a Option<ProgressBarSnapshot>,
    visible_lines: usize,
    scrollback_len: usize,
    any_modal_visible: bool,
}

impl WindowState {
    /// Run prettifier cell substitution, egui overlays, and GPU render pass.
    /// Returns collected post-render actions to handle after the renderer borrow is released.
    pub(super) fn submit_gpu_frame(&mut self, frame_data: FrameRenderData) -> PostRenderActions {
        let FrameRenderData {
            mut cells,
            cursor_pos: current_cursor_pos,
            cursor_style,
            is_alt_screen,
            scrollback_len,
            show_scrollbar,
            visible_lines,
            grid_cols,
            scrollback_marks,
            total_lines,
            debug_url_detect_time,
        } = frame_data;

        let mut actions = PostRenderActions::default();

        let render_start = std::time::Instant::now();

        #[allow(unused_assignments)]
        let mut debug_actual_render_time = std::time::Duration::ZERO;
        let _ = &debug_actual_render_time;

        // Process agent messages and refresh AI Inspector snapshot
        self.process_agent_messages_tick();

        // Check tmux gateway state before renderer borrow to avoid borrow conflicts.
        // Note: pane_padding is in logical pixels (config); we defer DPI scaling to
        // where it's used with physical pixel coordinates (via sizing.scale_factor).
        let is_tmux_gateway = self.is_gateway_active();
        let effective_pane_padding = if is_tmux_gateway {
            0.0
        } else {
            self.config.pane_padding
        };

        // Calculate status bar heights before mutable renderer borrow.
        // Note: These are in logical pixels; they get scaled to physical in RendererSizing.
        let is_tmux_connected = self.is_tmux_connected();
        let status_bar_height =
            crate::tmux_status_bar_ui::TmuxStatusBarUI::height(&self.config, is_tmux_connected);
        let custom_status_bar_height = self.status_bar_ui.height(&self.config, self.is_fullscreen);

        // Capture window size before mutable borrow (for badge rendering in egui)
        let window_size_for_badge = self.renderer.as_ref().map(|r| r.size());

        // Capture progress bar snapshot before mutable borrow
        let progress_snapshot = if self.config.progress_bar_enabled {
            self.tab_manager.active_tab().and_then(|tab| {
                tab.terminal
                    .try_write()
                    .ok()
                    .map(|term| ProgressBarSnapshot {
                        simple: term.progress_bar(),
                        named: term.named_progress_bars(),
                    })
            })
        } else {
            None
        };

        // Sync AI Inspector panel width before scrollbar update so the scrollbar
        // position uses the current panel width on this frame (not the previous one).
        self.sync_ai_inspector_width();

        // Prettifier cell substitution — replace raw cells with rendered content.
        // Always run when blocks exist: the cell cache stores raw terminal cells
        // (set before this point), so we must re-apply styled content every frame.
        //
        // Also collect inline graphics (rendered diagrams) for GPU compositing.
        let prettifier_graphics = if let Some(tab) = self.tab_manager.active_tab() {
            prettifier_cells::apply_prettifier_cell_substitution(
                tab,
                &mut cells,
                is_alt_screen,
                visible_lines,
                scrollback_len,
                grid_cols,
            )
        } else {
            Vec::new()
        };

        // Cache modal visibility before entering the renderer borrow scope.
        // Method calls borrow all of `self`, which conflicts with `&mut self.renderer`.
        let any_modal_visible = self.any_modal_ui_visible();

        // =====================================================================
        // Phase 1-2: GPU state upload
        //
        // Upload cell data, cursor, scrollbar, animations, and graphics to the GPU.
        // Produces `GpuUploadResult` (timing + sizing + hovered scrollbar mark).
        // Delegated to `renderer_ops::update_gpu_renderer_state`.
        // =====================================================================

        // Compute scroll offset before taking a mutable renderer borrow to avoid
        // simultaneous &mut self.tab_manager and &self.tab_manager in the same call.
        let scroll_offset = scroll_offset_from_tab(&self.tab_manager);

        let gpu_result = if let Some(renderer) = &mut self.renderer {
            Some(update_gpu_renderer_state(
                renderer,
                GpuStateUpdateParams {
                    tab_manager: &mut self.tab_manager,
                    config: &self.config,
                    cursor_anim: &self.cursor_anim,
                    window: &self.window,
                    debug: &self.debug,
                    cells: &cells,
                    current_cursor_pos,
                    cursor_style,
                    progress_snapshot: &progress_snapshot,
                    prettifier_graphics: &prettifier_graphics,
                    scroll_offset,
                    visible_lines,
                    scrollback_len,
                    total_lines,
                    is_alt_screen,
                    scrollback_marks: &scrollback_marks,
                    status_bar_height,
                    custom_status_bar_height,
                },
            ))
        } else {
            None
        };

        // Clear visual bell if its duration has elapsed.
        // This is separate from the GPU upload to avoid borrow conflicts.
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            let flash = tab.active_bell().visual_flash;
            if let Some(flash_start) = flash
                && flash_start.elapsed().as_millis() >= VISUAL_BELL_FLASH_DURATION_MS
            {
                tab.active_bell_mut().visual_flash = None;
            }
        }

        // =====================================================================
        // Phase 3: egui overlay rendering
        //
        // Run egui dialogs, overlays, and UI panels. Returns egui output for the
        // render phase and updates `actions` with deferred UI responses.
        // =====================================================================
        let egui_data = if let Some(ref gpu) = gpu_result {
            self.render_egui_frame(RenderEguiParams {
                actions: &mut actions,
                hovered_mark: &gpu.hovered_mark,
                window_size_for_badge: window_size_for_badge.as_ref(),
                progress_snapshot: &progress_snapshot,
                visible_lines,
                scrollback_len,
                any_modal_visible,
            })
        } else {
            None
        };

        // =====================================================================
        // Phase 4-5: Frame submission and timing
        // =====================================================================
        if let (Some(renderer), Some(gpu)) = (&mut self.renderer, gpu_result) {
            let super::renderer_ops::GpuUploadResult {
                debug_update_cells_time,
                debug_graphics_time,
                debug_anim_time,
                sizing,
                ..
            } = gpu;

            let debug_egui_time = self.debug.last_egui_time;

            // Calculate FPS and timing stats
            let avg_frame_time = if !self.debug.frame_times.is_empty() {
                self.debug.frame_times.iter().sum::<std::time::Duration>()
                    / self.debug.frame_times.len() as u32
            } else {
                std::time::Duration::ZERO
            };
            let fps = if avg_frame_time.as_secs_f64() > 0.0 {
                1.0 / avg_frame_time.as_secs_f64()
            } else {
                0.0
            };

            // Update FPS value for overlay display
            self.debug.fps_value = fps;

            // Log debug info every 60 frames (about once per second at 60 FPS)
            if self.debug.frame_times.len() >= 60 {
                let (cache_gen, cache_has_cells) = self
                    .tab_manager
                    .active_tab()
                    .map(|t| {
                        (
                            t.active_cache().generation,
                            t.active_cache().cells.is_some(),
                        )
                    })
                    .unwrap_or((0, false));
                log::info!(
                    "PERF: FPS={:.1} Frame={:.2}ms CellGen={:.2}ms({}) URLDetect={:.2}ms Anim={:.2}ms Graphics={:.2}ms egui={:.2}ms UpdateCells={:.2}ms ActualRender={:.2}ms Total={:.2}ms Cells={} Gen={} Cache={}",
                    fps,
                    avg_frame_time.as_secs_f64() * 1000.0,
                    self.debug.cell_gen_time.as_secs_f64() * 1000.0,
                    if self.debug.cache_hit { "HIT" } else { "MISS" },
                    debug_url_detect_time.as_secs_f64() * 1000.0,
                    debug_anim_time.as_secs_f64() * 1000.0,
                    debug_graphics_time.as_secs_f64() * 1000.0,
                    debug_egui_time.as_secs_f64() * 1000.0,
                    debug_update_cells_time.as_secs_f64() * 1000.0,
                    debug_actual_render_time.as_secs_f64() * 1000.0,
                    self.debug.render_time.as_secs_f64() * 1000.0,
                    cells.len(),
                    cache_gen,
                    if cache_has_cells { "YES" } else { "NO" }
                );
            }

            // Check if we have a pane manager with panes.
            // We use pane_count() > 0 instead of has_multiple_panes() because even with a
            // single pane in the manager (e.g., after closing one tmux split), we need to
            // render via the pane manager path since cells are in the pane's terminal,
            // not the main renderer buffer.
            let (has_pane_manager, pane_count) = self
                .tab_manager
                .active_tab()
                .and_then(|t| t.pane_manager.as_ref())
                .map(|pm| (pm.pane_count() > 0, pm.pane_count()))
                .unwrap_or((false, 0));

            crate::debug_trace!(
                "RENDER",
                "has_pane_manager={}, pane_count={}",
                has_pane_manager,
                pane_count
            );

            // Per-pane backgrounds only take effect when splits are active.
            let pane_0_bg: Option<crate::pane::PaneBackground> = None;

            // render_egui_frame returns Option<(FullOutput, Context)> with an owned Context
            // (a cheap Arc clone). The downstream render functions expect
            // Option<(FullOutput, &Context)>. We split the tuple so the Context lives in
            // a separate binding that outlives its borrow in the render call.
            let (egui_output, egui_ctx_store) = match egui_data {
                Some((output, ctx)) => (Some(output), Some(ctx)),
                None => (None, None),
            };

            let actual_render_start = std::time::Instant::now();
            let render_result = if has_pane_manager {
                // Gather all per-pane render data.
                let pane_render_data = self.tab_manager.active_tab_mut().and_then(|tab| {
                    pane_render::gather_pane_render_data(
                        tab,
                        &self.config,
                        &sizing,
                        effective_pane_padding,
                        self.cursor_anim.cursor_opacity,
                        pane_count,
                    )
                });

                if let Some((
                    pane_data,
                    dividers,
                    pane_titles,
                    focused_viewport,
                    focused_pane_scrollback_len,
                )) = pane_render_data
                {
                    // Update tab cache with the focused pane's scrollback_len so that scroll
                    // operations see the correct limit.
                    if focused_pane_scrollback_len > 0
                        && let Some(tab) = self.tab_manager.active_tab_mut()
                    {
                        tab.active_cache_mut().scrollback_len = focused_pane_scrollback_len;
                    }

                    // Get hovered divider index for hover color rendering
                    let hovered_divider_index = self
                        .tab_manager
                        .active_tab()
                        .and_then(|t| t.active_mouse().hovered_divider_index);

                    // Recombine output and a reference to the context.
                    let split_egui: Option<(egui::FullOutput, &egui::Context)> =
                        egui_output.zip(egui_ctx_store.as_ref());

                    // Render split panes
                    Self::render_split_panes_with_data(
                        renderer,
                        pane_render::SplitPaneRenderParams {
                            pane_data,
                            dividers,
                            pane_titles,
                            focused_viewport,
                            config: &self.config,
                            egui_data: split_egui,
                            hovered_divider_index,
                            show_scrollbar,
                        },
                    )
                } else {
                    // Fallback to single pane render
                    let single_egui: Option<(egui::FullOutput, &egui::Context)> =
                        egui_output.zip(egui_ctx_store.as_ref());
                    renderer.render(single_egui, false, show_scrollbar, pane_0_bg.as_ref())
                }
            } else {
                // Single pane - use standard render path
                let single_egui: Option<(egui::FullOutput, &egui::Context)> =
                    egui_output.zip(egui_ctx_store.as_ref());
                renderer.render(single_egui, false, show_scrollbar, pane_0_bg.as_ref())
            };

            match render_result {
                Ok(rendered) => {
                    if !rendered {
                        log::trace!("Skipped rendering - no changes");
                    }
                }
                Err(e) => {
                    if let Some(surface_error) = e.downcast_ref::<SurfaceError>() {
                        match surface_error {
                            SurfaceError::Outdated | SurfaceError::Lost => {
                                log::warn!(
                                    "Surface error detected ({:?}), reconfiguring...",
                                    surface_error
                                );
                                self.force_surface_reconfigure();
                            }
                            SurfaceError::Timeout => {
                                log::warn!("Surface timeout, will retry next frame");
                                self.request_redraw();
                            }
                            SurfaceError::OutOfMemory => {
                                log::error!("Surface out of memory: {:?}", surface_error);
                            }
                            _ => {
                                log::error!("Surface error: {:?}", surface_error);
                            }
                        }
                    } else {
                        log::error!("Render error: {}", e);
                    }
                }
            }
            debug_actual_render_time = actual_render_start.elapsed();
            let _ = debug_actual_render_time;

            self.debug.render_time = render_start.elapsed();
        }

        actions
    }

    /// Run all egui dialogs and overlay panels for the current frame (phase 3).
    ///
    /// Takes pre-captured state values and updates `actions` with deferred UI responses.
    /// Returns the egui output (`FullOutput` + `Context`) needed by the wgpu render call,
    /// or `None` if the egui context/window is not yet initialised for this window.
    fn render_egui_frame(
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
fn scroll_offset_from_tab(tab_manager: &crate::tab::TabManager) -> usize {
    tab_manager
        .active_tab()
        .map(|t| t.active_scroll_state().offset)
        .unwrap_or(0)
}

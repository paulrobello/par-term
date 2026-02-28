//! GPU render pipeline for WindowState.
//!
//! Contains the full rendering cycle:
//! - `render`: per-frame orchestration entry point
//! - `frame_setup`: `should_render_frame`, `update_frame_metrics`, `update_animations`, `sync_layout`
//! - `gather_data`: `gather_render_data` — snapshot terminal state into `FrameRenderData`
//! - `submit_gpu_frame`: egui + wgpu render pass, returns `PostRenderActions`
//! - `update_post_render_state`: dispatch post-render action queue
//! - `pane_render`: `gather_pane_render_data` + `render_split_panes_with_data` + `PaneRenderData`
//! - `egui_overlays`: standalone egui overlay renderers (FPS, resize, toast, tooltip, pane-id)
//! - `types`: shared data-transfer types (`RendererSizing`, `FrameRenderData`, `PostRenderActions`)

mod egui_overlays;
mod frame_setup;
mod gather_data;
mod pane_render;
mod post_render;
mod prettifier_cells;
mod types;

use types::{FrameRenderData, PostRenderActions, RendererSizing};

use crate::app::window_state::WindowState;
use crate::badge::render_badge;
use crate::config::{ShaderInstallPrompt, color_u8_to_f32_a};
use crate::progress_bar::{ProgressBarSnapshot, render_progress_bars};
use wgpu::SurfaceError;

impl WindowState {
    /// Main render function for this window
    pub(crate) fn render(&mut self) {
        // Skip rendering if shutting down
        if self.is_shutting_down {
            return;
        }

        if !self.should_render_frame() {
            return;
        }

        self.update_frame_metrics();
        self.update_animations();
        self.sync_layout();

        let Some(frame_data) = self.gather_render_data() else {
            return;
        };

        let actions = self.submit_gpu_frame(frame_data);
        self.update_post_render_state(actions);

        // Process any pending config saves that were deferred by debouncing
        self.process_pending_config_save();
    }

    /// Run prettifier cell substitution, egui overlays, and GPU render pass.
    /// Returns collected post-render actions to handle after the renderer borrow is released.
    fn submit_gpu_frame(&mut self, frame_data: FrameRenderData) -> PostRenderActions {
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

        let mut debug_update_cells_time = std::time::Duration::ZERO;
        #[allow(unused_assignments)]
        let mut debug_graphics_time = std::time::Duration::ZERO;
        #[allow(unused_assignments)]
        let mut debug_actual_render_time = std::time::Duration::ZERO;
        let _ = &debug_actual_render_time;
        // Process agent messages and refresh AI Inspector snapshot
        self.process_agent_messages_tick();

        // Check tmux gateway state before renderer borrow to avoid borrow conflicts
        // When tmux controls the layout, we don't use pane padding
        // Note: pane_padding is in logical pixels (config); we defer DPI scaling to
        // where it's used with physical pixel coordinates (via sizing.scale_factor).
        let is_tmux_gateway = self.is_gateway_active();
        let effective_pane_padding = if is_tmux_gateway {
            0.0
        } else {
            self.config.pane_padding
        };

        // Calculate status bar height before mutable renderer borrow
        // Note: This is in logical pixels; it gets scaled to physical in RendererSizing.
        let is_tmux_connected = self.is_tmux_connected();
        let status_bar_height =
            crate::tmux_status_bar_ui::TmuxStatusBarUI::height(&self.config, is_tmux_connected);

        // Calculate custom status bar height
        let custom_status_bar_height = self.status_bar_ui.height(&self.config, self.is_fullscreen);

        // Capture window size before mutable borrow (for badge rendering in egui)
        let window_size_for_badge = self.renderer.as_ref().map(|r| r.size());

        // Capture progress bar snapshot before mutable borrow
        let progress_snapshot = if self.config.progress_bar_enabled {
            self.tab_manager.active_tab().and_then(|tab| {
                tab.terminal
                    .try_lock()
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
        // Extraction: see prettifier_cells.rs for the full implementation.
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

        if let Some(renderer) = &mut self.renderer {
            // Status bar inset is handled by sync_status_bar_inset() above,
            // before cell gathering, so the grid height is correct.

            // Disable cursor shader when alt screen is active (TUI apps like vim, htop)
            let disable_cursor_shader =
                self.config.cursor_shader_disable_in_alt_screen && is_alt_screen;
            renderer.set_cursor_shader_disabled_for_alt_screen(disable_cursor_shader);

            // Only update renderer with cells if they changed (cache MISS)
            // This avoids re-uploading the same cell data to GPU on every frame
            if !self.debug.cache_hit {
                let t = std::time::Instant::now();
                renderer.update_cells(&cells);
                debug_update_cells_time = t.elapsed();
            }

            // Update cursor position and style for geometric rendering
            if let (Some(pos), Some(opacity), Some(style)) = (
                current_cursor_pos,
                Some(self.cursor_anim.cursor_opacity),
                cursor_style,
            ) {
                renderer.update_cursor(pos, opacity, style);
                // Forward cursor state to custom shader for Ghostty-compatible cursor animations
                // Use the configured cursor color
                let cursor_color = color_u8_to_f32_a(self.config.cursor_color, 1.0);
                renderer.update_shader_cursor(pos.0, pos.1, opacity, cursor_color, style);
            } else {
                renderer.clear_cursor();
            }

            // Update progress bar state for shader uniforms
            if let Some(ref snap) = progress_snapshot {
                use par_term_emu_core_rust::terminal::ProgressState;
                let state_val = match snap.simple.state {
                    ProgressState::Hidden => 0.0,
                    ProgressState::Normal => 1.0,
                    ProgressState::Error => 2.0,
                    ProgressState::Indeterminate => 3.0,
                    ProgressState::Warning => 4.0,
                };
                let active_count = (if snap.simple.is_active() { 1 } else { 0 })
                    + snap.named.values().filter(|b| b.state.is_active()).count();
                renderer.update_shader_progress(
                    state_val,
                    snap.simple.progress as f32 / 100.0,
                    if snap.has_active() { 1.0 } else { 0.0 },
                    active_count as f32,
                );
            } else {
                renderer.update_shader_progress(0.0, 0.0, 0.0, 0.0);
            }

            // Update scrollbar
            let scroll_offset = self
                .tab_manager
                .active_tab()
                .map(|t| t.scroll_state.offset)
                .unwrap_or(0);
            renderer.update_scrollbar(scroll_offset, visible_lines, total_lines, &scrollback_marks);

            // Compute and set command separator marks for single-pane rendering
            if self.config.command_separator_enabled {
                let separator_marks = crate::renderer::compute_visible_separator_marks(
                    &scrollback_marks,
                    scrollback_len,
                    scroll_offset,
                    visible_lines,
                );
                renderer.set_separator_marks(separator_marks);
            } else {
                renderer.set_separator_marks(Vec::new());
            }

            // Compute and set gutter indicators for prettified blocks
            {
                let gutter_data = if let Some(tab) = self.tab_manager.active_tab() {
                    if let Some(ref pipeline) = tab.prettifier {
                        if pipeline.is_enabled() {
                            let indicators = tab.gutter_manager.indicators_for_viewport(
                                pipeline,
                                scroll_offset,
                                visible_lines,
                            );
                            // Default gutter indicator color: semi-transparent highlight
                            let gutter_color = [0.3, 0.5, 0.8, 0.15];
                            indicators
                                .iter()
                                .flat_map(|ind| {
                                    (ind.row..ind.row + ind.height).map(move |r| (r, gutter_color))
                                })
                                .collect::<Vec<_>>()
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                renderer.set_gutter_indicators(gutter_data);
            }

            // Update animations and request redraw if frames changed
            // Use try_lock() to avoid blocking the event loop when PTY reader holds the lock
            let anim_start = std::time::Instant::now();
            if let Some(tab) = self.tab_manager.active_tab()
                && let Ok(terminal) = tab.terminal.try_lock()
                && terminal.update_animations()
            {
                // Animation frame changed - request continuous redraws while animations are playing
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            let debug_anim_time = anim_start.elapsed();

            // Update graphics from terminal (pass scroll_offset for view adjustment)
            // Include both current screen graphics and scrollback graphics
            // Use get_graphics_with_animations() to get current animation frames
            // Use try_lock() to avoid blocking the event loop when PTY reader holds the lock
            //
            // In split-pane mode each pane has its own PTY terminal; graphics are collected
            // per-pane inside the pane data gather loop below and do not go through here.
            let graphics_start = std::time::Instant::now();
            let has_pane_manager_for_graphics = self
                .tab_manager
                .active_tab()
                .and_then(|t| t.pane_manager.as_ref())
                .map(|pm| pm.pane_count() > 0)
                .unwrap_or(false);
            if !has_pane_manager_for_graphics
                && let Some(tab) = self.tab_manager.active_tab()
                && let Ok(terminal) = tab.terminal.try_lock()
            {
                let mut graphics = terminal.get_graphics_with_animations();
                let scrollback_len = terminal.scrollback_len();

                // Always include scrollback graphics (renderer will calculate visibility)
                let scrollback_graphics = terminal.get_scrollback_graphics();
                let scrollback_count = scrollback_graphics.len();
                graphics.extend(scrollback_graphics);

                debug_info!(
                    "APP",
                    "Got {} graphics ({} scrollback) from terminal (scroll_offset={}, scrollback_len={})",
                    graphics.len(),
                    scrollback_count,
                    scroll_offset,
                    scrollback_len
                );
                if let Err(e) = renderer.update_graphics(
                    &graphics,
                    scroll_offset,
                    scrollback_len,
                    visible_lines,
                ) {
                    log::error!("Failed to update graphics: {}", e);
                }
            }
            debug_graphics_time = graphics_start.elapsed();

            // Upload prettifier diagram graphics (rendered Mermaid, etc.) to the GPU.
            // These are appended to the sixel_graphics render list and composited in
            // the same pass as Sixel/iTerm2/Kitty graphics.
            if !prettifier_graphics.is_empty() {
                #[allow(clippy::type_complexity)]
                let refs: Vec<(u64, &[u8], u32, u32, isize, usize)> = prettifier_graphics
                    .iter()
                    .map(|(id, data, w, h, row, col)| (*id, data.as_slice(), *w, *h, *row, *col))
                    .collect();
                if let Err(e) = renderer.update_prettifier_graphics(&refs) {
                    crate::debug_error!(
                        "PRETTIFIER",
                        "Failed to upload prettifier graphics: {}",
                        e
                    );
                }
            }

            // Calculate visual bell flash intensity (0.0 = no flash, 1.0 = full flash)
            let visual_bell_flash = self
                .tab_manager
                .active_tab()
                .and_then(|t| t.bell.visual_flash);
            let visual_bell_intensity = if let Some(flash_start) = visual_bell_flash {
                const FLASH_DURATION_MS: u128 = 150;
                let elapsed = flash_start.elapsed().as_millis();
                if elapsed < FLASH_DURATION_MS {
                    // Request continuous redraws while flash is active
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                    // Fade out: start at 0.3 intensity, fade to 0
                    0.3 * (1.0 - (elapsed as f32 / FLASH_DURATION_MS as f32))
                } else {
                    // Flash complete - clear it
                    if let Some(tab) = self.tab_manager.active_tab_mut() {
                        tab.bell.visual_flash = None;
                    }
                    0.0
                }
            } else {
                0.0
            };

            // Update renderer with visual bell intensity
            renderer.set_visual_bell_intensity(visual_bell_intensity);

            // Prepare egui output if settings UI is visible
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

            // Capture badge state for closure (uses window_size_for_badge captured earlier)
            let badge_enabled = self.badge_state.enabled;
            let badge_state = if badge_enabled {
                // Update variables if dirty
                if self.badge_state.is_dirty() {
                    self.badge_state.interpolate();
                }
                Some(self.badge_state.clone())
            } else {
                None
            };

            // Capture session variables for status bar rendering (skip if bar is hidden)
            let status_bar_session_vars = if self.config.status_bar_enabled
                && !self
                    .status_bar_ui
                    .should_hide(&self.config, self.is_fullscreen)
            {
                Some(self.badge_state.variables.read().clone())
            } else {
                None
            };

            // Capture hovered scrollbar mark for tooltip display
            let hovered_mark: Option<crate::scrollback_metadata::ScrollbackMark> =
                if self.config.scrollbar_mark_tooltips && self.config.scrollbar_command_marks {
                    self.tab_manager
                        .active_tab()
                        .map(|tab| tab.mouse.position)
                        .and_then(|(mx, my)| {
                            renderer.scrollbar_mark_at_position(mx as f32, my as f32, 8.0)
                        })
                        .cloned()
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

            let egui_data = if let Some(window) = self.window.as_ref() {
                // Window is live; run egui if the context and state are also ready.
                if let (Some(egui_ctx), Some(egui_state)) = (&self.egui_ctx, &mut self.egui_state) {
                    let mut raw_input = egui_state.take_egui_input(window);

                    // Inject pending events from menu accelerators (Cmd+V/C/A intercepted by muda)
                    // when egui overlays (profile modal, search, etc.) are active
                    raw_input.events.append(&mut self.pending_egui_events);

                    // When no modal UI overlay is visible, filter out Tab key events to prevent
                    // egui's default focus navigation from stealing Tab/Shift+Tab from the terminal.
                    // Tab/Shift+Tab should only cycle focus between egui widgets when a modal is open.
                    // Note: Side panels (ai_inspector, profile drawer) are NOT modals — the terminal
                    // should still receive Tab/Shift+Tab when they are open.
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
                            self.config.copy_mode_show_status,
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

                    // Settings are now handled by standalone SettingsWindow only
                    // No overlay settings UI rendering needed

                    // Show help UI
                    self.overlay_ui.help_ui.show(ctx);

                    // Show clipboard history UI and collect action
                    actions.clipboard = self.overlay_ui.clipboard_history_ui.show(ctx);

                    // Show command history UI and collect action
                    actions.command_history = self.overlay_ui.command_history_ui.show(ctx);

                    // Show paste special UI and collect action
                    actions.paste_special = self.overlay_ui.paste_special_ui.show(ctx);

                    // Show search UI and collect action
                    actions.search = self.overlay_ui.search_ui.show(ctx, visible_lines, scrollback_len);

                    // Show AI Inspector panel and collect action
                    actions.inspector = self.overlay_ui.ai_inspector.show(ctx, &self.agent_state.available_agents);

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
                                            let result =
                                                crate::self_updater::perform_update(&version, &current_version);
                                            let _ = tx.send(result);
                                        });
                                    }
                                    // Don't close dialog while installing
                                }
                                crate::update_dialog::UpdateDialogAction::None => {}
                            }
                        } else {
                            // No update result, close dialog
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
                    if let (Some(snap), Some(size)) = (&progress_snapshot, window_size_for_badge) {
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
                    // This enables cut/copy/paste in egui text editors
                    egui_state.handle_platform_output(window, egui_output.platform_output.clone());

                    Some((egui_output, egui_ctx))
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
            if !self.egui_initialized && egui_data.is_some() {
                self.egui_initialized = true;
            }

            // Settings are now handled exclusively by standalone SettingsWindow
            // Config changes are applied via window_manager.apply_config_to_windows()

            let debug_egui_time = egui_start.elapsed();

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
                    .map(|t| (t.cache.generation, t.cache.cells.is_some()))
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

            // Render (with dirty tracking optimization)
            let actual_render_start = std::time::Instant::now();
            // Settings are handled by standalone SettingsWindow, not embedded UI

            // Extract renderer sizing info for split pane calculations
            let sizing = RendererSizing {
                size: renderer.size(),
                content_offset_y: renderer.content_offset_y(),
                content_offset_x: renderer.content_offset_x(),
                content_inset_bottom: renderer.content_inset_bottom(),
                content_inset_right: renderer.content_inset_right(),
                cell_width: renderer.cell_width(),
                cell_height: renderer.cell_height(),
                padding: renderer.window_padding(),
                status_bar_height: (status_bar_height + custom_status_bar_height)
                    * renderer.scale_factor(),
                scale_factor: renderer.scale_factor(),
            };

            // Check if we have a pane manager with panes - this just checks without modifying
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
            // In single-pane mode, skip per-pane background lookup.
            let pane_0_bg: Option<crate::pane::PaneBackground> = None;

            let render_result = if has_pane_manager {
                // Gather all per-pane render data.
                // `gather_pane_render_data` is a free function: it takes `&RendererSizing`
                // (not `&mut self`) so it can be called while `self.renderer` is mutably
                // borrowed.  We access `self.tab_manager` and `self.cursor_anim` directly
                // as individual field borrows, which the borrow checker allows even when
                // another field (`self.renderer`) is already borrowed.
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
                    // operations (mouse wheel, Page Up/Down, etc.) see the correct limit.
                    // Only update when non-zero to avoid clobbering a good value on lock failure.
                    // The `apply_scroll` function already clamps the target; we don't call
                    // `clamp_to_scrollback` here because that would reset an ongoing scroll.
                    if focused_pane_scrollback_len > 0
                        && let Some(tab) = self.tab_manager.active_tab_mut()
                    {
                        tab.cache.scrollback_len = focused_pane_scrollback_len;
                    }

                    // Get hovered divider index for hover color rendering
                    let hovered_divider_index = self
                        .tab_manager
                        .active_tab()
                        .and_then(|t| t.mouse.hovered_divider_index);

                    // Render split panes
                    Self::render_split_panes_with_data(
                        renderer,
                        pane_data,
                        dividers,
                        pane_titles,
                        focused_viewport,
                        &self.config,
                        egui_data,
                        hovered_divider_index,
                        show_scrollbar,
                    )
                } else {
                    // Fallback to single pane render
                    renderer.render(egui_data, false, show_scrollbar, pane_0_bg.as_ref())
                }
            } else {
                // Single pane - use standard render path
                renderer.render(egui_data, false, show_scrollbar, pane_0_bg.as_ref())
            };

            match render_result {
                Ok(rendered) => {
                    if !rendered {
                        log::trace!("Skipped rendering - no changes");
                    }
                }
                Err(e) => {
                    // Check if this is a wgpu surface error that requires reconfiguration
                    // This commonly happens when dragging windows between displays
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
                                if let Some(window) = &self.window {
                                    window.request_redraw();
                                }
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
}

//! GPU render pipeline for WindowState.
//!
//! Contains the full rendering cycle:
//! - `render`: per-frame orchestration entry point
//! - `frame_setup`: `should_render_frame`, `update_frame_metrics`, `update_animations`, `sync_layout`
//! - `gather_render_data`: snapshot terminal state into FrameRenderData
//! - `submit_gpu_frame`: egui + wgpu render pass, returns PostRenderActions
//! - `update_post_render_state`: dispatch post-render action queue
//! - `pane_render`: `render_split_panes_with_data` + `PaneRenderData`

mod frame_setup;
mod pane_render;
mod post_render;

use crate::ai_inspector::panel::InspectorAction;
use crate::app::window_state::WindowState;
use crate::badge::render_badge;
use crate::cell_renderer::PaneViewport;
use crate::clipboard_history_ui::ClipboardHistoryAction;
use crate::close_confirmation_ui::CloseConfirmAction;
use crate::command_history_ui::CommandHistoryAction;
use crate::config::{CursorStyle, ShaderInstallPrompt, color_u8_to_f32, color_u8_to_f32_a};
use crate::integrations_ui::IntegrationsResponse;
use crate::paste_special_ui::PasteSpecialAction;
use crate::profile_drawer_ui::ProfileDrawerAction;
use crate::progress_bar::{ProgressBarSnapshot, render_progress_bars};
use crate::quit_confirmation_ui::QuitConfirmAction;
use crate::remote_shell_install_ui::RemoteShellInstallAction;
use crate::renderer::PaneTitleInfo;
use crate::selection::SelectionMode;
use crate::shader_install_ui::ShaderInstallResponse;
use crate::ssh_connect_ui::SshConnectAction;
use crate::tab_bar_ui::TabBarAction;
use crate::tmux_session_picker_ui::SessionPickerAction;
use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;
use std::sync::Arc;
use wgpu::SurfaceError;
use winit::dpi::PhysicalSize;

struct RendererSizing {
    size: PhysicalSize<u32>,
    content_offset_y: f32,
    content_offset_x: f32,
    content_inset_bottom: f32,
    content_inset_right: f32,
    cell_width: f32,
    cell_height: f32,
    padding: f32,
    status_bar_height: f32,
    scale_factor: f32,
}

/// Data computed during `gather_render_data()` and consumed by the rest of `render()`.
struct FrameRenderData {
    /// Processed terminal cells (URL underlines + search highlights applied)
    cells: Vec<crate::cell_renderer::Cell>,
    /// Cursor position on screen (col, row), None if hidden
    cursor_pos: Option<(usize, usize)>,
    /// Cursor glyph style (from terminal or config overrides)
    cursor_style: Option<par_term_emu_core_rust::cursor::CursorStyle>,
    /// Whether alternate screen is active (vim, htop, etc.)
    is_alt_screen: bool,
    /// Total scrollback lines available
    scrollback_len: usize,
    /// Whether the scrollbar should be shown
    show_scrollbar: bool,
    /// Visible grid rows count
    visible_lines: usize,
    /// Visible grid columns count
    grid_cols: usize,
    /// Scrollback marks (command marks, trigger marks) for scrollbar and separators
    scrollback_marks: Vec<crate::scrollback_metadata::ScrollbackMark>,
    /// Total renderable lines (visible + scrollback)
    total_lines: usize,
    /// Time spent on URL detection this frame (Zero on cache hit)
    debug_url_detect_time: std::time::Duration,
}

/// Actions collected during the egui/GPU render pass to be handled after the renderer borrow ends.
pub(super) struct PostRenderActions {
    clipboard: ClipboardHistoryAction,
    command_history: CommandHistoryAction,
    paste_special: PasteSpecialAction,
    session_picker: SessionPickerAction,
    tab_action: TabBarAction,
    shader_install: ShaderInstallResponse,
    integrations: IntegrationsResponse,
    search: crate::search::SearchAction,
    inspector: InspectorAction,
    profile_drawer: ProfileDrawerAction,
    close_confirm: CloseConfirmAction,
    quit_confirm: QuitConfirmAction,
    remote_install: RemoteShellInstallAction,
    ssh_connect: SshConnectAction,
}

impl Default for PostRenderActions {
    fn default() -> Self {
        Self {
            clipboard: ClipboardHistoryAction::None,
            command_history: CommandHistoryAction::None,
            paste_special: PasteSpecialAction::None,
            session_picker: SessionPickerAction::None,
            tab_action: TabBarAction::None,
            shader_install: ShaderInstallResponse::None,
            integrations: IntegrationsResponse::default(),
            search: crate::search::SearchAction::None,
            inspector: InspectorAction::None,
            profile_drawer: ProfileDrawerAction::None,
            close_confirm: CloseConfirmAction::None,
            quit_confirm: QuitConfirmAction::None,
            remote_install: RemoteShellInstallAction::None,
            ssh_connect: SshConnectAction::None,
        }
    }
}

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
        #[allow(clippy::type_complexity)]
        let mut prettifier_graphics: Vec<(
            u64,
            std::sync::Arc<Vec<u8>>,
            u32,
            u32,
            isize,
            usize,
        )> = Vec::new();
        if !is_alt_screen
            && let Some(tab) = self.tab_manager.active_tab()
            && let Some(ref pipeline) = tab.prettifier
            && pipeline.is_enabled()
        {
            let scroll_off = tab.scroll_state.offset;
            let gutter_w = tab.gutter_manager.gutter_width;

            // Track which blocks we've already collected graphics from
            // to avoid duplicates when multiple viewport rows fall in
            // the same block.
            let mut collected_block_ids = std::collections::HashSet::new();

            for viewport_row in 0..visible_lines {
                let absolute_row = scrollback_len.saturating_sub(scroll_off) + viewport_row;
                if let Some(block) = pipeline.block_at_row(absolute_row) {
                    if !block.has_rendered() {
                        continue;
                    }

                    // Collect inline graphics from this block (once per block).
                    if collected_block_ids.insert(block.block_id) {
                        let block_start = block.content().start_row;
                        for graphic in block.buffer.rendered_graphics() {
                            if !graphic.is_rgba
                                || graphic.data.is_empty()
                                || graphic.pixel_width == 0
                                || graphic.pixel_height == 0
                            {
                                continue;
                            }
                            // Compute screen row: block_start + graphic.row within block,
                            // then convert to viewport coordinates.
                            let abs_graphic_row = block_start + graphic.row;
                            let view_start = scrollback_len.saturating_sub(scroll_off);
                            let screen_row = abs_graphic_row as isize - view_start as isize;

                            // Use block_id + graphic row as a stable texture ID
                            // (offset to avoid colliding with terminal graphic IDs).
                            let texture_id = 0x8000_0000_0000_0000_u64
                                | (block.block_id << 16)
                                | (graphic.row as u64);

                            crate::debug_info!(
                                "PRETTIFIER",
                                "uploading graphic: block={}, row={}, screen_row={}, {}x{} px, {} bytes RGBA",
                                block.block_id,
                                graphic.row,
                                screen_row,
                                graphic.pixel_width,
                                graphic.pixel_height,
                                graphic.data.len()
                            );

                            prettifier_graphics.push((
                                texture_id,
                                graphic.data.clone(),
                                graphic.pixel_width,
                                graphic.pixel_height,
                                screen_row,
                                graphic.col + gutter_w,
                            ));
                        }
                    }

                    let display_lines = block.buffer.display_lines();
                    let block_start = block.content().start_row;
                    let source_offset = absolute_row.saturating_sub(block_start);
                    // Use the source→rendered line mapping when available so
                    // that consumed source lines (e.g., code-fence closes) don't
                    // cause index drift.  Fall back to direct indexing when no
                    // mapping exists (unrendered blocks use source lines 1:1).
                    let rendered_idx = block
                        .buffer
                        .rendered_line_for_source(source_offset)
                        .unwrap_or(source_offset);
                    if let Some(styled_line) = display_lines.get(rendered_idx) {
                        crate::debug_trace!(
                            "PRETTIFIER",
                            "cell sub: vp_row={}, abs_row={}, block_id={}, src_off={}, rnd_idx={}, segs={}",
                            viewport_row,
                            absolute_row,
                            block.block_id,
                            source_offset,
                            rendered_idx,
                            styled_line.segments.len()
                        );
                        let cell_start = viewport_row * grid_cols;
                        let cell_end = (cell_start + grid_cols).min(cells.len());
                        if cell_start >= cells.len() {
                            break;
                        }
                        // Clear row
                        for cell in &mut cells[cell_start..cell_end] {
                            *cell = par_term_config::Cell::default();
                        }
                        // Write styled segments (offset by gutter width to avoid clipping)
                        let mut col = gutter_w;
                        for segment in &styled_line.segments {
                            for ch in segment.text.chars() {
                                if col >= grid_cols {
                                    break;
                                }
                                let idx = cell_start + col;
                                if idx < cells.len() {
                                    cells[idx].grapheme = ch.to_string();
                                    if let Some([r, g, b]) = segment.fg {
                                        cells[idx].fg_color = [r, g, b, 0xFF];
                                    }
                                    if let Some([r, g, b]) = segment.bg {
                                        cells[idx].bg_color = [r, g, b, 0xFF];
                                    }
                                    cells[idx].bold = segment.bold;
                                    cells[idx].italic = segment.italic;
                                    cells[idx].underline = segment.underline;
                                    cells[idx].strikethrough = segment.strikethrough;
                                }
                                col += 1;
                            }
                        }
                    }
                }
            }
        }

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
                if self.pane_identify_hide_time.is_some() {
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
                    // Show FPS overlay if enabled (top-right corner)
                    if show_fps {
                        egui::Area::new(egui::Id::new("fps_overlay"))
                            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-30.0, 10.0))
                            .order(egui::Order::Foreground)
                            .show(ctx, |ui| {
                                egui::Frame::NONE
                                    .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200))
                                    .inner_margin(egui::Margin::same(8))
                                    .corner_radius(4.0)
                                    .show(ui, |ui| {
                                        ui.style_mut().visuals.override_text_color =
                                            Some(egui::Color32::from_rgb(0, 255, 0));
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "FPS: {:.1}\nFrame: {:.2}ms",
                                                fps_value, frame_time_ms
                                            ))
                                            .monospace()
                                            .size(14.0),
                                        );
                                    });
                            });
                    }

                    // Show resize overlay if active (centered)
                    if self.resize_overlay_visible
                        && let Some((width_px, height_px, cols, rows)) = self.resize_dimensions
                    {
                        egui::Area::new(egui::Id::new("resize_overlay"))
                            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                            .order(egui::Order::Foreground)
                            .show(ctx, |ui| {
                                egui::Frame::NONE
                                    .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 220))
                                    .inner_margin(egui::Margin::same(16))
                                    .corner_radius(8.0)
                                    .show(ui, |ui| {
                                        ui.style_mut().visuals.override_text_color =
                                            Some(egui::Color32::from_rgb(255, 255, 255));
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{}×{}\n{}×{} px",
                                                cols, rows, width_px, height_px
                                            ))
                                            .monospace()
                                            .size(24.0),
                                        );
                                    });
                            });
                    }

                    // Show copy mode status bar overlay (when enabled in config)
                    if self.copy_mode.active && self.config.copy_mode_show_status {
                        let status = self.copy_mode.status_text();
                        let (mode_text, color) = if self.copy_mode.is_searching {
                            ("SEARCH", egui::Color32::from_rgb(255, 165, 0))
                        } else {
                            match self.copy_mode.visual_mode {
                                crate::copy_mode::VisualMode::None => {
                                    ("COPY", egui::Color32::from_rgb(100, 200, 100))
                                }
                                crate::copy_mode::VisualMode::Char => {
                                    ("VISUAL", egui::Color32::from_rgb(100, 150, 255))
                                }
                                crate::copy_mode::VisualMode::Line => {
                                    ("V-LINE", egui::Color32::from_rgb(100, 150, 255))
                                }
                                crate::copy_mode::VisualMode::Block => {
                                    ("V-BLOCK", egui::Color32::from_rgb(100, 150, 255))
                                }
                            }
                        };

                        egui::Area::new(egui::Id::new("copy_mode_status_bar"))
                            .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(0.0, 0.0))
                            .order(egui::Order::Foreground)
                            .show(ctx, |ui| {
                                let available_width = ui.available_width();
                                egui::Frame::NONE
                                    .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, 230))
                                    .inner_margin(egui::Margin::symmetric(12, 6))
                                    .show(ui, |ui| {
                                        ui.set_min_width(available_width);
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                egui::RichText::new(mode_text)
                                                    .monospace()
                                                    .size(13.0)
                                                    .color(color)
                                                    .strong(),
                                            );
                                            ui.separator();
                                            ui.label(
                                                egui::RichText::new(&status)
                                                    .monospace()
                                                    .size(12.0)
                                                    .color(egui::Color32::from_rgb(200, 200, 200)),
                                            );
                                        });
                                    });
                            });
                    }

                    // Show toast notification if active (top center)
                    if let Some(ref message) = self.toast_message {
                        egui::Area::new(egui::Id::new("toast_notification"))
                            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 60.0))
                            .order(egui::Order::Foreground)
                            .show(ctx, |ui| {
                                egui::Frame::NONE
                                    .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 240))
                                    .inner_margin(egui::Margin::symmetric(20, 12))
                                    .corner_radius(8.0)
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        egui::Color32::from_rgb(80, 80, 80),
                                    ))
                                    .show(ui, |ui| {
                                        ui.style_mut().visuals.override_text_color =
                                            Some(egui::Color32::from_rgb(255, 255, 255));
                                        ui.label(egui::RichText::new(message).size(16.0));
                                    });
                            });
                    }

                    // Show scrollbar mark tooltip if hovering over a mark
                    if let Some(ref mark) = hovered_mark {
                        // Format the tooltip content
                        let mut lines = Vec::new();

                        if let Some(ref cmd) = mark.command {
                            let truncated = if cmd.len() > 50 {
                                format!("{}...", &cmd[..47])
                            } else {
                                cmd.clone()
                            };
                            lines.push(format!("Command: {}", truncated));
                        }

                        if let Some(start_time) = mark.start_time {
                            use chrono::{DateTime, Local, Utc};
                            let dt =
                                DateTime::<Utc>::from_timestamp_millis(start_time as i64).expect("window_state: start_time millis out of valid timestamp range");
                            let local: DateTime<Local> = dt.into();
                            lines.push(format!("Time: {}", local.format("%H:%M:%S")));
                        }

                        if let Some(duration_ms) = mark.duration_ms {
                            if duration_ms < 1000 {
                                lines.push(format!("Duration: {}ms", duration_ms));
                            } else if duration_ms < 60000 {
                                lines
                                    .push(format!("Duration: {:.1}s", duration_ms as f64 / 1000.0));
                            } else {
                                let mins = duration_ms / 60000;
                                let secs = (duration_ms % 60000) / 1000;
                                lines.push(format!("Duration: {}m {}s", mins, secs));
                            }
                        }

                        if let Some(exit_code) = mark.exit_code {
                            lines.push(format!("Exit: {}", exit_code));
                        }

                        let tooltip_text = lines.join("\n");

                        // Calculate tooltip position, clamped to stay on screen
                        let mouse_pos = ctx.pointer_hover_pos().unwrap_or(egui::pos2(100.0, 100.0));
                        let tooltip_x = (mouse_pos.x - 180.0).max(10.0);
                        let tooltip_y = (mouse_pos.y - 20.0).max(10.0);

                        // Show tooltip near mouse position (offset to the left of scrollbar)
                        egui::Area::new(egui::Id::new("scrollbar_mark_tooltip"))
                            .order(egui::Order::Tooltip)
                            .fixed_pos(egui::pos2(tooltip_x, tooltip_y))
                            .show(ctx, |ui| {
                                ui.set_min_width(150.0);
                                egui::Frame::NONE
                                    .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 240))
                                    .inner_margin(egui::Margin::same(8))
                                    .corner_radius(4.0)
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        egui::Color32::from_rgb(80, 80, 80),
                                    ))
                                    .show(ui, |ui| {
                                        ui.set_min_width(140.0);
                                        ui.style_mut().visuals.override_text_color =
                                            Some(egui::Color32::from_rgb(220, 220, 220));
                                        ui.label(
                                            egui::RichText::new(&tooltip_text)
                                                .monospace()
                                                .size(12.0),
                                        );
                                    });
                            });
                    }

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
                            self.show_update_dialog = true;
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
                    if self.show_update_dialog {
                        // Poll for update install completion
                        if let Some(ref rx) = self.update_install_receiver
                            && let Ok(result) = rx.try_recv()
                        {
                            match result {
                                Ok(update_result) => {
                                    self.update_install_status = Some(format!(
                                        "Updated to v{}! Restart par-term to use the new version.",
                                        update_result.new_version
                                    ));
                                    self.update_installing = false;
                                    self.status_bar_ui.update_available_version = None;
                                }
                                Err(e) => {
                                    self.update_install_status =
                                        Some(format!("Update failed: {}", e));
                                    self.update_installing = false;
                                }
                            }
                            self.update_install_receiver = None;
                        }

                        if let Some(ref update_result) = self.last_update_result {
                            let dialog_action = crate::update_dialog::render(
                                ctx,
                                update_result,
                                env!("CARGO_PKG_VERSION"),
                                self.installation_type,
                                self.update_installing,
                                self.update_install_status.as_deref(),
                            );
                            match dialog_action {
                                crate::update_dialog::UpdateDialogAction::Dismiss => {
                                    if !self.update_installing {
                                        self.show_update_dialog = false;
                                        self.update_install_status = None;
                                    }
                                }
                                crate::update_dialog::UpdateDialogAction::SkipVersion(v) => {
                                    self.config.skipped_version = Some(v);
                                    self.show_update_dialog = false;
                                    self.status_bar_ui.update_available_version = None;
                                    self.update_install_status = None;
                                    let _ = self.config.save();
                                }
                                crate::update_dialog::UpdateDialogAction::InstallUpdate(v) => {
                                    if !self.update_installing {
                                        self.update_installing = true;
                                        self.update_install_status =
                                            Some("Downloading update...".to_string());
                                        let (tx, rx) = std::sync::mpsc::channel();
                                        self.update_install_receiver = Some(rx);
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
                            self.show_update_dialog = false;
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

                    // Render pane identify overlay (large index numbers centered on each pane)
                    if !pane_identify_bounds.is_empty() {
                        for (index, bounds) in &pane_identify_bounds {
                            let center_x = bounds.x + bounds.width / 2.0;
                            let center_y = bounds.y + bounds.height / 2.0;
                            egui::Area::new(egui::Id::new(format!("pane_identify_{}", index)))
                                .fixed_pos(egui::pos2(center_x - 30.0, center_y - 30.0))
                                .order(egui::Order::Foreground)
                                .interactable(false)
                                .show(ctx, |ui| {
                                    egui::Frame::NONE
                                        .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200))
                                        .inner_margin(egui::Margin::symmetric(16, 8))
                                        .corner_radius(8.0)
                                        .stroke(egui::Stroke::new(
                                            2.0,
                                            egui::Color32::from_rgb(100, 200, 255),
                                        ))
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(format!("Pane {}", index))
                                                    .monospace()
                                                    .size(28.0)
                                                    .color(egui::Color32::from_rgb(100, 200, 255)),
                                            );
                                        });
                                });
                        }
                    }

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
                // When splits are active and hide_window_padding_on_split is enabled,
                // use 0 padding so panes extend to the window edges
                let effective_padding =
                    if pane_count > 1 && self.config.hide_window_padding_on_split {
                        0.0
                    } else {
                        sizing.padding
                    };

                // Render panes from pane manager - inline data gathering to avoid borrow conflicts
                let content_width = sizing.size.width as f32
                    - effective_padding * 2.0
                    - sizing.content_offset_x
                    - sizing.content_inset_right;
                let content_height = sizing.size.height as f32
                    - sizing.content_offset_y
                    - sizing.content_inset_bottom
                    - effective_padding
                    - sizing.status_bar_height;

                // Gather all necessary data upfront while we can borrow tab_manager
                #[allow(clippy::type_complexity)]
                let pane_render_data: Option<(
                    Vec<pane_render::PaneRenderData>,
                    Vec<crate::pane::DividerRect>,
                    Vec<PaneTitleInfo>,
                    Option<PaneViewport>,
                    usize, // focused pane scrollback_len (for tab.cache update)
                )> = {
                    let tab = self.tab_manager.active_tab_mut();
                    if let Some(tab) = tab {
                        // Capture tab-level scroll offset before mutably borrowing pane_manager.
                        // In split-pane mode the focused pane uses tab.scroll_state.offset;
                        // non-focused panes always render at offset 0 (bottom).
                        let tab_scroll_offset = tab.scroll_state.offset;
                        if let Some(pm) = &mut tab.pane_manager {
                            // Update bounds
                            let bounds = crate::pane::PaneBounds::new(
                                effective_padding + sizing.content_offset_x,
                                sizing.content_offset_y,
                                content_width,
                                content_height,
                            );
                            pm.set_bounds(bounds);

                            // Calculate title bar height offset for terminal sizing
                            // Scale from logical pixels (config) to physical pixels
                            let title_height_offset = if self.config.show_pane_titles {
                                self.config.pane_title_height * sizing.scale_factor
                            } else {
                                0.0
                            };

                            // Resize all pane terminals to match their new bounds
                            // Scale pane_padding from logical to physical pixels
                            pm.resize_all_terminals_with_padding(
                                sizing.cell_width,
                                sizing.cell_height,
                                effective_pane_padding * sizing.scale_factor,
                                title_height_offset,
                            );

                            // Gather pane info
                            let focused_pane_id = pm.focused_pane_id();
                            let all_pane_ids: Vec<_> =
                                pm.all_panes().iter().map(|p| p.id).collect();
                            let dividers = pm.get_dividers();

                            let pane_bg_opacity = self.config.pane_background_opacity;
                            let inactive_opacity = if self.config.dim_inactive_panes {
                                self.config.inactive_pane_opacity
                            } else {
                                1.0
                            };
                            let cursor_opacity = self.cursor_anim.cursor_opacity;

                            // Pane title settings
                            // Scale from logical pixels (config) to physical pixels
                            let show_titles = self.config.show_pane_titles;
                            let title_height = self.config.pane_title_height * sizing.scale_factor;
                            let title_position = self.config.pane_title_position;
                            let title_text_color = color_u8_to_f32(self.config.pane_title_color);
                            let title_bg_color = color_u8_to_f32(self.config.pane_title_bg_color);

                            let mut pane_data = Vec::new();
                            let mut pane_titles = Vec::new();
                            let mut focused_pane_scrollback_len: usize = 0;
                            let mut focused_viewport: Option<PaneViewport> = None;

                            for pane_id in &all_pane_ids {
                                if let Some(pane) = pm.get_pane(*pane_id) {
                                    let is_focused = Some(*pane_id) == focused_pane_id;
                                    let bounds = pane.bounds;

                                    // Calculate viewport, adjusting for title bar if shown
                                    let (viewport_y, viewport_height) = if show_titles {
                                        use crate::config::PaneTitlePosition;
                                        match title_position {
                                            PaneTitlePosition::Top => (
                                                bounds.y + title_height,
                                                (bounds.height - title_height).max(0.0),
                                            ),
                                            PaneTitlePosition::Bottom => {
                                                (bounds.y, (bounds.height - title_height).max(0.0))
                                            }
                                        }
                                    } else {
                                        (bounds.y, bounds.height)
                                    };

                                    // Create viewport with padding for content inset
                                    // Scale pane_padding from logical to physical pixels
                                    let physical_pane_padding =
                                        effective_pane_padding * sizing.scale_factor;
                                    let viewport = PaneViewport::with_padding(
                                        bounds.x,
                                        viewport_y,
                                        bounds.width,
                                        viewport_height,
                                        is_focused,
                                        if is_focused {
                                            pane_bg_opacity
                                        } else {
                                            pane_bg_opacity * inactive_opacity
                                        },
                                        physical_pane_padding,
                                    );

                                    if is_focused {
                                        focused_viewport = Some(viewport);
                                    }

                                    // Build pane title info
                                    if show_titles {
                                        use crate::config::PaneTitlePosition;
                                        let title_y = match title_position {
                                            PaneTitlePosition::Top => bounds.y,
                                            PaneTitlePosition::Bottom => {
                                                bounds.y + bounds.height - title_height
                                            }
                                        };
                                        pane_titles.push(PaneTitleInfo {
                                            x: bounds.x,
                                            y: title_y,
                                            width: bounds.width,
                                            height: title_height,
                                            title: pane.get_title(),
                                            focused: is_focused,
                                            text_color: title_text_color,
                                            bg_color: title_bg_color,
                                        });
                                    }

                                    let cells = if let Ok(term) = pane.terminal.try_lock() {
                                        let scroll_offset =
                                            if is_focused { tab_scroll_offset } else { 0 };
                                        let selection =
                                            pane.mouse.selection.map(|sel| sel.normalized());
                                        let rectangular = pane
                                            .mouse
                                            .selection
                                            .map(|sel| sel.mode == SelectionMode::Rectangular)
                                            .unwrap_or(false);
                                        term.get_cells_with_scrollback(
                                            scroll_offset,
                                            selection,
                                            rectangular,
                                            None,
                                        )
                                    } else {
                                        Vec::new()
                                    };

                                    let need_marks = self.config.scrollbar_command_marks
                                        || self.config.command_separator_enabled;
                                    let (marks, pane_scrollback_len) = if need_marks {
                                        if let Ok(mut term) = pane.terminal.try_lock() {
                                            // Use cursor row 0 when unknown in split panes
                                            let sb_len = term.scrollback_len();
                                            term.update_scrollback_metadata(sb_len, 0);
                                            (term.scrollback_marks(), sb_len)
                                        } else {
                                            (Vec::new(), 0)
                                        }
                                    } else {
                                        // Still need the actual scrollback_len even without marks:
                                        // it's used for graphics position math (tex_v_start
                                        // cropping when graphic is partially off-top, and
                                        // view_start when showing scrollback graphics).
                                        let sb_len = if let Ok(term) = pane.terminal.try_lock() {
                                            term.scrollback_len()
                                        } else {
                                            0
                                        };
                                        (Vec::new(), sb_len)
                                    };
                                    let pane_scroll_offset =
                                        if is_focused { tab_scroll_offset } else { 0 };

                                    // Cache the focused pane's scrollback_len so that scroll
                                    // operations (mouse wheel, Page Up, etc.) can use it without
                                    // needing to lock the terminal. Only update when the value is
                                    // non-zero (lock succeeded) to avoid clobbering a good cached
                                    // value with a transient lock-failure fallback of 0.
                                    if is_focused && pane_scrollback_len > 0 {
                                        focused_pane_scrollback_len = pane_scrollback_len;
                                    }

                                    // Per-pane backgrounds only apply when multiple panes exist
                                    let pane_background = if all_pane_ids.len() > 1
                                        && pane.background().has_image()
                                    {
                                        Some(pane.background().clone())
                                    } else {
                                        None
                                    };

                                    let cursor_pos = if let Ok(term) = pane.terminal.try_lock() {
                                        if term.is_cursor_visible() {
                                            Some(term.cursor_position())
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    };

                                    // Grid size must match the terminal's actual size
                                    // (accounting for padding and title bar, same as resize_all_terminals_with_padding)
                                    let content_width = (bounds.width
                                        - physical_pane_padding * 2.0)
                                        .max(sizing.cell_width);
                                    let content_height = (viewport_height
                                        - physical_pane_padding * 2.0)
                                        .max(sizing.cell_height);
                                    let cols = (content_width / sizing.cell_width).floor() as usize;
                                    let rows =
                                        (content_height / sizing.cell_height).floor() as usize;
                                    let cols = cols.max(1);
                                    let rows = rows.max(1);

                                    // Collect inline graphics (Sixel/iTerm2/Kitty) from this
                                    // pane's PTY terminal.  Each pane has its own PTY so graphics
                                    // are never in the shared tab.terminal.
                                    let pane_graphics = if let Ok(term) = pane.terminal.try_lock() {
                                        let mut g = term.get_graphics_with_animations();
                                        let sb = term.get_scrollback_graphics();
                                        crate::debug_log!(
                                            "GRAPHICS",
                                            "pane {:?}: active_graphics={}, scrollback_graphics={}, scrollback_len={}, scroll_offset={}, visible_rows={}, viewport=({},{},{}x{})",
                                            pane_id,
                                            g.len(),
                                            sb.len(),
                                            pane_scrollback_len,
                                            pane_scroll_offset,
                                            rows,
                                            viewport.x,
                                            viewport.y,
                                            viewport.width,
                                            viewport.height
                                        );
                                        for (i, gfx) in g.iter().chain(sb.iter()).enumerate() {
                                            crate::debug_log!(
                                                "GRAPHICS",
                                                "  graphic[{}]: id={}, pos=({},{}), scroll_offset_rows={}, scrollback_row={:?}, size={}x{}",
                                                i,
                                                gfx.id,
                                                gfx.position.0,
                                                gfx.position.1,
                                                gfx.scroll_offset_rows,
                                                gfx.scrollback_row,
                                                gfx.width,
                                                gfx.height
                                            );
                                        }
                                        g.extend(sb);
                                        g
                                    } else {
                                        crate::debug_log!(
                                            "GRAPHICS",
                                            "pane {:?}: try_lock() failed, no graphics",
                                            pane_id
                                        );
                                        Vec::new()
                                    };

                                    pane_data.push(pane_render::PaneRenderData {
                                        viewport,
                                        cells,
                                        grid_size: (cols, rows),
                                        cursor_pos,
                                        cursor_opacity: if is_focused {
                                            cursor_opacity
                                        } else {
                                            0.0
                                        },
                                        marks,
                                        scrollback_len: pane_scrollback_len,
                                        scroll_offset: pane_scroll_offset,
                                        background: pane_background,
                                        graphics: pane_graphics,
                                    });
                                }
                            }

                            Some((
                                pane_data,
                                dividers,
                                pane_titles,
                                focused_viewport,
                                focused_pane_scrollback_len,
                            ))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };

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

    /// Gather all data needed for this render frame.
    /// Returns None if rendering should be skipped (no renderer, no active tab, terminal locked, etc.)
    fn gather_render_data(&mut self) -> Option<FrameRenderData> {
        let (renderer_size, visible_lines, grid_cols) = if let Some(renderer) = &self.renderer {
            let (cols, rows) = renderer.grid_size();
            (renderer.size(), rows, cols)
        } else {
            return None;
        };

        // Get active tab's terminal and immediate state snapshots (avoid long borrows)
        let (
            terminal,
            scroll_offset,
            mouse_selection,
            cache_cells,
            cache_generation,
            cache_scroll_offset,
            cache_cursor_pos,
            cache_selection,
            cached_scrollback_len,
            cached_terminal_title,
            hovered_url,
        ) = match self.tab_manager.active_tab() {
            Some(t) => (
                t.terminal.clone(),
                t.scroll_state.offset,
                t.mouse.selection,
                t.cache.cells.clone(),
                t.cache.generation,
                t.cache.scroll_offset,
                t.cache.cursor_pos,
                t.cache.selection,
                t.cache.scrollback_len,
                t.cache.terminal_title.clone(),
                t.mouse.hovered_url.clone(),
            ),
            None => return None,
        };

        // Check if shell has exited
        let _is_running = if let Ok(term) = terminal.try_lock() {
            term.is_running()
        } else {
            true // Assume running if locked
        };

        // Get scroll offset and selection from active tab

        // Get terminal cells for rendering (with dirty tracking optimization)
        // Also capture alt screen state to disable cursor shader for TUI apps
        let (mut cells, current_cursor_pos, cursor_style, is_alt_screen, current_generation) =
            if let Ok(term) = terminal.try_lock() {
                // Get current generation to check if terminal content has changed
                let current_generation = term.update_generation();

                // Normalize selection if it exists and extract mode
                let (selection, rectangular) = if let Some(sel) = mouse_selection {
                    (
                        Some(sel.normalized()),
                        sel.mode == SelectionMode::Rectangular,
                    )
                } else {
                    (None, false)
                };

                // Get cursor position and opacity (only show if we're at the bottom with no scroll offset
                // and the cursor is visible - TUI apps hide cursor via DECTCEM escape sequence)
                // If lock_cursor_visibility is enabled, ignore the terminal's visibility state
                // In copy mode, use the copy mode cursor position instead
                let cursor_visible = self.config.lock_cursor_visibility || term.is_cursor_visible();
                let current_cursor_pos = if self.copy_mode.active {
                    self.copy_mode.screen_cursor_pos(scroll_offset)
                } else if scroll_offset == 0 && cursor_visible {
                    Some(term.cursor_position())
                } else {
                    None
                };

                let cursor = current_cursor_pos.map(|pos| (pos, self.cursor_anim.cursor_opacity));

                // Get cursor style for geometric rendering
                // In copy mode, always use SteadyBlock for clear visibility
                // If lock_cursor_style is enabled, use the config's cursor style instead of terminal's
                // If lock_cursor_blink is enabled and cursor_blink is false, force steady cursor
                let cursor_style = if self.copy_mode.active && current_cursor_pos.is_some() {
                    Some(TermCursorStyle::SteadyBlock)
                } else if current_cursor_pos.is_some() {
                    if self.config.lock_cursor_style {
                        // Convert config cursor style to terminal cursor style
                        let style = if self.config.cursor_blink {
                            match self.config.cursor_style {
                                CursorStyle::Block => TermCursorStyle::BlinkingBlock,
                                CursorStyle::Beam => TermCursorStyle::BlinkingBar,
                                CursorStyle::Underline => TermCursorStyle::BlinkingUnderline,
                            }
                        } else {
                            match self.config.cursor_style {
                                CursorStyle::Block => TermCursorStyle::SteadyBlock,
                                CursorStyle::Beam => TermCursorStyle::SteadyBar,
                                CursorStyle::Underline => TermCursorStyle::SteadyUnderline,
                            }
                        };
                        Some(style)
                    } else {
                        let mut style = term.cursor_style();
                        // If blink is locked off, convert blinking styles to steady
                        if self.config.lock_cursor_blink && !self.config.cursor_blink {
                            style = match style {
                                TermCursorStyle::BlinkingBlock => TermCursorStyle::SteadyBlock,
                                TermCursorStyle::BlinkingBar => TermCursorStyle::SteadyBar,
                                TermCursorStyle::BlinkingUnderline => {
                                    TermCursorStyle::SteadyUnderline
                                }
                                other => other,
                            };
                        }
                        Some(style)
                    }
                } else {
                    None
                };

                log::trace!(
                    "Cursor: pos={:?}, opacity={:.2}, style={:?}, scroll={}, visible={}",
                    current_cursor_pos,
                    self.cursor_anim.cursor_opacity,
                    cursor_style,
                    scroll_offset,
                    term.is_cursor_visible()
                );

                // Check if we need to regenerate cells
                // Only regenerate when content actually changes, not on every cursor blink
                let needs_regeneration = cache_cells.is_none()
                || current_generation != cache_generation
                || scroll_offset != cache_scroll_offset
                || current_cursor_pos != cache_cursor_pos // Regenerate if cursor position changed
                || mouse_selection != cache_selection; // Regenerate if selection changed (including clearing)

                let cell_gen_start = std::time::Instant::now();
                let (cells, is_cache_hit) = if needs_regeneration {
                    // Generate fresh cells
                    let fresh_cells = term.get_cells_with_scrollback(
                        scroll_offset,
                        selection,
                        rectangular,
                        cursor,
                    );

                    (fresh_cells, false)
                } else {
                    // Cache hit: clone the Vec through the Arc (one allocation instead of two).
                    // apply_url_underlines needs a mutable Vec, so we still need an owned copy,
                    // but the Arc clone that extracted cache_cells from tab.cache was free.
                    (cache_cells.as_ref().expect("window_state: cache_cells must be Some when needs_regeneration is false").as_ref().clone(), true)
                };
                self.debug.cache_hit = is_cache_hit;
                self.debug.cell_gen_time = cell_gen_start.elapsed();

                // Check if alt screen is active (TUI apps like vim, htop)
                let is_alt_screen = term.is_alt_screen_active();

                (
                    cells,
                    current_cursor_pos,
                    cursor_style,
                    is_alt_screen,
                    current_generation,
                )
            } else if let Some(cached) = cache_cells {
                // Terminal locked (e.g., upload in progress), use cached cells so the
                // rest of the render pipeline (including file transfer overlay) can proceed.
                // Unwrap the Arc: if this is the sole reference the Vec is moved for free,
                // otherwise a clone is made (rare — only if another Arc clone is live).
                let cached_vec = Arc::try_unwrap(cached).unwrap_or_else(|a| (*a).clone());
                (cached_vec, cache_cursor_pos, None, false, cache_generation)
            } else {
                return None; // Terminal locked and no cache available, skip this frame
            };

        // --- Prettifier pipeline update ---
        // Feed terminal output changes to the prettifier, check debounce, and handle
        // alt-screen transitions. This runs outside the terminal lock.
        // Capture cell dims from the renderer before borrowing the tab mutably.
        let prettifier_cell_dims = self
            .renderer
            .as_ref()
            .map(|r| (r.cell_width(), r.cell_height()));
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            // Detect alt-screen transitions
            if is_alt_screen != tab.was_alt_screen {
                if let Some(ref mut pipeline) = tab.prettifier {
                    pipeline.on_alt_screen_change(is_alt_screen);
                }
                tab.was_alt_screen = is_alt_screen;
            }

            // Always check debounce (cheap: just a timestamp comparison)
            if let Some(ref mut pipeline) = tab.prettifier {
                // Keep prettifier cell dims in sync with the GPU renderer so
                // that inline graphics (e.g., Mermaid diagrams) are sized
                // correctly instead of using the fallback estimate.
                if let Some((cw, ch)) = prettifier_cell_dims {
                    pipeline.update_cell_dims(cw, ch);
                }
                pipeline.check_debounce();
            }
        }

        // Ensure cursor visibility flag for cell renderer reflects current config every frame
        // (so toggling "Hide default cursor" takes effect immediately even if no other changes)
        // Resolve hides_cursor: per-shader override -> metadata defaults -> global config
        let resolved_hides_cursor = self
            .config
            .cursor_shader
            .as_ref()
            .and_then(|name| self.config.cursor_shader_configs.get(name))
            .and_then(|override_cfg| override_cfg.hides_cursor)
            .or_else(|| {
                self.config
                    .cursor_shader
                    .as_ref()
                    .and_then(|name| self.shader_state.cursor_shader_metadata_cache.get(name))
                    .and_then(|meta| meta.defaults.hides_cursor)
            })
            .unwrap_or(self.config.cursor_shader_hides_cursor);
        // Resolve disable_in_alt_screen: per-shader override -> metadata defaults -> global config
        let resolved_disable_in_alt_screen = self
            .config
            .cursor_shader
            .as_ref()
            .and_then(|name| self.config.cursor_shader_configs.get(name))
            .and_then(|override_cfg| override_cfg.disable_in_alt_screen)
            .or_else(|| {
                self.config
                    .cursor_shader
                    .as_ref()
                    .and_then(|name| self.shader_state.cursor_shader_metadata_cache.get(name))
                    .and_then(|meta| meta.defaults.disable_in_alt_screen)
            })
            .unwrap_or(self.config.cursor_shader_disable_in_alt_screen);
        let hide_cursor_for_shader = self.config.cursor_shader_enabled
            && resolved_hides_cursor
            && !(resolved_disable_in_alt_screen && is_alt_screen);
        if let Some(renderer) = &mut self.renderer {
            renderer.set_cursor_hidden_for_shader(hide_cursor_for_shader);
        }

        // Update cache with regenerated cells (if needed)
        // Need to re-borrow as mutable after the terminal lock is released
        if !self.debug.cache_hit
            && let Some(tab) = self.tab_manager.active_tab_mut()
            && let Ok(term) = tab.terminal.try_lock()
        {
            let current_generation = term.update_generation();
            tab.cache.cells = Some(Arc::new(cells.clone()));
            tab.cache.generation = current_generation;
            tab.cache.scroll_offset = tab.scroll_state.offset;
            tab.cache.cursor_pos = current_cursor_pos;
            tab.cache.selection = tab.mouse.selection;
        }

        let mut show_scrollbar = self.should_show_scrollbar();

        let (scrollback_len, terminal_title, shell_lifecycle_events) =
            if let Ok(mut term) = terminal.try_lock() {
                // Use cursor row 0 when cursor not visible (e.g., alt screen)
                let cursor_row = current_cursor_pos.map(|(_, row)| row).unwrap_or(0);
                let sb_len = term.scrollback_len();
                term.update_scrollback_metadata(sb_len, cursor_row);

                // Drain shell lifecycle events for the prettifier pipeline
                let shell_events = term.drain_shell_lifecycle_events();

                // Feed newly completed commands into persistent history from two sources:
                // 1. Scrollback marks (populated via set_mark_command_at from grid text extraction)
                // 2. Core library command history (populated by the terminal emulator core)
                // Both sources are checked because command text may come from either path
                // depending on shell integration quality. The synced_commands set prevents
                // duplicate adds across frames and sources.
                for mark in term.scrollback_marks() {
                    if let Some(ref cmd) = mark.command
                        && !cmd.is_empty()
                        && self.overlay_ui.synced_commands.insert(cmd.clone())
                    {
                        self.overlay_ui.command_history.add(
                            cmd.clone(),
                            mark.exit_code,
                            mark.duration_ms,
                        );
                    }
                }
                for (cmd, exit_code, duration_ms) in term.core_command_history() {
                    if !cmd.is_empty() && self.overlay_ui.synced_commands.insert(cmd.clone()) {
                        self.overlay_ui
                            .command_history
                            .add(cmd, exit_code, duration_ms);
                    }
                }

                (sb_len, term.get_title(), shell_events)
            } else {
                (
                    cached_scrollback_len,
                    cached_terminal_title.clone(),
                    Vec::new(),
                )
            };

        // Capture prettifier block count before processing events/feed so we can
        // detect when new blocks are added and invalidate the cell cache.
        let prettifier_block_count_before = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.prettifier.as_ref())
            .map(|p| p.active_blocks().len())
            .unwrap_or(0);

        // Forward shell lifecycle events to the prettifier pipeline (outside terminal lock)
        if !shell_lifecycle_events.is_empty()
            && let Some(tab) = self.tab_manager.active_tab_mut()
            && let Some(ref mut pipeline) = tab.prettifier
        {
            for event in &shell_lifecycle_events {
                match event {
                    par_term_terminal::ShellLifecycleEvent::CommandStarted {
                        command,
                        absolute_line,
                    } => {
                        tab.cache.prettifier_command_start_line = Some(*absolute_line);
                        tab.cache.prettifier_command_text = Some(command.clone());
                        pipeline.on_command_start(command);
                    }
                    par_term_terminal::ShellLifecycleEvent::CommandFinished { absolute_line } => {
                        if let Some(start) = tab.cache.prettifier_command_start_line.take() {
                            let cmd_text = tab.cache.prettifier_command_text.take();
                            // Read full command output from scrollback so the
                            // prettified block covers the entire output, not just
                            // the visible portion. This ensures scrolling through
                            // long output shows prettified content throughout.
                            let output_start = start + 1;
                            if let Ok(term) = terminal.try_lock() {
                                let lines = term.lines_text_range(output_start, *absolute_line);
                                crate::debug_info!(
                                    "PRETTIFIER",
                                    "submit_command_output: {} lines (rows {}..{})",
                                    lines.len(),
                                    output_start,
                                    absolute_line
                                );
                                pipeline.submit_command_output(lines, cmd_text);
                            } else {
                                // Lock failed — fall back to boundary detector state
                                pipeline.on_command_end();
                            }
                        } else {
                            pipeline.on_command_end();
                        }
                    }
                }
            }
        }

        // Feed terminal output lines to the prettifier pipeline (gated on content changes).
        // Skip per-frame viewport feed for CommandOutput scope — it reads full output
        // from scrollback on CommandFinished instead.
        if let Some(tab) = self.tab_manager.active_tab_mut()
            && let Some(ref mut pipeline) = tab.prettifier
            && pipeline.is_enabled()
            && !is_alt_screen
            && pipeline.detection_scope()
                != crate::prettifier::boundary::DetectionScope::CommandOutput
            && (current_generation != tab.cache.prettifier_feed_generation
                || scroll_offset != tab.cache.prettifier_feed_scroll_offset)
        {
            tab.cache.prettifier_feed_generation = current_generation;
            tab.cache.prettifier_feed_scroll_offset = scroll_offset;

            // Heuristic Claude Code session detection from visible output.
            // One-time: scan for signature patterns if not yet detected.
            if !pipeline.claude_code().is_active() {
                'detect: for row_idx in 0..visible_lines {
                    let start = row_idx * grid_cols;
                    let end = (start + grid_cols).min(cells.len());
                    if start >= cells.len() {
                        break;
                    }
                    let row_text: String = cells[start..end]
                        .iter()
                        .map(|c| {
                            let g = c.grapheme.as_str();
                            if g.is_empty() || g == "\0" { " " } else { g }
                        })
                        .collect();
                    // Look for Claude Code signature patterns in output.
                    if row_text.contains("Claude Code")
                        || row_text.contains("claude.ai/code")
                        || row_text.contains("Tips for getting the best")
                        || (row_text.contains("Model:")
                            && (row_text.contains("Opus")
                                || row_text.contains("Sonnet")
                                || row_text.contains("Haiku")))
                    {
                        crate::debug_info!(
                            "PRETTIFIER",
                            "Claude Code session detected from output heuristic"
                        );
                        pipeline.mark_claude_code_active();
                        break 'detect;
                    }
                }
            }

            let is_claude_session = pipeline.claude_code().is_active();

            if is_claude_session {
                // Clear blocks when visible content changes. Claude Code
                // rewrites the screen in-place (e.g., permission prompts,
                // progress updates) without growing scrollback, so we hash
                // a sample of visible rows to detect viewport-level changes.
                let viewport_hash = {
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    // Sample every 4th row for speed; enough to catch redraws.
                    for row_idx in (0..visible_lines).step_by(4) {
                        let start = row_idx * grid_cols;
                        let end = (start + grid_cols).min(cells.len());
                        if start >= cells.len() {
                            break;
                        }
                        for c in &cells[start..end] {
                            c.grapheme.as_str().hash(&mut hasher);
                        }
                    }
                    scrollback_len.hash(&mut hasher);
                    scroll_offset.hash(&mut hasher);
                    hasher.finish()
                };
                let viewport_changed = viewport_hash != tab.cache.prettifier_feed_last_hash;
                if viewport_changed {
                    tab.cache.prettifier_feed_last_hash = viewport_hash;
                    if !pipeline.active_blocks().is_empty() {
                        pipeline.clear_blocks();
                        crate::debug_log!("PRETTIFIER", "CC viewport changed, cleared all blocks");
                    }
                }

                // Claude Code session: segment the viewport by action bullets
                // (⏺) and collapse markers. Each segment is submitted independently
                // so detection sees focused content blocks rather than the entire
                // viewport (which mixes UI chrome with markdown and causes false
                // positives). Deduplication in handle_block prevents duplicates.
                pipeline.reset_boundary();

                crate::debug_log!(
                    "PRETTIFIER",
                    "per-frame feed (CC): scanning {} visible lines, viewport_changed={}, scrollback={}, scroll_offset={}",
                    visible_lines,
                    viewport_changed,
                    scrollback_len,
                    scroll_offset
                );

                // Collect all rows with raw + reconstructed text.
                let mut rows: Vec<(String, String, usize)> = Vec::new(); // (raw, recon, abs_row)

                for row_idx in 0..visible_lines {
                    let absolute_row = scrollback_len.saturating_sub(scroll_offset) + row_idx;
                    let start = row_idx * grid_cols;
                    let end = (start + grid_cols).min(cells.len());
                    if start >= cells.len() {
                        break;
                    }

                    let row_text: String = cells[start..end]
                        .iter()
                        .map(|c| {
                            let g = c.grapheme.as_str();
                            if g.is_empty() || g == "\0" { " " } else { g }
                        })
                        .collect();

                    let line = super::reconstruct_markdown_from_cells(&cells[start..end]);
                    rows.push((row_text, line, absolute_row));
                }

                // Split into segments at action bullets (⏺) and collapse markers.
                // Each segment is the content between two boundaries.
                let mut segments: Vec<Vec<(String, usize)>> = Vec::new();
                let mut current: Vec<(String, usize)> = Vec::new();

                for (raw, recon, abs_row) in &rows {
                    let trimmed = raw.trim();
                    // Collapse markers — boundary, include the line in the
                    // preceding segment so row alignment is preserved (skipping
                    // it would cause the overlay to render wrong content at this row).
                    if raw.contains("(ctrl+o to expand)") {
                        current.push((recon.clone(), *abs_row));
                        segments.push(std::mem::take(&mut current));
                        continue;
                    }
                    // Action bullets (⏺) start a new segment
                    if trimmed.starts_with('⏺') || trimmed.starts_with("● ") {
                        if !current.is_empty() {
                            segments.push(std::mem::take(&mut current));
                        }
                        // Include this line in the new segment
                        current.push((recon.clone(), *abs_row));
                        continue;
                    }
                    // Horizontal rules (─────) are boundaries
                    if trimmed.len() > 10 && trimmed.chars().all(|c| c == '─' || c == '━') {
                        if !current.is_empty() {
                            segments.push(std::mem::take(&mut current));
                        }
                        continue;
                    }
                    current.push((recon.clone(), *abs_row));
                }
                if !current.is_empty() {
                    segments.push(current);
                }

                crate::debug_log!(
                    "PRETTIFIER",
                    "CC segmentation: {} total rows -> {} segments",
                    rows.len(),
                    segments.len()
                );

                // Submit each segment that has enough content for detection.
                // Short segments (tool call one-liners) are skipped.
                // The pipeline's handle_block() deduplicates by content hash,
                // so resubmitting the same segment on successive frames is cheap.
                let min_segment_lines = 5;
                let mut submitted = 0usize;
                let mut skipped_short = 0usize;
                let mut skipped_empty = 0usize;
                for mut segment in segments {
                    let non_empty = segment.iter().filter(|(l, _)| !l.trim().is_empty()).count();
                    if non_empty < min_segment_lines {
                        skipped_short += 1;
                        continue;
                    }

                    let pre_len = segment.len();
                    super::preprocess_claude_code_segment(&mut segment);
                    if segment.is_empty() {
                        skipped_empty += 1;
                        continue;
                    }

                    crate::debug_log!(
                        "PRETTIFIER",
                        "CC segment: {} lines (was {} before preprocess), rows={}..{}, first={:?}",
                        segment.len(),
                        pre_len,
                        segment.first().map(|(_, r)| *r).unwrap_or(0),
                        segment.last().map(|(_, r)| *r + 1).unwrap_or(0),
                        segment
                            .first()
                            .map(|(l, _)| &l[..l.floor_char_boundary(60)])
                    );

                    submitted += 1;
                    pipeline.submit_command_output(
                        std::mem::take(&mut segment),
                        Some("claude".to_string()),
                    );
                }

                crate::debug_log!(
                    "PRETTIFIER",
                    "CC segmentation complete: submitted={}, skipped_short={}, skipped_empty={}",
                    submitted,
                    skipped_short,
                    skipped_empty
                );
            } else {
                // Non-Claude session: submit the entire visible content as a
                // single block. This gives the detector full context (avoids
                // splitting markdown at blank lines) and reduces block churn.
                //
                // Throttle: during streaming, content changes every frame (~16ms).
                // Recompute a quick hash and skip if content hasn't changed.
                // If content did change, only re-submit if enough time has elapsed
                // (150ms) to avoid rendering 60 intermediate states per second.
                pipeline.reset_boundary();

                let mut lines: Vec<(String, usize)> = Vec::with_capacity(visible_lines);
                for row_idx in 0..visible_lines {
                    let absolute_row = scrollback_len.saturating_sub(scroll_offset) + row_idx;

                    let start = row_idx * grid_cols;
                    let end = (start + grid_cols).min(cells.len());
                    if start >= cells.len() {
                        break;
                    }

                    let line: String = cells[start..end]
                        .iter()
                        .map(|c| {
                            let g = c.grapheme.as_str();
                            if g.is_empty() || g == "\0" { " " } else { g }
                        })
                        .collect::<String>()
                        .trim_end()
                        .to_string();

                    lines.push((line, absolute_row));
                }

                if !lines.is_empty() {
                    // Quick content hash for dedup.
                    let content_hash = {
                        use std::hash::{Hash, Hasher};
                        let mut hasher = std::collections::hash_map::DefaultHasher::new();
                        for (line, row) in &lines {
                            line.hash(&mut hasher);
                            row.hash(&mut hasher);
                        }
                        hasher.finish()
                    };

                    if content_hash == tab.cache.prettifier_feed_last_hash {
                        // Identical content — skip entirely.
                        crate::debug_trace!(
                            "PRETTIFIER",
                            "per-frame feed (non-CC): content unchanged, skipping"
                        );
                    } else {
                        let elapsed = tab.cache.prettifier_feed_last_time.elapsed();
                        let throttle = std::time::Duration::from_millis(150);
                        let has_block = !pipeline.active_blocks().is_empty();

                        if has_block && elapsed < throttle {
                            // Actively streaming with an existing prettified block.
                            // Defer re-render to avoid per-frame churn.
                            crate::debug_trace!(
                                "PRETTIFIER",
                                "per-frame feed (non-CC): throttled ({:.0}ms < {}ms), deferring",
                                elapsed.as_secs_f64() * 1000.0,
                                throttle.as_millis()
                            );
                        } else {
                            crate::debug_log!(
                                "PRETTIFIER",
                                "per-frame feed (non-CC): submitting {} visible lines as single block, scrollback={}, scroll_offset={}",
                                visible_lines,
                                scrollback_len,
                                scroll_offset
                            );
                            tab.cache.prettifier_feed_last_hash = content_hash;
                            tab.cache.prettifier_feed_last_time = std::time::Instant::now();
                            pipeline.submit_command_output(lines, None);
                        }
                    }
                }
            }
        }

        // If new prettified blocks were added during event processing or per-frame feed,
        // invalidate the cell cache so the next frame runs cell substitution.
        {
            let block_count_after = self
                .tab_manager
                .active_tab()
                .and_then(|t| t.prettifier.as_ref())
                .map(|p| p.active_blocks().len())
                .unwrap_or(0);
            if block_count_after > prettifier_block_count_before {
                crate::debug_info!(
                    "PRETTIFIER",
                    "new blocks detected ({} -> {}), invalidating cell cache",
                    prettifier_block_count_before,
                    block_count_after
                );
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.cache.cells = None;
                }
            }
        }

        // Update cache scrollback and clamp scroll state.
        //
        // In pane mode the focused pane's own terminal holds the scrollback, not
        // `tab.terminal`.  Clamping here with `tab.terminal.scrollback_len()` would
        // incorrectly cap (or zero-out) the scroll offset every frame.  The correct
        // clamp happens later in the pane render path once we know the focused pane's
        // actual scrollback length.
        let is_pane_mode = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.pane_manager.as_ref())
            .map(|pm| pm.pane_count() > 0)
            .unwrap_or(false);
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.cache.scrollback_len = scrollback_len;
            if !is_pane_mode {
                tab.scroll_state
                    .clamp_to_scrollback(tab.cache.scrollback_len);
            }
        }

        // Keep copy mode dimensions in sync with terminal
        if self.copy_mode.active
            && let Ok(term) = terminal.try_lock()
        {
            let (cols, rows) = term.dimensions();
            self.copy_mode.update_dimensions(cols, rows, scrollback_len);
        }

        let need_marks =
            self.config.scrollbar_command_marks || self.config.command_separator_enabled;
        let mut scrollback_marks = if need_marks {
            if let Ok(term) = terminal.try_lock() {
                term.scrollback_marks()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Append trigger-generated marks
        if let Some(tab) = self.tab_manager.active_tab() {
            scrollback_marks.extend(tab.trigger_marks.iter().cloned());
        }

        // Keep scrollbar visible when mark indicators exist (even if no scrollback).
        if !scrollback_marks.is_empty() {
            show_scrollbar = true;
        }

        // Update window title if terminal has set one via OSC sequences
        // Only if allow_title_change is enabled and we're not showing a URL tooltip
        if self.config.allow_title_change
            && hovered_url.is_none()
            && terminal_title != cached_terminal_title
        {
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.cache.terminal_title = terminal_title.clone();
            }
            if let Some(window) = &self.window {
                if terminal_title.is_empty() {
                    // Restore configured title when terminal clears title
                    window.set_title(&self.format_title(&self.config.window_title));
                } else {
                    // Use terminal-set title with window number
                    window.set_title(&self.format_title(&terminal_title));
                }
            }
        }

        // Total lines = visible lines + actual scrollback content
        let total_lines = visible_lines + scrollback_len;

        // Detect URLs in visible area (only when content changed)
        // This optimization saves ~0.26ms per frame on cache hits
        let url_detect_start = std::time::Instant::now();
        let debug_url_detect_time = if !self.debug.cache_hit {
            // Content changed - re-detect URLs
            self.detect_urls();
            url_detect_start.elapsed()
        } else {
            // Content unchanged - use cached URL detection
            std::time::Duration::ZERO
        };

        // Apply URL underlining to cells (always apply, since cells might be regenerated)
        let url_underline_start = std::time::Instant::now();
        self.apply_url_underlines(&mut cells, &renderer_size);
        let _debug_url_underline_time = url_underline_start.elapsed();

        // Update search and apply search highlighting
        if self.overlay_ui.search_ui.visible {
            // Get all searchable lines from cells (ensures consistent wide character handling)
            if let Some(tab) = self.tab_manager.active_tab()
                && let Ok(term) = tab.terminal.try_lock()
            {
                let lines_iter =
                    crate::app::search_highlight::get_all_searchable_lines(&term, visible_lines);
                self.overlay_ui.search_ui.update_search(lines_iter);
            }

            // Apply search highlighting to visible cells
            let scroll_offset = self
                .tab_manager
                .active_tab()
                .map(|t| t.scroll_state.offset)
                .unwrap_or(0);
            // Use actual terminal grid columns from renderer
            self.apply_search_highlights(
                &mut cells,
                grid_cols,
                scroll_offset,
                scrollback_len,
                visible_lines,
            );
        }

        // Update cursor blink state
        self.update_cursor_blink();

        Some(FrameRenderData {
            cells,
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
        })
    }
}

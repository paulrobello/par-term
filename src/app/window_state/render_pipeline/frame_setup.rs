//! Frame setup helpers for the render pipeline.
//!
//! These functions run at the beginning of every render cycle:
//! - `should_render_frame`: FPS gate — decides whether to render this frame
//! - `update_frame_metrics`: rolling frame-time tracking for FPS overlay
//! - `update_animations`: scroll animation tick, tab title refresh, font rebuild
//! - `sync_layout`: tab bar / status bar geometry sync with renderer

use crate::app::window_state::WindowState;

impl WindowState {
    /// Returns true if enough time has elapsed since the last frame and rendering should proceed.
    /// Updates last_render_time and resets needs_redraw on success.
    pub(super) fn should_render_frame(&mut self) -> bool {
        let target_fps = if self.config.pause_refresh_on_blur && !self.focus_state.is_focused {
            self.config.unfocused_fps
        } else {
            self.config.max_fps
        };
        let frame_interval = std::time::Duration::from_millis((1000 / target_fps.max(1)) as u64);
        if let Some(last_render) = self.focus_state.last_render_time
            && last_render.elapsed() < frame_interval
        {
            return false;
        }
        self.focus_state.last_render_time = Some(std::time::Instant::now());
        self.focus_state.needs_redraw = false;
        true
    }

    /// Record the start of this render frame for timing and update rolling frame-time metrics.
    pub(super) fn update_frame_metrics(&mut self) {
        let frame_start = std::time::Instant::now();
        self.debug.render_start = Some(frame_start);
        if let Some(last_start) = self.debug.last_frame_start {
            let frame_time = frame_start.duration_since(last_start);
            self.debug.frame_times.push_back(frame_time);
            if self.debug.frame_times.len() > 60 {
                self.debug.frame_times.pop_front();
            }
        }
        self.debug.last_frame_start = Some(frame_start);
    }

    /// Tick scroll animations, refresh tab titles, and rebuild renderer if font settings changed.
    pub(super) fn update_animations(&mut self) {
        let animation_running = if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.scroll_state.update_animation()
        } else {
            false
        };

        // Update tab titles from terminal OSC sequences
        self.tab_manager
            .update_all_titles(self.config.tab_title_mode);

        // Rebuild renderer if font-related settings changed
        if self.pending_font_rebuild {
            if let Err(e) = self.rebuild_renderer() {
                log::error!("Failed to rebuild renderer after font change: {}", e);
            }
            self.pending_font_rebuild = false;
        }

        if animation_running && let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// Sync tab bar and status bar geometry with the renderer every frame.
    /// Resizes terminal grids if the tab bar dimensions changed.
    pub(super) fn sync_layout(&mut self) {
        // Sync tab bar offsets with renderer's content offsets
        // This ensures the terminal grid correctly accounts for the tab bar position
        let tab_count = self.tab_manager.tab_count();
        let tab_bar_height = self.tab_bar_ui.get_height(tab_count, &self.config);
        let tab_bar_width = self.tab_bar_ui.get_width(tab_count, &self.config);
        crate::debug_trace!(
            "TAB_SYNC",
            "Tab count={}, tab_bar_height={:.0}, tab_bar_width={:.0}, position={:?}, mode={:?}",
            tab_count,
            tab_bar_height,
            tab_bar_width,
            self.config.tab_bar_position,
            self.config.tab_bar_mode
        );
        if let Some(renderer) = &mut self.renderer {
            let grid_changed = Self::apply_tab_bar_offsets_for_position(
                self.config.tab_bar_position,
                renderer,
                tab_bar_height,
                tab_bar_width,
            );
            if let Some((new_cols, new_rows)) = grid_changed {
                let cell_width = renderer.cell_width();
                let cell_height = renderer.cell_height();
                let width_px = (new_cols as f32 * cell_width) as usize;
                let height_px = (new_rows as f32 * cell_height) as usize;

                for tab in self.tab_manager.tabs_mut() {
                    if let Ok(mut term) = tab.terminal.try_write() {
                        term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                        let _ = term.resize_with_pixels(new_cols, new_rows, width_px, height_px);
                    }
                    tab.cache.cells = None;
                }
                crate::debug_info!(
                    "TAB_SYNC",
                    "Tab bar offsets changed (position={:?}), resized terminals to {}x{}",
                    self.config.tab_bar_position,
                    new_cols,
                    new_rows
                );
            }
        }

        // Sync status bar inset so the terminal grid does not extend behind it.
        // Must happen before cell gathering so the row count is correct.
        self.sync_status_bar_inset();
    }
}

//! Tab management operations for WindowState.
//!
//! This module contains methods for creating, closing, and switching between tabs.

use std::sync::Arc;

use super::window_state::WindowState;

impl WindowState {
    /// Create a new tab
    pub fn new_tab(&mut self) {
        // Check max tabs limit
        if self.config.max_tabs > 0 && self.tab_manager.tab_count() >= self.config.max_tabs {
            log::warn!(
                "Cannot create new tab: max_tabs limit ({}) reached",
                self.config.max_tabs
            );
            return;
        }

        match self.tab_manager.new_tab(
            &self.config,
            Arc::clone(&self.runtime),
            self.config.tab_inherit_cwd,
        ) {
            Ok(tab_id) => {
                // Start refresh task for the new tab and resize to match window
                if let Some(window) = &self.window
                    && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                {
                    tab.start_refresh_task(
                        Arc::clone(&self.runtime),
                        Arc::clone(window),
                        self.config.max_fps,
                    );

                    // Resize terminal to match current renderer dimensions
                    if let Some(renderer) = &self.renderer
                        && let Ok(mut term) = tab.terminal.try_lock()
                    {
                        let (cols, rows) = renderer.grid_size();
                        let size = renderer.size();
                        let width_px = size.width as usize;
                        let height_px = size.height as usize;

                        // Set cell dimensions
                        term.set_cell_dimensions(
                            renderer.cell_width() as u32,
                            renderer.cell_height() as u32,
                        );

                        // Resize terminal to match window size
                        let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
                        log::info!(
                            "Resized new tab {} terminal to {}x{} ({}x{} px)",
                            tab_id,
                            cols,
                            rows,
                            width_px,
                            height_px
                        );
                    }
                }

                self.needs_redraw = true;
                self.request_redraw();
            }
            Err(e) => {
                log::error!("Failed to create new tab: {}", e);
            }
        }
    }

    /// Close the current tab
    /// Returns true if the window should close (last tab was closed)
    pub fn close_current_tab(&mut self) -> bool {
        if let Some(tab_id) = self.tab_manager.active_tab_id() {
            let is_last = self.tab_manager.close_tab(tab_id);
            self.needs_redraw = true;
            self.request_redraw();
            is_last
        } else {
            true // No tabs, window should close
        }
    }

    /// Switch to next tab
    pub fn next_tab(&mut self) {
        self.tab_manager.next_tab();
        self.clear_and_invalidate();
    }

    /// Switch to previous tab
    pub fn prev_tab(&mut self) {
        self.tab_manager.prev_tab();
        self.clear_and_invalidate();
    }

    /// Switch to tab by index (1-based)
    pub fn switch_to_tab_index(&mut self, index: usize) {
        self.tab_manager.switch_to_index(index);
        self.clear_and_invalidate();
    }

    /// Move current tab left
    pub fn move_tab_left(&mut self) {
        self.tab_manager.move_active_tab_left();
        self.needs_redraw = true;
        self.request_redraw();
    }

    /// Move current tab right
    pub fn move_tab_right(&mut self) {
        self.tab_manager.move_active_tab_right();
        self.needs_redraw = true;
        self.request_redraw();
    }

    /// Duplicate current tab
    pub fn duplicate_tab(&mut self) {
        match self
            .tab_manager
            .duplicate_active_tab(&self.config, Arc::clone(&self.runtime))
        {
            Ok(Some(tab_id)) => {
                // Start refresh task for the new tab
                if let Some(window) = &self.window
                    && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                {
                    tab.start_refresh_task(
                        Arc::clone(&self.runtime),
                        Arc::clone(window),
                        self.config.max_fps,
                    );
                }
                self.needs_redraw = true;
                self.request_redraw();
            }
            Ok(None) => {
                log::debug!("No active tab to duplicate");
            }
            Err(e) => {
                log::error!("Failed to duplicate tab: {}", e);
            }
        }
    }

    /// Check if there are multiple tabs
    pub fn has_multiple_tabs(&self) -> bool {
        self.tab_manager.has_multiple_tabs()
    }

    /// Get the active tab's terminal
    #[allow(dead_code)]
    pub fn active_terminal(
        &self,
    ) -> Option<&Arc<tokio::sync::Mutex<crate::terminal::TerminalManager>>> {
        self.tab_manager.active_tab().map(|tab| &tab.terminal)
    }

    // ========================================================================
    // Split Pane Operations
    // ========================================================================

    /// Split the current pane horizontally (panes stacked top/bottom)
    pub fn split_pane_horizontal(&mut self) {
        // In tmux mode, send split command to tmux instead
        if self.is_tmux_connected() && self.split_pane_via_tmux(false) {
            crate::debug_info!("TMUX", "Sent horizontal split command to tmux");
            return;
        }
        // Fall through to local split if tmux command failed or not connected

        // Get bounds info from renderer for proper pane sizing
        let bounds_info = self.renderer.as_ref().map(|r| {
            let size = r.size();
            let padding = r.window_padding();
            let content_offset_y = r.content_offset_y();
            let cell_width = r.cell_width();
            let cell_height = r.cell_height();
            (size, padding, content_offset_y, cell_width, cell_height)
        });

        if let Some(tab) = self.tab_manager.active_tab_mut() {
            // Set pane bounds before split if we have renderer info
            if let Some((size, padding, content_offset_y, cell_width, cell_height)) = bounds_info {
                let content_width = size.width as f32 - padding * 2.0;
                let content_height = size.height as f32 - content_offset_y - padding;
                let bounds = crate::pane::PaneBounds::new(
                    padding,
                    content_offset_y,
                    content_width,
                    content_height,
                );
                tab.set_pane_bounds(bounds, cell_width, cell_height);
            }

            match tab.split_horizontal(&self.config, Arc::clone(&self.runtime)) {
                Ok(Some(pane_id)) => {
                    log::info!("Split pane horizontally, new pane {}", pane_id);
                    // Clear renderer cells to remove stale single-pane data
                    if let Some(renderer) = &mut self.renderer {
                        renderer.clear_all_cells();
                    }
                    // Invalidate tab cache
                    tab.cache.cells = None;
                    self.needs_redraw = true;
                    self.request_redraw();
                }
                Ok(None) => {
                    log::info!(
                        "Horizontal split not yet functional (renderer integration pending)"
                    );
                }
                Err(e) => {
                    log::error!("Failed to split pane horizontally: {}", e);
                }
            }
        }
    }

    /// Split the current pane vertically (panes side by side)
    pub fn split_pane_vertical(&mut self) {
        // In tmux mode, send split command to tmux instead
        if self.is_tmux_connected() && self.split_pane_via_tmux(true) {
            crate::debug_info!("TMUX", "Sent vertical split command to tmux");
            return;
        }
        // Fall through to local split if tmux command failed or not connected

        // Get bounds info from renderer for proper pane sizing
        let bounds_info = self.renderer.as_ref().map(|r| {
            let size = r.size();
            let padding = r.window_padding();
            let content_offset_y = r.content_offset_y();
            let cell_width = r.cell_width();
            let cell_height = r.cell_height();
            (size, padding, content_offset_y, cell_width, cell_height)
        });

        if let Some(tab) = self.tab_manager.active_tab_mut() {
            // Set pane bounds before split if we have renderer info
            if let Some((size, padding, content_offset_y, cell_width, cell_height)) = bounds_info {
                let content_width = size.width as f32 - padding * 2.0;
                let content_height = size.height as f32 - content_offset_y - padding;
                let bounds = crate::pane::PaneBounds::new(
                    padding,
                    content_offset_y,
                    content_width,
                    content_height,
                );
                tab.set_pane_bounds(bounds, cell_width, cell_height);
            }

            match tab.split_vertical(&self.config, Arc::clone(&self.runtime)) {
                Ok(Some(pane_id)) => {
                    log::info!("Split pane vertically, new pane {}", pane_id);
                    // Clear renderer cells to remove stale single-pane data
                    if let Some(renderer) = &mut self.renderer {
                        renderer.clear_all_cells();
                    }
                    // Invalidate tab cache
                    tab.cache.cells = None;
                    self.needs_redraw = true;
                    self.request_redraw();
                }
                Ok(None) => {
                    log::info!("Vertical split not yet functional (renderer integration pending)");
                }
                Err(e) => {
                    log::error!("Failed to split pane vertically: {}", e);
                }
            }
        }
    }

    /// Close the focused pane in the current tab
    ///
    /// If this is the last pane, the tab is closed.
    /// Returns true if the window should close (last tab was closed).
    pub fn close_focused_pane(&mut self) -> bool {
        // In tmux mode, send kill-pane command to tmux
        if self.is_tmux_connected() && self.close_pane_via_tmux() {
            crate::debug_info!("TMUX", "Sent kill-pane command to tmux");
            // Don't close the local pane - wait for tmux layout change
            return false;
        }
        // Fall through to local close if tmux command failed or not connected

        if let Some(tab) = self.tab_manager.active_tab_mut()
            && tab.has_multiple_panes()
        {
            let is_last_pane = tab.close_focused_pane();
            if is_last_pane {
                // Last pane closed, close the tab
                return self.close_current_tab();
            }
            self.needs_redraw = true;
            self.request_redraw();
            return false;
        }
        // Single pane or no tab, close the tab
        self.close_current_tab()
    }

    /// Check if the current tab has multiple panes
    pub fn has_multiple_panes(&self) -> bool {
        self.tab_manager
            .active_tab()
            .is_some_and(|tab| tab.has_multiple_panes())
    }

    /// Navigate to an adjacent pane in the given direction
    pub fn navigate_pane(&mut self, direction: crate::pane::NavigationDirection) {
        if let Some(tab) = self.tab_manager.active_tab_mut()
            && tab.has_multiple_panes()
        {
            tab.navigate_pane(direction);
            self.needs_redraw = true;
            self.request_redraw();
        }
    }

    /// Resize the focused pane in the given direction
    ///
    /// Growing left/up decreases the pane's ratio, growing right/down increases it
    pub fn resize_pane(&mut self, direction: crate::pane::NavigationDirection) {
        use crate::pane::NavigationDirection;

        // Resize step: 5% per keypress
        const RESIZE_DELTA: f32 = 0.05;

        // Determine delta based on direction
        // Right/Down: grow focused pane (positive delta)
        // Left/Up: shrink focused pane (negative delta)
        let delta = match direction {
            NavigationDirection::Right | NavigationDirection::Down => RESIZE_DELTA,
            NavigationDirection::Left | NavigationDirection::Up => -RESIZE_DELTA,
        };

        if let Some(tab) = self.tab_manager.active_tab_mut()
            && let Some(pm) = tab.pane_manager_mut()
            && let Some(focused_id) = pm.focused_pane_id()
        {
            pm.resize_split(focused_id, delta);
            self.needs_redraw = true;
            self.request_redraw();
        }
    }
}

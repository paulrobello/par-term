//! Window-level tmux notification handlers.
//!
//! Covers window add, close, and rename events from the tmux control-mode
//! session. Each event maps to a corresponding tab operation.

use crate::app::window_state::WindowState;
use crate::tmux::TmuxWindowId;

impl WindowState {
    /// Handle window add notification - creates a new tab
    pub(super) fn handle_tmux_window_add(&mut self, window_id: TmuxWindowId) {
        crate::debug_info!("TMUX", "Window added: @{}", window_id);

        // Check max tabs limit
        if self.config.max_tabs > 0 && self.tab_manager.tab_count() >= self.config.max_tabs {
            crate::debug_error!(
                "TMUX",
                "Cannot create tab for tmux window @{}: max_tabs limit ({}) reached",
                window_id,
                self.config.max_tabs
            );
            return;
        }

        // Get current grid size from renderer
        let grid_size = self.renderer.as_ref().map(|r| r.grid_size());

        // Create a new tab for this tmux window
        match self.tab_manager.new_tab(
            &self.config,
            std::sync::Arc::clone(&self.runtime),
            false, // Don't inherit CWD from active tab for tmux
            grid_size,
        ) {
            Ok(tab_id) => {
                crate::debug_info!(
                    "TMUX",
                    "Created tab {} for tmux window @{}",
                    tab_id,
                    window_id
                );

                // Register the mapping
                self.tmux_state.tmux_sync.map_window(window_id, tab_id);

                // Set initial title based on tmux window ID
                // Note: These tabs are for displaying tmux windows, but the gateway tab
                // is where the actual tmux control connection lives. We store the tmux pane ID
                // on the tab so we know which pane to route input to.
                if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
                    tab.set_title(&format!("tmux @{}", window_id));
                    // Note: Don't set tmux_gateway_active here - only the gateway tab is the gateway

                    // Start refresh task for the new tab
                    if let Some(window) = &self.window {
                        tab.start_refresh_task(
                            std::sync::Arc::clone(&self.runtime),
                            std::sync::Arc::clone(window),
                            self.config.max_fps,
                            self.config.inactive_tab_fps,
                        );
                    }

                    // Resize terminal to match current renderer dimensions
                    // try_lock: intentional — called during window-add handling in the sync event
                    // loop. On miss: the new tmux tab's terminal is not resized this frame; the
                    // size will be corrected on the next Resized event.
                    if let Some(renderer) = &self.renderer
                        && let Ok(mut term) = tab.terminal.try_write()
                    {
                        let (cols, rows) = renderer.grid_size();
                        let size = renderer.size();
                        let width_px = size.width as usize;
                        let height_px = size.height as usize;

                        term.set_cell_dimensions(
                            renderer.cell_width() as u32,
                            renderer.cell_height() as u32,
                        );
                        let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
                        crate::debug_info!(
                            "TMUX",
                            "Resized tmux tab {} terminal to {}x{}",
                            tab_id,
                            cols,
                            rows
                        );
                    }
                }
            }
            Err(e) => {
                crate::debug_error!(
                    "TMUX",
                    "Failed to create tab for tmux window @{}: {}",
                    window_id,
                    e
                );
            }
        }
    }
}

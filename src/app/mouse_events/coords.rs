use crate::app::window_state::WindowState;
use std::sync::Arc;

impl WindowState {
    /// Convert pixel coordinates to terminal cell coordinates
    pub(crate) fn pixel_to_cell(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        if let Some(renderer) = &self.renderer {
            // Use actual cell dimensions from renderer for accurate coordinate mapping
            let cell_width = renderer.cell_width() as f64;
            let cell_height = renderer.cell_height() as f64;
            let padding = renderer.window_padding() as f64;
            let content_offset_y = renderer.content_offset_y() as f64;
            let content_offset_x = renderer.content_offset_x() as f64;

            // Account for window padding (all sides) and content offsets (tab bar)
            let adjusted_x = (x - padding - content_offset_x).max(0.0);
            let adjusted_y = (y - padding - content_offset_y).max(0.0);

            let col = (adjusted_x / cell_width) as usize;
            let row = (adjusted_y / cell_height) as usize;

            Some((col, row))
        } else {
            None
        }
    }

    /// Convert pixel coordinates to cell coordinates relative to a specific pane
    ///
    /// Accounts for pane bounds, pane padding, and pane title bar offset.
    /// Returns None if the pixel coordinates are outside the pane's bounds
    /// or if the renderer is not available.
    pub(crate) fn pixel_to_pane_cell(
        &self,
        x: f64,
        y: f64,
        pane_bounds: &crate::pane::PaneBounds,
    ) -> Option<(usize, usize)> {
        let renderer = self.renderer.as_ref()?;

        // Check if the point is inside the pane's bounds
        let bx = pane_bounds.x as f64;
        let by = pane_bounds.y as f64;
        let bw = pane_bounds.width as f64;
        let bh = pane_bounds.height as f64;
        if x < bx || x >= bx + bw || y < by || y >= by + bh {
            return None;
        }

        let cell_width = renderer.cell_width() as f64;
        let cell_height = renderer.cell_height() as f64;

        // Calculate physical pane padding (config is logical, scale for DPI)
        let scale = renderer.scale_factor() as f64;
        let pane_padding = if self.is_gateway_active() {
            0.0
        } else {
            self.config.pane_padding as f64 * scale
        };

        // Account for pane title bar if enabled
        let title_offset = if self.config.show_pane_titles {
            self.config.pane_title_height as f64 * scale
        } else {
            0.0
        };

        // Convert to pane-local coordinates
        let local_x = (x - bx - pane_padding).max(0.0);
        let local_y = (y - by - pane_padding - title_offset).max(0.0);

        let col = (local_x / cell_width) as usize;
        let row = (local_y / cell_height) as usize;

        Some((col, row))
    }

    /// Handle a file being dropped into the terminal window.
    ///
    /// Quotes the file path according to the configured style and writes it
    /// to the active terminal session.
    pub(crate) fn handle_dropped_file(&mut self, path: std::path::PathBuf) {
        use crate::shell_quote::quote_path;

        // Quote the path according to the configured style
        let quoted_path = quote_path(&path, self.config.dropped_file_quote_style);

        log::debug!(
            "File dropped: {:?} -> {} (style: {:?})",
            path,
            quoted_path,
            self.config.dropped_file_quote_style
        );

        // Write the quoted path to the terminal
        if let Some(tab) = self.tab_manager.active_tab() {
            let terminal_clone = Arc::clone(&tab.terminal);
            let runtime = Arc::clone(&self.runtime);

            runtime.spawn(async move {
                let term = terminal_clone.write().await;
                let bytes = quoted_path.as_bytes().to_vec();
                if let Err(e) = term.write(&bytes) {
                    log::error!("Failed to write dropped file path to terminal: {}", e);
                }
            });

            // Request redraw in case terminal needs to update
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }
}

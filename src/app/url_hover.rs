//! URL detection and hover state for WindowState.
//!
//! This module contains methods for detecting URLs in the terminal
//! and applying visual styling to indicate clickable links.

use crate::url_detection;

use super::window_state::WindowState;

impl WindowState {
    /// Detect URLs in the visible terminal area (both regex-detected and OSC 8 hyperlinks)
    pub(crate) fn detect_urls(&mut self) {
        // Gather data from active tab
        let (cols, rows, visible_cells, scroll_offset, hyperlink_urls) = {
            let tab = if let Some(t) = self.tab_manager.active_tab() {
                t
            } else {
                return;
            };

            // try_lock: intentional — URL hover detection on every mouse-move frame in
            // the sync event loop. On miss: hovered URL is not updated this frame. The
            // cursor shows the last known state — benign cosmetic lag.
            if let Ok(term) = tab.terminal.try_write() {
                let (cols, rows) = term.dimensions();
                let scroll_offset = tab.scroll_state.offset;
                let visible_cells =
                    term.get_cells_with_scrollback(scroll_offset, None, false, None);

                if visible_cells.is_empty() || cols == 0 {
                    return;
                }

                // Build hyperlink ID to URL mapping from terminal
                let mut hyperlink_urls = std::collections::HashMap::new();
                let all_hyperlinks = term.get_all_hyperlinks();
                for hyperlink_info in all_hyperlinks {
                    // Get the hyperlink ID from the first position
                    if let Some((col, row)) = hyperlink_info.positions.first() {
                        // Get the cell at this position to find the hyperlink_id
                        let cell_idx = row * cols + col;
                        if let Some(cell) = visible_cells.get(cell_idx)
                            && let Some(id) = cell.hyperlink_id
                        {
                            hyperlink_urls.insert(id, hyperlink_info.url.clone());
                        }
                    }
                }

                (cols, rows, visible_cells, scroll_offset, hyperlink_urls)
            } else {
                return;
            }
        };

        // Check if hover state needs to be cleared before taking mutable borrow.
        // This resets the pointer cursor and title bar file info when content changes
        // so they don't persist for files that have scrolled off screen.
        let had_hovered_url = self
            .tab_manager
            .active_tab()
            .is_some_and(|t| t.mouse.hovered_url.is_some());
        if had_hovered_url && let Some(window) = &self.window {
            window.set_cursor(winit::window::CursorIcon::Text);
            let title = self.format_title(&self.config.window_title);
            window.set_title(&title);
        }

        // Clear and rebuild detected URLs
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.mouse.detected_urls.clear();
            tab.mouse.hovered_url = None;

            // Extract text from each visible line and detect URLs
            for row in 0..rows {
                let start_idx = row * cols;
                let end_idx = start_idx.saturating_add(cols);
                if end_idx > visible_cells.len() {
                    break;
                }

                let row_cells = &visible_cells[start_idx..end_idx];

                // Build line text and byte-to-column mapping.
                // Regex returns byte offsets into the string, but we need column
                // indices for cell highlighting. When graphemes contain multi-byte
                // UTF-8 (prompt icons, Unicode chars, etc.), byte offsets diverge
                // from column positions.
                let mut line = String::with_capacity(cols);
                let mut byte_to_col: Vec<usize> = Vec::with_capacity(cols * 4);
                for (col_idx, cell) in row_cells.iter().enumerate() {
                    for _ in 0..cell.grapheme.len() {
                        byte_to_col.push(col_idx);
                    }
                    line.push_str(&cell.grapheme);
                }
                // Sentinel for end-of-string byte positions (exclusive end)
                byte_to_col.push(cols);

                let map_byte_to_col = |byte_offset: usize| -> usize {
                    byte_to_col.get(byte_offset).copied().unwrap_or(cols)
                };

                // Adjust row to account for scroll offset
                let absolute_row = row + scroll_offset;

                // Detect regex-based URLs in this line and convert byte offsets to columns
                let regex_urls = url_detection::detect_urls_in_line(&line, absolute_row);
                tab.mouse
                    .detected_urls
                    .extend(regex_urls.into_iter().map(|mut url| {
                        url.start_col = map_byte_to_col(url.start_col);
                        url.end_col = map_byte_to_col(url.end_col);
                        url
                    }));

                // Detect OSC 8 hyperlinks in this row (already use column indices)
                let osc8_urls =
                    url_detection::detect_osc8_hyperlinks(row_cells, absolute_row, &hyperlink_urls);
                tab.mouse.detected_urls.extend(osc8_urls);

                // Detect file paths for semantic history (if enabled)
                if self.config.semantic_history_enabled {
                    let file_paths = url_detection::detect_file_paths_in_line(&line, absolute_row);
                    tab.mouse
                        .detected_urls
                        .extend(file_paths.into_iter().map(|mut fp| {
                            crate::debug_trace!(
                                "SEMANTIC",
                                "Detected path: {:?} at cols {}..{} row {}",
                                fp.url,
                                map_byte_to_col(fp.start_col),
                                map_byte_to_col(fp.end_col),
                                fp.row
                            );
                            fp.start_col = map_byte_to_col(fp.start_col);
                            fp.end_col = map_byte_to_col(fp.end_col);
                            fp
                        }));
                }
            }
        }
    }

    /// Apply visual styling to cells that are part of detected URLs
    /// Changes the foreground color to indicate clickable URLs
    pub(crate) fn apply_url_underlines(
        &self,
        cells: &mut [crate::cell_renderer::Cell],
        _renderer_size: &winit::dpi::PhysicalSize<u32>,
    ) {
        let tab = if let Some(t) = self.tab_manager.active_tab() {
            t
        } else {
            return;
        };

        if tab.mouse.detected_urls.is_empty() {
            return;
        }

        // Get actual terminal columns from the terminal
        // try_lock: intentional — URL highlight rendering in the sync render path.
        // On miss: URL highlight falls back to default (no highlight this frame). Cosmetic.
        let cols = if let Ok(term) = tab.terminal.try_write() {
            let (cols, _rows) = term.dimensions();
            cols
        } else {
            return;
        };

        let c = self.config.link_highlight_color;
        let url_color = [c[0], c[1], c[2], 255];

        let scroll_offset = tab.scroll_state.offset;

        // Apply color styling to cells that are part of URLs
        for url in &tab.mouse.detected_urls {
            // Convert absolute row (with scroll offset) to viewport-relative row
            if url.row < scroll_offset {
                continue; // URL is above the visible area
            }
            let viewport_row = url.row - scroll_offset;

            // Calculate cell indices for this URL
            for col in url.start_col..url.end_col {
                let cell_idx = viewport_row * cols + col;
                if cell_idx < cells.len() {
                    cells[cell_idx].fg_color = url_color;
                    if self.config.link_highlight_underline {
                        cells[cell_idx].underline = true;
                    }
                }
            }
        }
    }
}

//! URL detection and hover state for WindowState.
//!
//! This module contains methods for detecting URLs in the terminal
//! and applying visual styling to indicate clickable links.

use crate::cell_renderer::Cell;
use crate::url_detection;

use super::WindowState;

/// Pre-gathered data for URL detection, avoiding redundant cell generation.
pub(crate) struct UrlDetectData<'a> {
    pub cells: &'a [Cell],
    pub cols: usize,
    pub rows: usize,
    pub scroll_offset: usize,
}

impl WindowState {
    /// Detect URLs in the visible terminal area (both regex-detected and OSC 8 hyperlinks).
    ///
    /// Accepts pre-generated cells from the render pipeline to avoid a redundant
    /// (and potentially blocking) `get_cells_with_scrollback()` call.  Only the
    /// hyperlink metadata still requires a terminal lock, which is acquired
    /// non-blockingly via `try_get_all_hyperlinks()`.
    pub(crate) fn detect_urls(&mut self, data: UrlDetectData<'_>) {
        let UrlDetectData {
            cells: visible_cells,
            cols,
            rows,
            scroll_offset,
        } = data;

        if visible_cells.is_empty() || cols == 0 {
            return;
        }

        // Fetch OSC 8 hyperlink metadata non-blockingly.
        // On lock contention (PTY reader busy), skip hyperlink detection for this
        // frame — regex-based URLs still work, and stale OSC 8 data from the
        // previous successful fetch is acceptable.
        let hyperlink_urls = {
            let tab = if let Some(t) = self.tab_manager.active_tab() {
                t
            } else {
                return;
            };

            let pane_terminal = tab
                .pane_manager
                .as_ref()
                .and_then(|pm| pm.focused_pane())
                .map(|p| std::sync::Arc::clone(&p.terminal));
            let pane_terminal = match pane_terminal {
                Some(t) => t,
                None => std::sync::Arc::clone(&tab.terminal),
            };

            // try_read: intentional — hyperlink metadata only needs read access.
            // On miss: skip OSC 8 hyperlink detection (regex URLs still detected).
            if let Ok(term) = pane_terminal.try_read() {
                let mut map = std::collections::HashMap::new();
                if let Some(all_hyperlinks) = term.try_get_all_hyperlinks() {
                    for hyperlink_info in all_hyperlinks {
                        if let Some((col, row)) = hyperlink_info.positions.first() {
                            let cell_idx = row * cols + col;
                            if let Some(cell) = visible_cells.get(cell_idx)
                                && let Some(id) = cell.hyperlink_id
                            {
                                map.insert(id, hyperlink_info.url.clone());
                            }
                        }
                    }
                }
                map
            } else {
                std::collections::HashMap::new()
            }
        };

        // Build new URL list into a local vec — keeps detected_urls stable
        // until the full list is ready so there is no intermediate empty-list frame.
        let mut new_urls: Vec<url_detection::DetectedUrl> = Vec::new();

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
            new_urls.extend(regex_urls.into_iter().map(|mut url| {
                url.start_col = map_byte_to_col(url.start_col);
                url.end_col = map_byte_to_col(url.end_col);
                url
            }));

            // Detect OSC 8 hyperlinks in this row (already use column indices)
            if !hyperlink_urls.is_empty() {
                let osc8_urls =
                    url_detection::detect_osc8_hyperlinks(row_cells, absolute_row, &hyperlink_urls);
                new_urls.extend(osc8_urls);
            }

            // Detect file paths for semantic history (if enabled)
            if self.config.load().semantic_history_enabled {
                let file_paths = url_detection::detect_file_paths_in_line(&line, absolute_row);
                new_urls.extend(file_paths.into_iter().map(|mut fp| {
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

        // Commit the new URL list.
        // Hover state (hovered_url, hovered_url_bounds) and cursor are intentionally
        // NOT touched here — mouse_move owns that state. On the next mouse-move event,
        // mouse_move will verify the hovered URL still exists in the new list and clear
        // hover + cursor if it has scrolled away. This avoids cursor flicker that would
        // occur if we reset the cursor here and then had to restore it immediately after.
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.active_mouse_mut().detected_urls = new_urls;
            tab.active_mouse_mut().url_detect_scroll_offset = scroll_offset;
        }
    }
}

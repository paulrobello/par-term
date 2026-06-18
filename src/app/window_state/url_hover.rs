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

        // Per-visible-row soft-wrap continuation flags, so a URL that wraps
        // across rows is detected as a single link instead of a truncated
        // per-row fragment. `wrapped[r] == true` means visible row r is a
        // soft-wrap continuation of r-1. On lock contention this is empty and
        // detection falls back to per-row behaviour for this frame.
        let wrapped = self
            .tab_manager
            .active_tab()
            .and_then(|tab| {
                tab.pane_manager
                    .as_ref()
                    .and_then(|pm| pm.focused_pane())
                    .map(|p| std::sync::Arc::clone(&p.terminal))
            })
            .and_then(|pane_terminal| {
                // Consume the read guard within this closure: only the owned
                // Vec<bool> escapes, so nothing references `pane_terminal`.
                pane_terminal
                    .try_read()
                    .ok()
                    .map(|term| term.viewport_wrap_flags(scroll_offset, rows))
            })
            .unwrap_or_default();

        let mut row = 0usize;
        while row < rows {
            let start_idx = row * cols;
            let end_idx = start_idx.saturating_add(cols);
            if end_idx > visible_cells.len() {
                break;
            }

            // Group this row with any following soft-wrap continuations into one
            // logical line, so a URL split across a wrap is matched as a whole.
            let mut group_end = row + 1;
            while group_end < rows
                && wrapped.get(group_end).copied().unwrap_or(false)
                && group_end * cols + cols <= visible_cells.len()
            {
                group_end += 1;
            }

            // Build the joined logical-line text plus a byte -> (visible_row, col)
            // map. Regex returns byte offsets; we map them back to per-row columns
            // so each wrapped segment gets its own clickable entry with the full
            // URL. When graphemes contain multi-byte UTF-8, byte offsets diverge
            // from column positions.
            let mut line = String::with_capacity((group_end - row) * cols);
            let mut byte_to_cell: Vec<(usize, usize)> = Vec::with_capacity(line.capacity() * 4);
            for r in row..group_end {
                let rs = r * cols;
                for (col_idx, cell) in visible_cells[rs..rs + cols].iter().enumerate() {
                    for _ in 0..cell.grapheme.len() {
                        byte_to_cell.push((r, col_idx));
                    }
                    line.push_str(&cell.grapheme);
                }
            }
            // Sentinel for byte offsets at/after the string end (exclusive-end
            // lookups). Matches the previous per-row `byte_to_col.push(cols)`.
            byte_to_cell.push((group_end - 1, cols));

            let absolute_row = row + scroll_offset;

            // Detect regex-based URLs in the joined line and emit one segment
            // per wrapped row, each carrying the full URL text.
            let regex_urls = url_detection::detect_urls_in_line(&line, absolute_row);
            for url in regex_urls {
                push_url_segments(
                    &mut new_urls,
                    &url.url,
                    &url.item_type,
                    &byte_to_cell,
                    url.start_col,
                    url.end_col,
                    scroll_offset,
                );
            }

            // Detect OSC 8 hyperlinks per row (the URL is stored by id and is
            // never truncated by wrapping, so no cross-row join is needed).
            if !hyperlink_urls.is_empty() {
                for r in row..group_end {
                    let rs = r * cols;
                    let row_cells = &visible_cells[rs..rs + cols];
                    let osc8_urls = url_detection::detect_osc8_hyperlinks(
                        row_cells,
                        r + scroll_offset,
                        &hyperlink_urls,
                    );
                    new_urls.extend(osc8_urls);
                }
            }

            // Detect file paths for semantic history (if enabled), using the same
            // wrap-aware segmentation as URLs.
            if self.config.load().semantic_history_enabled {
                let file_paths = url_detection::detect_file_paths_in_line(&line, absolute_row);
                for fp in file_paths {
                    crate::debug_trace!(
                        "SEMANTIC",
                        "Detected path: {:?} at bytes {}..{} row {}",
                        fp.url,
                        fp.start_col,
                        fp.end_col,
                        fp.row
                    );
                    push_url_segments(
                        &mut new_urls,
                        &fp.url,
                        &fp.item_type,
                        &byte_to_cell,
                        fp.start_col,
                        fp.end_col,
                        scroll_offset,
                    );
                }
            }

            row = group_end;
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

/// Emit one [`DetectedUrl`] per visible row spanned by a regex match.
///
/// A soft-wrapped URL/path is matched against the joined logical-line text, so
/// `byte_to_cell` may map the match across several visible rows. Each touched
/// row becomes its own clickable segment carrying the full `full_text`, so
/// clicking any wrapped portion of the link opens the complete URL/path rather
/// than the truncated per-row fragment.
///
/// `byte_to_cell[byte] = (visible_row, col)`; `[start_byte, end_byte)` is the
/// match's byte range (exclusive end). For a single-row match this reduces to
/// exactly the previous per-row behaviour.
fn push_url_segments(
    out: &mut Vec<url_detection::DetectedUrl>,
    full_text: &str,
    item_type: &url_detection::DetectedItemType,
    byte_to_cell: &[(usize, usize)],
    start_byte: usize,
    end_byte: usize,
    scroll_offset: usize,
) {
    // Collect each touched row's min/max column. `byte_to_cell` is built
    // left-to-right, so rows appear contiguously and in order.
    let mut segs: Vec<(usize, usize, usize)> = Vec::new(); // (row, min_col, max_col)
    for bi in start_byte..end_byte {
        let Some(&(row, col)) = byte_to_cell.get(bi) else {
            continue;
        };
        match segs.last_mut() {
            Some((r, min_col, max_col)) if *r == row => {
                if col < *min_col {
                    *min_col = col;
                }
                if col > *max_col {
                    *max_col = col;
                }
            }
            _ => segs.push((row, col, col)),
        }
    }

    for (row, min_col, max_col) in segs {
        out.push(url_detection::DetectedUrl {
            url: full_text.to_string(),
            start_col: min_col,
            end_col: max_col + 1, // exclusive
            row: row + scroll_offset,
            hyperlink_id: None,
            item_type: item_type.clone(),
        });
    }
}

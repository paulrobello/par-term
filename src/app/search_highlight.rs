//! Search highlighting functionality for terminal cells.

use crate::app::window_state::WindowState;
use crate::cell_renderer::Cell;

impl WindowState {
    /// Apply search highlighting to cells that contain matches.
    ///
    /// # Arguments
    /// * `cells` - The visible cells to modify
    /// * `cols` - Number of columns in the terminal
    /// * `scroll_offset` - Current scroll position (0 = at bottom)
    /// * `scrollback_len` - Total scrollback length
    /// * `visible_lines` - Number of visible terminal rows
    pub(crate) fn apply_search_highlights(
        &self,
        cells: &mut [Cell],
        cols: usize,
        scroll_offset: usize,
        scrollback_len: usize,
        visible_lines: usize,
    ) {
        let matches = self.overlay_ui.search_ui.matches();
        if matches.is_empty() {
            return;
        }

        let current_match_idx = self.overlay_ui.search_ui.current_match_index();
        let highlight_color = self.config.search.search_highlight_color;
        let current_highlight_color = self.config.search.search_current_highlight_color;

        // Calculate the range of absolute lines that are currently visible
        // scroll_offset = 0 means we're at the bottom (most recent content)
        // scroll_offset = scrollback_len means we're at the top
        let total_lines = scrollback_len + visible_lines;
        let visible_end = total_lines.saturating_sub(scroll_offset);
        let visible_start = visible_end.saturating_sub(visible_lines);

        log::trace!(
            "Search highlight: scroll_offset={}, scrollback_len={}, visible_lines={}, total_lines={}, visible_range={}..{}",
            scroll_offset,
            scrollback_len,
            visible_lines,
            total_lines,
            visible_start,
            visible_end
        );

        for (match_idx, search_match) in matches.iter().enumerate() {
            // Check if this match is in the visible range
            if search_match.line < visible_start || search_match.line >= visible_end {
                continue;
            }

            // Convert absolute line to viewport row
            let viewport_row = search_match.line - visible_start;

            // Determine which highlight color to use
            let color = if match_idx == current_match_idx {
                current_highlight_color
            } else {
                highlight_color
            };

            // Apply highlighting to each character in the match
            for offset in 0..search_match.length {
                let col = search_match.column + offset;
                if col >= cols {
                    break; // Match extends beyond visible columns
                }

                let cell_idx = viewport_row * cols + col;
                if cell_idx < cells.len() {
                    // Set background color to highlight color
                    cells[cell_idx].bg_color = color;
                    // Keep text visible - if bg is bright, might want to adjust fg
                    // For now, leave fg color as-is since our highlight colors have transparency
                }
            }
        }
    }
}

/// Get the current screen lines as strings (the visible terminal content, not scrollback).
///
/// # Arguments
/// * `term` - The terminal manager
/// * `visible_lines` - Number of visible terminal rows
pub(crate) fn get_current_screen_lines(
    term: &crate::terminal::TerminalManager,
    visible_lines: usize,
) -> Vec<String> {
    // Get cells with scroll_offset=0 to get current screen content
    let cells = term.get_cells_with_scrollback(0, None, false, None);

    // Convert cells to lines
    let cols = term.dimensions().0;
    cells_to_lines(&cells, cols, visible_lines)
}

/// Get all searchable lines (scrollback + current screen) as an iterator of (line_index, line_text).
///
/// This function ensures consistent handling of wide characters by converting all content
/// from cells rather than using pre-built scrollback strings.
///
/// # Arguments
/// * `term` - The terminal manager
/// * `visible_lines` - Number of visible terminal rows
///
/// # Returns
/// Iterator of (absolute_line_index, line_text) pairs where line 0 is the oldest scrollback line.
pub(crate) fn get_all_searchable_lines(
    term: &crate::terminal::TerminalManager,
    visible_lines: usize,
) -> impl Iterator<Item = (usize, String)> {
    let cols = term.dimensions().0;
    let scrollback_len = term.scrollback_len();

    // Get scrollback lines from their cell representation
    let scrollback_lines = term.scrollback_as_cells();
    let scrollback_iter = scrollback_lines
        .into_iter()
        .enumerate()
        .map(move |(idx, cells)| {
            let line = cells_row_to_string(&cells, cols);
            (idx, line)
        });

    // Get current screen lines
    let screen_cells = term.get_cells_with_scrollback(0, None, false, None);
    let current_lines = cells_to_lines(&screen_cells, cols, visible_lines);
    let current_iter = current_lines
        .into_iter()
        .enumerate()
        .map(move |(idx, line)| (scrollback_len + idx, line));

    scrollback_iter.chain(current_iter)
}

/// Convert a flat array of cells into lines.
fn cells_to_lines(cells: &[Cell], cols: usize, num_lines: usize) -> Vec<String> {
    let mut lines = Vec::with_capacity(num_lines);

    for row in 0..num_lines {
        let row_start = row * cols;
        let row_end = (row_start + cols).min(cells.len());

        if row_start >= cells.len() {
            lines.push(String::new());
            continue;
        }

        let line = cells_row_to_string(&cells[row_start..row_end], cols);
        lines.push(line);
    }

    lines
}

/// Convert a row of cells to a string for searching.
/// Wide character spacers are converted to spaces to maintain cell index alignment.
fn cells_row_to_string(cells: &[Cell], _cols: usize) -> String {
    let line: String = cells
        .iter()
        .map(|cell| {
            if cell.grapheme.is_empty() || cell.wide_char_spacer {
                ' '
            } else {
                cell.grapheme.chars().next().unwrap_or(' ')
            }
        })
        .collect();

    // Trim trailing whitespace but keep the line
    line.trim_end().to_string()
}

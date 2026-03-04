//! Search highlighting functionality for terminal cells.

use super::WindowState;
use crate::cell_renderer::Cell;
use crate::search::SearchMatch;

impl WindowState {
    /// Apply search highlighting to cells that contain matches.
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
        apply_search_highlights_to_cells(
            cells,
            cols,
            scroll_offset,
            scrollback_len,
            visible_lines,
            matches,
            self.overlay_ui.search_ui.current_match_index(),
            self.config.search.search_highlight_color,
            self.config.search.search_current_highlight_color,
        );
    }
}

/// Apply search highlights directly to a cell slice.
///
/// Used both by the single-pane path (via `WindowState::apply_search_highlights`)
/// and the pane-manager path (applied per-pane after `gather_pane_render_data`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_search_highlights_to_cells(
    cells: &mut [Cell],
    cols: usize,
    scroll_offset: usize,
    scrollback_len: usize,
    visible_lines: usize,
    matches: &[SearchMatch],
    current_match_idx: usize,
    highlight_color: [u8; 4],
    current_highlight_color: [u8; 4],
) {
    if matches.is_empty() {
        return;
    }

    let total_lines = scrollback_len + visible_lines;
    let visible_end = total_lines.saturating_sub(scroll_offset);
    let visible_start = visible_end.saturating_sub(visible_lines);

    for (match_idx, search_match) in matches.iter().enumerate() {
        if search_match.line < visible_start || search_match.line >= visible_end {
            continue;
        }

        let viewport_row = search_match.line - visible_start;
        let color = if match_idx == current_match_idx {
            current_highlight_color
        } else {
            highlight_color
        };

        for offset in 0..search_match.length {
            let col = search_match.column + offset;
            if col >= cols {
                break;
            }
            let cell_idx = viewport_row * cols + col;
            if cell_idx < cells.len() {
                cells[cell_idx].bg_color = color;
            }
        }
    }
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

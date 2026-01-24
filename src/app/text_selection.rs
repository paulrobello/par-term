//! Text selection operations for WindowState.
//!
//! This module contains methods for selecting text in the terminal,
//! including word selection, line selection, and text extraction.

use crate::selection::{Selection, SelectionMode};

use super::window_state::WindowState;

impl WindowState {
    pub(crate) fn select_word_at(&mut self, col: usize, row: usize) {
        let tab = if let Some(t) = self.tab_manager.active_tab() {
            t
        } else {
            return;
        };

        let (cols, visible_cells, _scroll_offset) = if let Ok(term) = tab.terminal.try_lock() {
            let (cols, _rows) = term.dimensions();
            let scroll_offset = tab.scroll_state.offset;
            let visible_cells = term.get_cells_with_scrollback(scroll_offset, None, false, None);
            (cols, visible_cells, scroll_offset)
        } else {
            return;
        };

        if visible_cells.is_empty() || cols == 0 {
            return;
        }

        let cell_idx = row * cols + col;
        if cell_idx >= visible_cells.len() {
            return;
        }

        // Find word boundaries
        let mut start_col = col;
        let mut end_col = col;

        // Expand left
        for c in (0..col).rev() {
            let idx = row * cols + c;
            if idx >= visible_cells.len() {
                break;
            }
            let ch = visible_cells[idx].grapheme.chars().next().unwrap_or('\0');
            if ch.is_alphanumeric() || ch == '_' {
                start_col = c;
            } else {
                break;
            }
        }

        // Expand right
        for c in col..cols {
            let idx = row * cols + c;
            if idx >= visible_cells.len() {
                break;
            }
            let ch = visible_cells[idx].grapheme.chars().next().unwrap_or('\0');
            if ch.is_alphanumeric() || ch == '_' {
                end_col = c;
            } else {
                break;
            }
        }

        // Now update mouse state
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.mouse.selection = Some(Selection::new(
                (start_col, row),
                (end_col, row),
                SelectionMode::Normal,
            ));
        }
    }

    /// Select entire line at the given row (used for triple-click)
    pub(crate) fn select_line_at(&mut self, row: usize) {
        let cols = if let Some(tab) = self.tab_manager.active_tab() {
            if let Ok(term) = tab.terminal.try_lock() {
                let (cols, _rows) = term.dimensions();
                cols
            } else {
                return;
            }
        } else {
            return;
        };

        if cols == 0 {
            return;
        }

        // Store the row in start/end - Line mode uses rows only
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.mouse.selection = Some(Selection::new(
                (0, row),
                (cols.saturating_sub(1), row),
                SelectionMode::Line,
            ));
        }
    }

    /// Extend line selection to include rows from anchor to current row
    pub(crate) fn extend_line_selection(&mut self, current_row: usize) {
        // Get cols from terminal and click_position from mouse
        let (cols, anchor_row) = {
            let tab = if let Some(t) = self.tab_manager.active_tab() {
                t
            } else {
                return;
            };

            let cols = if let Ok(term) = tab.terminal.try_lock() {
                let (cols, _rows) = term.dimensions();
                if cols == 0 {
                    return;
                }
                cols
            } else {
                return;
            };

            // Use click_position as the anchor row (the originally triple-clicked row)
            let anchor_row = tab
                .mouse
                .click_position
                .map(|(_, r)| r)
                .unwrap_or(current_row);
            (cols, anchor_row)
        };

        // Now update mouse selection
        if let Some(tab) = self.tab_manager.active_tab_mut()
            && let Some(ref mut selection) = tab.mouse.selection
            && selection.mode == SelectionMode::Line
        {
            // For line selection, always ensure full lines are selected
            // by setting columns appropriately based on drag direction
            if current_row >= anchor_row {
                // Dragging down or same row: start at col 0, end at last col
                selection.start = (0, anchor_row);
                selection.end = (cols.saturating_sub(1), current_row);
            } else {
                // Dragging up: start at last col (anchor row), end at col 0 (current row)
                // After normalization, this becomes: start=(0, current_row), end=(cols-1, anchor_row)
                selection.start = (cols.saturating_sub(1), anchor_row);
                selection.end = (0, current_row);
            }
        }
    }

    /// Extract selected text from terminal
    pub(crate) fn get_selected_text(&self) -> Option<String> {
        let tab = self.tab_manager.active_tab()?;
        let selection = tab.mouse.selection.as_ref()?;

        let term = tab.terminal.try_lock().ok()?;
        let (start, end) = selection.normalized();
        let (start_col, start_row) = start;
        let (end_col, end_row) = end;

        let (cols, rows) = term.dimensions();
        let visible_cells =
            term.get_cells_with_scrollback(tab.scroll_state.offset, None, false, None);
        if visible_cells.is_empty() || cols == 0 {
            return None;
        }

        let mut visible_lines = Vec::with_capacity(rows);
        for row in 0..rows {
            let start_idx = row * cols;
            let end_idx = start_idx.saturating_add(cols);
            if end_idx > visible_cells.len() {
                break;
            }

            let mut line = String::with_capacity(cols);
            for cell in &visible_cells[start_idx..end_idx] {
                line.push_str(&cell.grapheme);
            }
            visible_lines.push(line);
        }

        if visible_lines.is_empty() {
            return None;
        }

        let mut selected_text = String::new();
        let max_row = visible_lines.len().saturating_sub(1);
        let start_row = start_row.min(max_row);
        let end_row = end_row.min(max_row);

        if selection.mode == SelectionMode::Line {
            // Line selection: extract full lines
            #[allow(clippy::needless_range_loop)]
            for row in start_row..=end_row {
                if row > start_row {
                    selected_text.push('\n');
                }
                let line = &visible_lines[row];
                // Trim trailing spaces from each line but keep the content
                selected_text.push_str(line.trim_end());
            }
        } else if selection.mode == SelectionMode::Rectangular {
            // Rectangular selection: extract same columns from each row
            let min_col = start_col.min(end_col);
            let max_col = start_col.max(end_col);

            #[allow(clippy::needless_range_loop)]
            for row in start_row..=end_row {
                if row > start_row {
                    selected_text.push('\n');
                }
                let line = &visible_lines[row];
                selected_text.push_str(&Self::extract_columns(line, min_col, Some(max_col)));
            }
        } else if start_row == end_row {
            // Normal single-line selection
            let line = &visible_lines[start_row];
            selected_text = Self::extract_columns(line, start_col, Some(end_col));
        } else {
            // Normal multi-line selection
            for (idx, row) in (start_row..=end_row).enumerate() {
                let line = &visible_lines[row];
                if idx == 0 {
                    selected_text.push_str(&Self::extract_columns(line, start_col, None));
                } else if row == end_row {
                    selected_text.push('\n');
                    selected_text.push_str(&Self::extract_columns(line, 0, Some(end_col)));
                } else {
                    selected_text.push('\n');
                    selected_text.push_str(line);
                }
            }
        }

        Some(selected_text)
    }
}

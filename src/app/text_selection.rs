//! Text selection operations for WindowState.
//!
//! This module contains methods for selecting text in the terminal,
//! including word selection, line selection, and text extraction.
//!
//! Supports:
//! - Smart selection: Regex-based patterns (URLs, emails, paths) checked first
//! - Configurable word characters: User-defined characters considered part of a word

use crate::selection::{Selection, SelectionMode};
use crate::smart_selection::{find_word_boundaries, is_word_char};

use super::window_state::WindowState;

impl WindowState {
    /// Select word at the given position using smart selection and word boundary rules.
    ///
    /// Selection priority:
    /// 1. If smart_selection_enabled, try pattern-based selection (URLs, emails, etc.)
    /// 2. Fall back to word boundary selection using configurable word_characters
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

        // Build the line string for this row
        let row_start_idx = row * cols;
        let row_end_idx = (row_start_idx + cols).min(visible_cells.len());
        let line: String = visible_cells[row_start_idx..row_end_idx]
            .iter()
            .map(|c| c.grapheme.as_str())
            .collect();

        // Get config values
        let smart_selection_enabled = self.config.smart_selection_enabled;
        let word_characters = self.config.word_characters.clone();
        let smart_selection_rules = self.config.smart_selection_rules.clone();

        // Try smart selection first if enabled
        let (start_col, end_col) = if smart_selection_enabled {
            // Get or create the smart selection matcher
            let matcher = self
                .smart_selection_cache
                .get_matcher(&smart_selection_rules);

            if let Some((start, end)) = matcher.find_match_at(&line, col) {
                (start, end)
            } else {
                // Fall back to word boundary selection
                find_word_boundaries(&line, col, &word_characters)
            }
        } else {
            // Smart selection disabled, use word boundary selection
            find_word_boundaries(&line, col, &word_characters)
        };

        // Now update mouse state
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.mouse.selection = Some(Selection::new(
                (start_col, row),
                (end_col, row),
                SelectionMode::Normal,
            ));
        }
    }

    /// Select word at position using only word boundary selection (no smart patterns).
    /// This is useful for manual word selection that should ignore smart patterns.
    #[allow(dead_code)]
    pub(crate) fn select_word_at_simple(&mut self, col: usize, row: usize) {
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

        // Get word characters from config
        let word_characters = &self.config.word_characters;

        // Find word boundaries using configurable word characters
        let mut start_col = col;
        let mut end_col = col;

        // Expand left
        for c in (0..col).rev() {
            let idx = row * cols + c;
            if idx >= visible_cells.len() {
                break;
            }
            let ch = visible_cells[idx].grapheme.chars().next().unwrap_or('\0');
            if is_word_char(ch, word_characters) {
                start_col = c;
            } else {
                break;
            }
        }

        // Check if clicked position is a word character
        let clicked_char = visible_cells[cell_idx]
            .grapheme
            .chars()
            .next()
            .unwrap_or('\0');
        if !is_word_char(clicked_char, word_characters) {
            // Not a word character, select just this cell
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.mouse.selection = Some(Selection::new(
                    (col, row),
                    (col, row),
                    SelectionMode::Normal,
                ));
            }
            return;
        }

        // Expand right
        for c in col..cols {
            let idx = row * cols + c;
            if idx >= visible_cells.len() {
                break;
            }
            let ch = visible_cells[idx].grapheme.chars().next().unwrap_or('\0');
            if is_word_char(ch, word_characters) {
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

    /// Extract selected text and normalize it for clipboard copy operations.
    ///
    /// Applies the `copy_trailing_newline` setting and drops selections that become
    /// empty after normalization to avoid clobbering an existing clipboard payload
    /// (for example an image clipboard) with an empty text write.
    pub(crate) fn get_selected_text_for_copy(&self) -> Option<String> {
        let mut selected_text = self.get_selected_text()?;
        if selected_text.is_empty() {
            return None;
        }

        // Inverted config logic: false means strip trailing line endings.
        if !self.config.copy_trailing_newline {
            while selected_text.ends_with('\n') || selected_text.ends_with('\r') {
                selected_text.pop();
            }
        }

        if selected_text.is_empty() {
            log::debug!(
                "Skipping clipboard copy: selection became empty after newline normalization"
            );
            return None;
        }

        Some(selected_text)
    }
}

//! Text selection operations for WindowState.
//!
//! This module contains methods for selecting text in the terminal,
//! including word selection, line selection, and text extraction.
//!
//! Supports:
//! - Smart selection: Regex-based patterns (URLs, emails, paths) checked first
//! - Configurable word characters: User-defined characters considered part of a word

use crate::selection::{Selection, SelectionMode};
use crate::smart_selection::find_word_boundaries;
use crate::terminal::TerminalManager;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::window_state::WindowState;

impl WindowState {
    /// Get the terminal and scroll offset for text selection operations.
    ///
    /// In split-pane mode, returns the focused pane's terminal and scroll offset.
    /// Otherwise, returns the tab's terminal and scroll offset.
    fn selection_terminal_and_offset(&self) -> Option<(Arc<RwLock<TerminalManager>>, usize)> {
        let tab = self.tab_manager.active_tab()?;

        if let Some(ref pm) = tab.pane_manager
            && let Some(focused_pane) = pm.focused_pane()
        {
            Some((
                Arc::clone(&focused_pane.terminal),
                focused_pane.scroll_state.offset,
            ))
        } else {
            Some((Arc::clone(&tab.terminal), tab.active_scroll_state().offset))
        }
    }

    /// Select word at the given position using smart selection and word boundary rules.
    ///
    /// Selection priority:
    /// 1. If smart_selection_enabled, try pattern-based selection (URLs, emails, etc.)
    /// 2. Fall back to word boundary selection using configurable word_characters
    pub(crate) fn select_word_at(&mut self, col: usize, row: usize) {
        let (terminal_arc, scroll_offset) = if let Some(v) = self.selection_terminal_and_offset() {
            v
        } else {
            return;
        };

        // blocking_write: user-initiated double-click selection — must succeed
        // for the word to be highlighted.
        let term = terminal_arc.blocking_write();
        let (cols, _rows) = term.dimensions();
        let visible_cells = term.get_cells_with_scrollback(scroll_offset, None, false, None);
        drop(term); // Release lock before accessing self fields

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

        // Now update per-pane selection state
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.selection_mouse_mut().selection = Some(Selection::new(
                (start_col, row),
                (end_col, row),
                SelectionMode::Normal,
            ));
        }
    }

    /// Select entire line at the given row (used for triple-click)
    pub(crate) fn select_line_at(&mut self, row: usize) {
        let (terminal_arc, _scroll_offset) = if let Some(v) = self.selection_terminal_and_offset() {
            v
        } else {
            return;
        };

        // blocking_write: user-initiated triple-click selection — must succeed
        // for the line to be highlighted.
        let term = terminal_arc.blocking_write();
        let (cols, _rows) = term.dimensions();
        drop(term);

        if cols == 0 {
            return;
        }

        // Store the row in start/end - Line mode uses rows only
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.selection_mouse_mut().selection = Some(Selection::new(
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
            let (terminal_arc, _scroll_offset) =
                if let Some(v) = self.selection_terminal_and_offset() {
                    v
                } else {
                    return;
                };

            // try_write: intentional — triple-click drag extension runs on every
            // mouse-move frame. On miss: selection is not extended this frame;
            // the user sees a brief lag. High-frequency; acceptable loss.
            let cols = if let Ok(term) = terminal_arc.try_write() {
                let (cols, _rows) = term.dimensions();
                if cols == 0 {
                    return;
                }
                cols
            } else {
                return;
            };

            // Use click_position as the anchor row (the originally triple-clicked row)
            let anchor_row = self
                .tab_manager
                .active_tab()
                .and_then(|t| t.selection_mouse().click_position)
                .map(|(_, r)| r)
                .unwrap_or(current_row);
            (cols, anchor_row)
        };

        // Now update per-pane selection
        if let Some(tab) = self.tab_manager.active_tab_mut()
            && let Some(ref mut selection) = tab.selection_mouse_mut().selection
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

    /// Extract selected text from terminal.
    ///
    /// Uses `blocking_write()` because this is called on mouse release (user-initiated)
    /// and must succeed to copy the selection to the clipboard. In split-pane mode,
    /// reads from the focused pane's terminal rather than the tab's gateway terminal.
    pub(crate) fn get_selected_text(&self) -> Option<String> {
        let tab = self.tab_manager.active_tab()?;
        let selection = tab.selection_mouse().selection.as_ref()?;

        // Get the correct terminal and scroll offset (pane-aware)
        let (terminal_arc, scroll_offset) = self.selection_terminal_and_offset()?;

        // blocking_write: user-initiated copy on mouse release — must succeed to
        // avoid silently dropping the selection. This is an infrequent operation
        // (once per mouse release) so the brief lock wait is acceptable.
        let term = terminal_arc.blocking_write();
        let (start, end) = selection.normalized();
        let (start_col, start_row) = start;
        let (end_col, end_row) = end;

        let (cols, rows) = term.dimensions();
        let visible_cells = term.get_cells_with_scrollback(scroll_offset, None, false, None);
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

    /// Get copy text from the prettifier pipeline if the selection overlaps a prettified block.
    ///
    /// Returns the rendered or source text from the block based on the clipboard config's
    /// `default_copy` setting. Returns `None` if no prettifier is active or the selection
    /// doesn't overlap a prettified block.
    pub(crate) fn get_prettifier_copy_text(&self) -> Option<String> {
        let tab = self.tab_manager.active_tab()?;
        let pipeline = tab.prettifier.as_ref()?;
        if !pipeline.is_enabled() {
            return None;
        }
        let selection = tab.selection_mouse().selection.as_ref()?;
        let (start, _end) = selection.normalized();
        let start_row = start.1 + tab.active_scroll_state().offset;

        let block = pipeline.block_at_row(start_row)?;

        // Use the clipboard default_copy config to decide what to return.
        let default_copy = &self.config.content_prettifier.clipboard.default_copy;
        if default_copy == "source" {
            Some(block.buffer.source_text())
        } else {
            // "rendered" (default): prefer rendered text, fall back to source.
            block
                .buffer
                .rendered_text()
                .or_else(|| Some(block.buffer.source_text()))
        }
    }
}

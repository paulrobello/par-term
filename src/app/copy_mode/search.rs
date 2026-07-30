//! Copy mode search helpers for WindowState.
//!
//! Extracted from `copy_mode_handler` to keep that file focused on key dispatch.
//! Contains:
//! - `handle_copy_mode_search_key` — search input mode key handling
//! - `execute_copy_mode_search` — search execution (forward/backward)
//! - `search_lines_forward` / `search_lines_backward` — line scanning helpers
//! - `get_copy_mode_line_text` — line text accessor
//! - `after_copy_mode_motion` — post-motion housekeeping
//! - `sync_copy_mode_selection` — selection synchronization
//! - `follow_copy_mode_cursor` — viewport scrolling to follow cursor
//! - `yank_copy_mode_selection` — clipboard yank

use crate::app::window_state::WindowState;
use crate::copy_mode::SearchDirection;
use par_term_config::text::{
    byte_offset_to_column, column_to_byte_offset, lowercase_with_source_map,
};
use winit::event::KeyEvent;
use winit::keyboard::{Key, NamedKey};

/// Case-insensitive substring search over `text`, restricted to matches that
/// start at or after character `from_char`.
///
/// Returns the character index in `text` (not a byte offset) where the match
/// starts, so the result can be used directly as a copy-mode cursor column —
/// the same character indexing `Grid::row_text` feeds the motion helpers.
fn find_case_insensitive_forward(text: &str, query_lower: &str, from_char: usize) -> Option<usize> {
    let (lowered, sources) = lowercase_with_source_map(text);
    // Lowercasing can expand one character into several, so the search window
    // has to be located through the source map rather than by column arithmetic.
    let first = sources.partition_point(|&source| source < from_char);
    let window_start = column_to_byte_offset(&lowered, first);
    let found = lowered[window_start..].find(query_lower)?;
    let match_char = byte_offset_to_column(&lowered, window_start + found);
    sources.get(match_char).copied()
}

/// Case-insensitive substring search over `text` for the *last* match that
/// starts before character `before_char` (`None` searches the whole line).
///
/// Returns a character index in `text`, as [`find_case_insensitive_forward`] does.
fn find_case_insensitive_backward(
    text: &str,
    query_lower: &str,
    before_char: Option<usize>,
) -> Option<usize> {
    let (lowered, sources) = lowercase_with_source_map(text);
    let window_end = match before_char {
        Some(before) => {
            let last = sources.partition_point(|&source| source < before);
            column_to_byte_offset(&lowered, last)
        }
        None => lowered.len(),
    };
    let found = lowered[..window_end].rfind(query_lower)?;
    let match_char = byte_offset_to_column(&lowered, found);
    sources.get(match_char).copied()
}

impl WindowState {
    /// Handle key events during search input mode
    pub(crate) fn handle_copy_mode_search_key(&mut self, event: &KeyEvent) {
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                self.copy_mode.cancel_search();
                self.focus_state.needs_redraw = true;
                self.request_redraw();
            }
            Key::Named(NamedKey::Enter) => {
                self.copy_mode.is_searching = false;
                self.execute_copy_mode_search(false);
            }
            Key::Named(NamedKey::Backspace) => {
                self.copy_mode.search_backspace();
                self.focus_state.needs_redraw = true;
                self.request_redraw();
            }
            Key::Character(ch) => {
                for c in ch.chars() {
                    self.copy_mode.search_input(c);
                }
                self.focus_state.needs_redraw = true;
                self.request_redraw();
            }
            _ => {}
        }
    }

    /// Execute search in the current direction (or reversed if `reverse` is true)
    pub(crate) fn execute_copy_mode_search(&mut self, reverse: bool) {
        if self.copy_mode.search_query.is_empty() {
            return;
        }

        let query = self.copy_mode.search_query.clone();
        let forward = match self.copy_mode.search_direction {
            SearchDirection::Forward => !reverse,
            SearchDirection::Backward => reverse,
        };

        let current_line = self.copy_mode.cursor_absolute_line;
        let current_col = self.copy_mode.cursor_col;

        // Get all lines from terminal for searching
        // try_lock: intentional — copy mode search in sync event loop.
        // On miss: search is skipped this keypress; result stays at current position.
        let total = self.copy_mode.scrollback_len + self.copy_mode.rows;
        let found = self
            .tab_manager
            .active_tab()
            .and_then(|tab| {
                tab.try_with_terminal_mut(|term| {
                    if forward {
                        self.search_lines_forward(term, &query, current_line, current_col, total)
                    } else {
                        self.search_lines_backward(term, &query, current_line, current_col)
                    }
                })
            })
            .flatten();

        if let Some((line, col)) = found {
            self.copy_mode.cursor_absolute_line = line;
            self.copy_mode.cursor_col = col.min(self.copy_mode.cols.saturating_sub(1));
            self.after_copy_mode_motion();
            crate::debug_info!("COPY_MODE", "Search found '{}' at {}:{}", query, line, col);
        } else {
            self.show_toast("Pattern not found");
            self.focus_state.needs_redraw = true;
            self.request_redraw();
        }
    }

    /// Search forward through lines for a query string
    pub(crate) fn search_lines_forward(
        &self,
        term: &par_term_terminal::TerminalManager,
        query: &str,
        start_line: usize,
        start_col: usize,
        total_lines: usize,
    ) -> Option<(usize, usize)> {
        let query_lower = lowercase_with_source_map(query).0;

        // Search from current position to end
        for abs_line in start_line..total_lines {
            if let Some(text) = term.line_text_at_absolute(abs_line) {
                let from_char = if abs_line == start_line {
                    start_col + 1
                } else {
                    0
                };
                if let Some(col) = find_case_insensitive_forward(&text, &query_lower, from_char) {
                    return Some((abs_line, col));
                }
            }
        }
        // Wrap around from beginning
        for abs_line in 0..start_line {
            if let Some(text) = term.line_text_at_absolute(abs_line)
                && let Some(col) = find_case_insensitive_forward(&text, &query_lower, 0)
            {
                return Some((abs_line, col));
            }
        }
        None
    }

    /// Search backward through lines for a query string
    pub(crate) fn search_lines_backward(
        &self,
        term: &par_term_terminal::TerminalManager,
        query: &str,
        start_line: usize,
        start_col: usize,
    ) -> Option<(usize, usize)> {
        let query_lower = lowercase_with_source_map(query).0;

        // Search from current position to beginning
        for abs_line in (0..=start_line).rev() {
            if let Some(text) = term.line_text_at_absolute(abs_line) {
                let before_char = (abs_line == start_line).then_some(start_col);
                if let Some(col) = find_case_insensitive_backward(&text, &query_lower, before_char)
                {
                    return Some((abs_line, col));
                }
            }
        }
        // Wrap around from end
        let total = self.copy_mode.scrollback_len + self.copy_mode.rows;
        for abs_line in (start_line + 1..total).rev() {
            if let Some(text) = term.line_text_at_absolute(abs_line)
                && let Some(col) = find_case_insensitive_backward(&text, &query_lower, None)
            {
                return Some((abs_line, col));
            }
        }
        None
    }

    /// Get the text of the line at the copy mode cursor's current absolute line
    pub(crate) fn get_copy_mode_line_text(&self) -> Option<String> {
        let abs_line = self.copy_mode.cursor_absolute_line;
        // try_lock: intentional — reading line text for copy mode in sync event loop.
        // On miss: returns None (no text). The line action (yank/open) is skipped.
        self.tab_manager
            .active_tab()
            .and_then(|tab| tab.try_with_terminal_mut(|term| term.line_text_at_absolute(abs_line)))
            .flatten()
    }

    /// Post-motion housekeeping: sync selection, follow cursor, redraw
    pub(crate) fn after_copy_mode_motion(&mut self) {
        // Handle pending yank operator (e.g. yw, yj, etc.)
        if self.copy_mode.pending_operator.is_some() {
            // For pending yank, we need a visual selection first
            // Simple approach: just clear the pending operator
            // Full vi would create a temporary selection, but that's complex
            self.copy_mode.pending_operator = None;
        }

        self.sync_copy_mode_selection();
        self.follow_copy_mode_cursor();
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Synchronize the copy mode visual selection with the tab's mouse selection
    pub(crate) fn sync_copy_mode_selection(&mut self) {
        let scroll_offset = self
            .with_active_tab(|t| t.active_scroll_state().offset)
            .unwrap_or(0);

        let selection = self.copy_mode.compute_selection(scroll_offset);

        self.with_active_tab_mut(|tab| {
            tab.selection_mouse_mut().selection = selection;
            tab.active_cache_mut().cells = None; // Invalidate cache to re-render selection
        });
    }

    /// Scroll the viewport to follow the copy mode cursor if it moved offscreen
    pub(crate) fn follow_copy_mode_cursor(&mut self) {
        let current_offset = self
            .with_active_tab(|t| t.active_scroll_state().offset)
            .unwrap_or(0);

        if let Some(new_offset) = self.copy_mode.required_scroll_offset(current_offset) {
            self.set_scroll_target(new_offset);
            // After scrolling, re-sync selection coordinates
            self.sync_copy_mode_selection();
        }
    }

    /// Yank the current visual selection to clipboard, optionally exiting copy mode
    pub(crate) fn yank_copy_mode_selection(&mut self) {
        if let Some(text) = self.get_selected_text_for_copy() {
            let text_len = text.len();
            let auto_exit = self.config.load().copy_mode.copy_mode_auto_exit_on_yank;
            match self.input_handler.copy_to_clipboard(&text) {
                Ok(()) => {
                    let line_count = text.lines().count();
                    let msg = if line_count > 1 {
                        format!("{} lines yanked", line_count)
                    } else {
                        format!("{} chars yanked", text_len)
                    };
                    if auto_exit {
                        self.exit_copy_mode();
                    } else {
                        // Stay in copy mode but clear visual selection
                        self.copy_mode.visual_mode = crate::copy_mode::VisualMode::None;
                        self.copy_mode.selection_anchor = None;
                        self.with_active_tab_mut(|tab| {
                            tab.selection_mouse_mut().selection = None;
                            tab.active_cache_mut().cells = None;
                        });
                        self.focus_state.needs_redraw = true;
                        self.request_redraw();
                    }
                    self.show_toast(msg);
                }
                Err(e) => {
                    crate::debug_error!("COPY_MODE", "Failed to copy to clipboard: {}", e);
                    self.show_toast("Failed to copy to clipboard");
                }
            }
        } else if self.config.load().copy_mode.copy_mode_auto_exit_on_yank {
            self.exit_copy_mode();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{find_case_insensitive_backward, find_case_insensitive_forward};
    use par_term_config::text::lowercase_with_source_map;

    const ACCENTED: &str = "un café au lait";
    const CJK: &str = "日本語 test 日本語";
    const EMOJI: &str = "😀 alpha 😁 beta";

    fn lower(query: &str) -> String {
        lowercase_with_source_map(query).0
    }

    /// Every returned column must address the intended character of the
    /// original line under the same char indexing the copy-mode motions use.
    fn assert_column_addresses(text: &str, col: usize, expected: char) {
        assert_eq!(
            text.chars().nth(col),
            Some(expected),
            "column {col} of {text:?}"
        );
    }

    #[test]
    fn forward_returns_a_column_not_a_byte_offset() {
        let col = find_case_insensitive_forward(ACCENTED, &lower("au"), 0)
            .expect("`au` occurs in the line");
        assert_eq!(col, 8);
        assert_column_addresses(ACCENTED, col, 'a');

        let col = find_case_insensitive_forward(CJK, &lower("TEST"), 0)
            .expect("`test` occurs in the line");
        assert_eq!(col, 4);
        assert_column_addresses(CJK, col, 't');

        let col = find_case_insensitive_forward(EMOJI, &lower("beta"), 0)
            .expect("`beta` occurs in the line");
        assert_eq!(col, 10);
        assert_column_addresses(EMOJI, col, 'b');
    }

    #[test]
    fn forward_start_column_is_a_column() {
        // Starting past the first "日本語" must find the second one, and the
        // start column must not be mistaken for a byte offset (which would be
        // 3x larger here and skip the whole line).
        let col = find_case_insensitive_forward(CJK, &lower("日本語"), 1).expect("second match");
        assert_eq!(col, 9);
        assert_column_addresses(CJK, col, '日');
        assert_eq!(
            find_case_insensitive_forward(CJK, &lower("日本語"), 10),
            None
        );
    }

    #[test]
    fn backward_finds_the_last_match_before_the_cursor() {
        let col = find_case_insensitive_backward(CJK, &lower("日本語"), Some(9))
            .expect("first match precedes column 9");
        assert_eq!(col, 0);

        let col = find_case_insensitive_backward(CJK, &lower("日本語"), None).expect("last match");
        assert_eq!(col, 9);

        // A match that straddles the cursor column must not be returned.
        assert_eq!(
            find_case_insensitive_backward(ACCENTED, &lower("café"), Some(5)),
            None
        );
        assert_eq!(
            find_case_insensitive_backward(ACCENTED, &lower("café"), Some(7)),
            Some(3)
        );
    }

    #[test]
    fn out_of_range_start_column_is_saturating() {
        assert_eq!(
            find_case_insensitive_forward(CJK, &lower("test"), 9_999),
            None
        );
        assert_eq!(
            find_case_insensitive_backward(CJK, &lower("test"), Some(9_999)),
            Some(4)
        );
        assert_eq!(find_case_insensitive_forward("", &lower("x"), 3), None);
    }

    #[test]
    fn expanding_lowercase_still_round_trips_to_the_source_column() {
        // `İ` (U+0130) lowercases to two characters, so a position in the
        // lowercased line is not a position in the line.
        let text = "aİb";
        assert_eq!(text.chars().count(), 3);
        assert_eq!(lower(text).chars().count(), 4);

        let col = find_case_insensitive_forward(text, &lower("b"), 0).expect("`b` occurs");
        assert_eq!(col, 2);
        assert_column_addresses(text, col, 'b');

        let col = find_case_insensitive_backward(text, &lower("b"), None).expect("`b` occurs");
        assert_eq!(col, 2);
        assert_column_addresses(text, col, 'b');

        // The expanding character itself maps back to its single source column.
        let col = find_case_insensitive_forward(text, &lower("İ"), 0).expect("`İ` occurs");
        assert_eq!(col, 1);
        assert_column_addresses(text, col, 'İ');
    }

    #[test]
    fn ascii_results_match_the_previous_byte_offset_behavior() {
        let text = "Hello World hello";
        let query = lower("hello");
        for from_char in 0..text.len() {
            let expected = text.to_lowercase()[from_char..]
                .find(&query)
                .map(|pos| from_char + pos);
            assert_eq!(
                find_case_insensitive_forward(text, &query, from_char),
                expected,
                "from {from_char}"
            );
        }
        for before_char in 0..=text.len() {
            let expected = text.to_lowercase()[..before_char].rfind(&query);
            assert_eq!(
                find_case_insensitive_backward(text, &query, Some(before_char)),
                expected,
                "before {before_char}"
            );
        }
    }
}

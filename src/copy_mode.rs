//! Vi-style Copy Mode state machine.
//!
//! Copy Mode provides keyboard-driven text selection and navigation,
//! matching iTerm2's Copy Mode. When active, all keyboard input navigates
//! an independent cursor through the terminal buffer (including scrollback).

use crate::selection::{Selection, SelectionMode};
use crate::smart_selection::is_word_char;
use std::collections::HashMap;

/// Visual selection mode in copy mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualMode {
    /// No visual selection active
    None,
    /// Character-wise selection (v)
    Char,
    /// Line-wise selection (V)
    Line,
    /// Block/rectangular selection (Ctrl+V)
    Block,
}

/// Pending operator waiting for a motion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingOperator {
    /// Yank (copy) operator
    Yank,
}

/// Search direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDirection {
    Forward,
    Backward,
}

/// A named mark position
#[derive(Debug, Clone, Copy)]
pub struct Mark {
    pub col: usize,
    pub absolute_line: usize,
}

/// Copy mode state machine.
///
/// Uses absolute line indexing:
/// - Line 0 = oldest scrollback line
/// - Line `scrollback_len - 1` = newest scrollback line
/// - Line `scrollback_len` = top of visible screen (at scroll_offset=0)
/// - Line `scrollback_len + rows - 1` = bottom of visible screen
pub struct CopyModeState {
    /// Whether copy mode is active
    pub active: bool,
    /// Cursor column position
    pub cursor_col: usize,
    /// Cursor absolute line position
    pub cursor_absolute_line: usize,
    /// Current visual selection mode
    pub visual_mode: VisualMode,
    /// Selection anchor point (absolute_line, col) - set when entering visual mode
    pub selection_anchor: Option<(usize, usize)>,
    /// Count prefix for motions (e.g., 5j moves down 5 lines)
    pub count: Option<usize>,
    /// Pending operator waiting for a motion
    pub pending_operator: Option<PendingOperator>,
    /// Named marks (a-z)
    pub marks: HashMap<char, Mark>,
    /// Terminal columns
    pub cols: usize,
    /// Terminal rows
    pub rows: usize,
    /// Scrollback buffer length
    pub scrollback_len: usize,
    /// Current search query
    pub search_query: String,
    /// Search direction
    pub search_direction: SearchDirection,
    /// Whether search input mode is active
    pub is_searching: bool,
    /// Waiting for second 'g' in 'gg'
    pub(crate) pending_g: bool,
    /// Waiting for mark name after 'm'
    pub(crate) pending_mark_set: bool,
    /// Waiting for mark name after "'"
    pub(crate) pending_mark_goto: bool,
}

impl Default for CopyModeState {
    fn default() -> Self {
        Self::new()
    }
}

impl CopyModeState {
    /// Create a new inactive copy mode state
    pub fn new() -> Self {
        Self {
            active: false,
            cursor_col: 0,
            cursor_absolute_line: 0,
            visual_mode: VisualMode::None,
            selection_anchor: None,
            count: None,
            pending_operator: None,
            marks: HashMap::new(),
            cols: 80,
            rows: 24,
            scrollback_len: 0,
            search_query: String::new(),
            search_direction: SearchDirection::Forward,
            is_searching: false,
            pending_g: false,
            pending_mark_set: false,
            pending_mark_goto: false,
        }
    }

    /// Enter copy mode at the given cursor position
    pub fn enter(
        &mut self,
        cursor_col: usize,
        cursor_row: usize,
        cols: usize,
        rows: usize,
        scrollback_len: usize,
    ) {
        self.active = true;
        self.cols = cols;
        self.rows = rows;
        self.scrollback_len = scrollback_len;
        // Convert screen row to absolute line
        self.cursor_absolute_line = scrollback_len + cursor_row;
        self.cursor_col = cursor_col.min(cols.saturating_sub(1));
        self.visual_mode = VisualMode::None;
        self.selection_anchor = None;
        self.count = None;
        self.pending_operator = None;
        self.search_query.clear();
        self.is_searching = false;
        self.pending_g = false;
        self.pending_mark_set = false;
        self.pending_mark_goto = false;
    }

    /// Exit copy mode, clearing all state
    pub fn exit(&mut self) {
        self.active = false;
        self.visual_mode = VisualMode::None;
        self.selection_anchor = None;
        self.count = None;
        self.pending_operator = None;
        self.is_searching = false;
        self.pending_g = false;
        self.pending_mark_set = false;
        self.pending_mark_goto = false;
    }

    // ========================================================================
    // Count prefix
    // ========================================================================

    /// Push a digit to the count prefix
    pub fn push_count_digit(&mut self, digit: u8) {
        let current = self.count.unwrap_or(0);
        self.count = Some(current * 10 + digit as usize);
    }

    /// Get the effective count (defaults to 1 if no count set)
    pub fn effective_count(&mut self) -> usize {
        let c = self.count.unwrap_or(1);
        self.count = None;
        c
    }

    /// Total number of lines (scrollback + screen)
    fn total_lines(&self) -> usize {
        self.scrollback_len + self.rows
    }

    /// Maximum valid absolute line index
    fn max_line(&self) -> usize {
        self.total_lines().saturating_sub(1)
    }

    // ========================================================================
    // Basic motions
    // ========================================================================

    /// Move cursor left by count
    pub fn move_left(&mut self) {
        let count = self.effective_count();
        self.cursor_col = self.cursor_col.saturating_sub(count);
    }

    /// Move cursor right by count
    pub fn move_right(&mut self) {
        let count = self.effective_count();
        self.cursor_col = (self.cursor_col + count).min(self.cols.saturating_sub(1));
    }

    /// Move cursor up by count
    pub fn move_up(&mut self) {
        let count = self.effective_count();
        self.cursor_absolute_line = self.cursor_absolute_line.saturating_sub(count);
    }

    /// Move cursor down by count
    pub fn move_down(&mut self) {
        let count = self.effective_count();
        self.cursor_absolute_line = (self.cursor_absolute_line + count).min(self.max_line());
    }

    /// Move cursor to start of line
    pub fn move_to_line_start(&mut self) {
        self.cursor_col = 0;
    }

    /// Move cursor to end of line
    pub fn move_to_line_end(&mut self) {
        self.cursor_col = self.cols.saturating_sub(1);
    }

    /// Move cursor to first non-blank character on the line
    pub fn move_to_first_non_blank(&mut self, line_text: &str) {
        let first_non_blank = line_text
            .chars()
            .position(|c| !c.is_whitespace())
            .unwrap_or(0);
        self.cursor_col = first_non_blank.min(self.cols.saturating_sub(1));
    }

    // ========================================================================
    // Word motions
    // ========================================================================

    /// Move forward to start of next word
    pub fn move_word_forward(&mut self, line_text: &str, word_chars: &str) {
        let count = self.effective_count();
        let chars: Vec<char> = line_text.chars().collect();
        let mut col = self.cursor_col;

        for _ in 0..count {
            if col >= chars.len() {
                break;
            }
            // Skip current word characters
            while col < chars.len() && is_word_char(chars[col], word_chars) {
                col += 1;
            }
            // Skip non-word characters (whitespace/punctuation)
            while col < chars.len() && !is_word_char(chars[col], word_chars) {
                col += 1;
            }
        }

        self.cursor_col = col.min(self.cols.saturating_sub(1));
    }

    /// Move backward to start of previous word
    pub fn move_word_backward(&mut self, line_text: &str, word_chars: &str) {
        let count = self.effective_count();
        let chars: Vec<char> = line_text.chars().collect();
        let mut col = self.cursor_col;

        for _ in 0..count {
            if col == 0 {
                break;
            }
            col = col.saturating_sub(1);
            // Skip non-word characters backward
            while col > 0 && !is_word_char(chars[col], word_chars) {
                col -= 1;
            }
            // Skip word characters backward to find start
            while col > 0 && is_word_char(chars[col - 1], word_chars) {
                col -= 1;
            }
        }

        self.cursor_col = col;
    }

    /// Move forward to end of current/next word
    pub fn move_word_end(&mut self, line_text: &str, word_chars: &str) {
        let count = self.effective_count();
        let chars: Vec<char> = line_text.chars().collect();
        let mut col = self.cursor_col;

        for _ in 0..count {
            if col >= chars.len().saturating_sub(1) {
                break;
            }
            col += 1;
            // Skip non-word characters
            while col < chars.len() && !is_word_char(chars[col], word_chars) {
                col += 1;
            }
            // Move to end of word
            while col < chars.len().saturating_sub(1) && is_word_char(chars[col + 1], word_chars) {
                col += 1;
            }
        }

        self.cursor_col = col.min(self.cols.saturating_sub(1));
    }

    /// Move forward to start of next WORD (whitespace-delimited)
    pub fn move_big_word_forward(&mut self, line_text: &str) {
        let count = self.effective_count();
        let chars: Vec<char> = line_text.chars().collect();
        let mut col = self.cursor_col;

        for _ in 0..count {
            // Skip non-whitespace
            while col < chars.len() && !chars[col].is_whitespace() {
                col += 1;
            }
            // Skip whitespace
            while col < chars.len() && chars[col].is_whitespace() {
                col += 1;
            }
        }

        self.cursor_col = col.min(self.cols.saturating_sub(1));
    }

    /// Move backward to start of previous WORD (whitespace-delimited)
    pub fn move_big_word_backward(&mut self, line_text: &str) {
        let count = self.effective_count();
        let chars: Vec<char> = line_text.chars().collect();
        let mut col = self.cursor_col;

        for _ in 0..count {
            if col == 0 {
                break;
            }
            col = col.saturating_sub(1);
            // Skip whitespace backward
            while col > 0 && chars[col].is_whitespace() {
                col -= 1;
            }
            // Skip non-whitespace backward
            while col > 0 && !chars[col - 1].is_whitespace() {
                col -= 1;
            }
        }

        self.cursor_col = col;
    }

    /// Move forward to end of current/next WORD (whitespace-delimited)
    pub fn move_big_word_end(&mut self, line_text: &str) {
        let count = self.effective_count();
        let chars: Vec<char> = line_text.chars().collect();
        let mut col = self.cursor_col;

        for _ in 0..count {
            if col >= chars.len().saturating_sub(1) {
                break;
            }
            col += 1;
            // Skip whitespace
            while col < chars.len() && chars[col].is_whitespace() {
                col += 1;
            }
            // Move to end of WORD
            while col < chars.len().saturating_sub(1) && !chars[col + 1].is_whitespace() {
                col += 1;
            }
        }

        self.cursor_col = col.min(self.cols.saturating_sub(1));
    }

    // ========================================================================
    // Page motions
    // ========================================================================

    /// Move half page up
    pub fn half_page_up(&mut self) {
        let half = self.rows / 2;
        let count = self.effective_count();
        self.cursor_absolute_line = self.cursor_absolute_line.saturating_sub(half * count);
    }

    /// Move half page down
    pub fn half_page_down(&mut self) {
        let half = self.rows / 2;
        let count = self.effective_count();
        self.cursor_absolute_line = (self.cursor_absolute_line + half * count).min(self.max_line());
    }

    /// Move full page up
    pub fn page_up(&mut self) {
        let count = self.effective_count();
        self.cursor_absolute_line = self.cursor_absolute_line.saturating_sub(self.rows * count);
    }

    /// Move full page down
    pub fn page_down(&mut self) {
        let count = self.effective_count();
        self.cursor_absolute_line =
            (self.cursor_absolute_line + self.rows * count).min(self.max_line());
    }

    /// Go to top of buffer (line 0)
    pub fn goto_top(&mut self) {
        self.cursor_absolute_line = 0;
    }

    /// Go to bottom of buffer (last line)
    pub fn goto_bottom(&mut self) {
        self.cursor_absolute_line = self.max_line();
    }

    /// Go to specific absolute line (for count+G)
    pub fn goto_line(&mut self, line: usize) {
        self.cursor_absolute_line = line.min(self.max_line());
    }

    // ========================================================================
    // Visual mode
    // ========================================================================

    /// Toggle character-wise visual mode
    pub fn toggle_visual_char(&mut self) {
        if self.visual_mode == VisualMode::Char {
            self.visual_mode = VisualMode::None;
            self.selection_anchor = None;
        } else {
            self.visual_mode = VisualMode::Char;
            self.selection_anchor = Some((self.cursor_absolute_line, self.cursor_col));
        }
    }

    /// Toggle line-wise visual mode
    pub fn toggle_visual_line(&mut self) {
        if self.visual_mode == VisualMode::Line {
            self.visual_mode = VisualMode::None;
            self.selection_anchor = None;
        } else {
            self.visual_mode = VisualMode::Line;
            self.selection_anchor = Some((self.cursor_absolute_line, self.cursor_col));
        }
    }

    /// Toggle block/rectangular visual mode
    pub fn toggle_visual_block(&mut self) {
        if self.visual_mode == VisualMode::Block {
            self.visual_mode = VisualMode::None;
            self.selection_anchor = None;
        } else {
            self.visual_mode = VisualMode::Block;
            self.selection_anchor = Some((self.cursor_absolute_line, self.cursor_col));
        }
    }

    /// Compute a `Selection` from the current visual mode state.
    ///
    /// The selection coordinates are in screen-relative terms
    /// (row = line - viewport_top) for rendering.
    /// `scroll_offset` is the current viewport scroll position.
    pub fn compute_selection(&self, scroll_offset: usize) -> Option<Selection> {
        if self.visual_mode == VisualMode::None {
            return None;
        }

        let (anchor_line, anchor_col) = self.selection_anchor?;

        // Convert absolute lines to viewport-relative rows
        // viewport_top = scrollback_len - scroll_offset (absolute line at top of screen)
        let viewport_top = self.scrollback_len.saturating_sub(scroll_offset);

        // Both anchor and cursor must produce valid viewport rows for rendering
        // We allow negative (above screen) and beyond-screen positions
        // by clamping to 0..rows-1 for rendering purposes
        let anchor_row = anchor_line.saturating_sub(viewport_top);
        let cursor_row = self.cursor_absolute_line.saturating_sub(viewport_top);

        let mode = match self.visual_mode {
            VisualMode::None => return None,
            VisualMode::Char => SelectionMode::Normal,
            VisualMode::Line => SelectionMode::Line,
            VisualMode::Block => SelectionMode::Rectangular,
        };

        let start = (anchor_col, anchor_row);
        let end = (self.cursor_col, cursor_row);

        Some(Selection::new(start, end, mode))
    }

    // ========================================================================
    // Marks
    // ========================================================================

    /// Set a named mark at the current cursor position
    pub fn set_mark(&mut self, name: char) {
        self.marks.insert(
            name,
            Mark {
                col: self.cursor_col,
                absolute_line: self.cursor_absolute_line,
            },
        );
    }

    /// Jump to a named mark, returning true if the mark exists
    pub fn goto_mark(&mut self, name: char) -> bool {
        if let Some(mark) = self.marks.get(&name) {
            self.cursor_col = mark.col;
            self.cursor_absolute_line = mark.absolute_line;
            true
        } else {
            false
        }
    }

    // ========================================================================
    // Search
    // ========================================================================

    /// Start search input mode
    pub fn start_search(&mut self, direction: SearchDirection) {
        self.is_searching = true;
        self.search_direction = direction;
        self.search_query.clear();
    }

    /// Add a character to the search query
    pub fn search_input(&mut self, ch: char) {
        self.search_query.push(ch);
    }

    /// Remove the last character from the search query
    pub fn search_backspace(&mut self) {
        self.search_query.pop();
    }

    /// Cancel search mode without executing
    pub fn cancel_search(&mut self) {
        self.is_searching = false;
        self.search_query.clear();
    }

    // ========================================================================
    // Viewport
    // ========================================================================

    /// Get the cursor position in screen coordinates, if visible.
    ///
    /// Returns `(col, row)` where row is relative to the viewport top.
    /// Returns `None` if the cursor is outside the visible viewport.
    pub fn screen_cursor_pos(&self, scroll_offset: usize) -> Option<(usize, usize)> {
        let viewport_top = self.scrollback_len.saturating_sub(scroll_offset);
        let viewport_bottom = viewport_top + self.rows;

        if self.cursor_absolute_line >= viewport_top && self.cursor_absolute_line < viewport_bottom
        {
            let screen_row = self.cursor_absolute_line - viewport_top;
            Some((self.cursor_col, screen_row))
        } else {
            None
        }
    }

    /// Calculate the scroll offset needed to make the cursor visible.
    ///
    /// Returns `Some(new_offset)` if scrolling is needed, `None` if cursor is already visible.
    pub fn required_scroll_offset(&self, current_offset: usize) -> Option<usize> {
        let viewport_top = self.scrollback_len.saturating_sub(current_offset);
        let viewport_bottom = viewport_top + self.rows;

        if self.cursor_absolute_line < viewport_top {
            // Cursor is above viewport — scroll up
            let new_offset = self
                .scrollback_len
                .saturating_sub(self.cursor_absolute_line);
            Some(new_offset)
        } else if self.cursor_absolute_line >= viewport_bottom {
            // Cursor is below viewport — scroll down
            let lines_below = self.cursor_absolute_line - viewport_top;
            let needed_offset = current_offset
                .saturating_sub(lines_below.saturating_sub(self.rows.saturating_sub(1)));
            // Alternatively: the viewport top should be cursor_line - (rows - 1)
            let target_viewport_top = self
                .cursor_absolute_line
                .saturating_sub(self.rows.saturating_sub(1));
            let new_offset = self.scrollback_len.saturating_sub(target_viewport_top);
            // Clamp to valid range
            let _ = needed_offset; // suppress unused
            Some(new_offset.min(self.scrollback_len))
        } else {
            None
        }
    }

    /// Update the scrollback length (call when terminal state changes)
    pub fn update_dimensions(&mut self, cols: usize, rows: usize, scrollback_len: usize) {
        self.cols = cols;
        self.rows = rows;
        self.scrollback_len = scrollback_len;
        // Clamp cursor to valid range
        self.cursor_col = self.cursor_col.min(cols.saturating_sub(1));
        self.cursor_absolute_line = self.cursor_absolute_line.min(self.max_line());
    }

    /// Get a status line description of the current mode
    pub fn status_text(&self) -> String {
        if self.is_searching {
            let dir = match self.search_direction {
                SearchDirection::Forward => '/',
                SearchDirection::Backward => '?',
            };
            format!("{}{}", dir, self.search_query)
        } else {
            let mode = match self.visual_mode {
                VisualMode::None => "COPY",
                VisualMode::Char => "VISUAL",
                VisualMode::Line => "VISUAL LINE",
                VisualMode::Block => "VISUAL BLOCK",
            };
            let pos = format!(
                "{}:{} (abs {})",
                self.cursor_absolute_line
                    .saturating_sub(self.scrollback_len),
                self.cursor_col,
                self.cursor_absolute_line,
            );
            format!("-- {} -- {}", mode, pos)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enter_exit() {
        let mut cm = CopyModeState::new();
        assert!(!cm.active);

        cm.enter(5, 10, 80, 24, 100);
        assert!(cm.active);
        assert_eq!(cm.cursor_col, 5);
        assert_eq!(cm.cursor_absolute_line, 110); // scrollback(100) + row(10)
        assert_eq!(cm.cols, 80);
        assert_eq!(cm.rows, 24);

        cm.exit();
        assert!(!cm.active);
    }

    #[test]
    fn test_basic_motions() {
        let mut cm = CopyModeState::new();
        cm.enter(10, 5, 80, 24, 100);

        cm.move_left();
        assert_eq!(cm.cursor_col, 9);

        cm.move_right();
        assert_eq!(cm.cursor_col, 10);

        cm.move_up();
        assert_eq!(cm.cursor_absolute_line, 104);

        cm.move_down();
        assert_eq!(cm.cursor_absolute_line, 105);

        cm.move_to_line_start();
        assert_eq!(cm.cursor_col, 0);

        cm.move_to_line_end();
        assert_eq!(cm.cursor_col, 79);
    }

    #[test]
    fn test_count_prefix() {
        let mut cm = CopyModeState::new();
        cm.enter(10, 12, 80, 24, 100);

        cm.push_count_digit(5);
        cm.move_down();
        assert_eq!(cm.cursor_absolute_line, 117);
    }

    #[test]
    fn test_boundary_clamping() {
        let mut cm = CopyModeState::new();
        cm.enter(0, 0, 80, 24, 0);

        // Can't go above line 0
        cm.move_up();
        assert_eq!(cm.cursor_absolute_line, 0);

        // Can't go left of col 0
        cm.move_left();
        assert_eq!(cm.cursor_col, 0);

        // Can't go past max line
        cm.goto_bottom();
        assert_eq!(cm.cursor_absolute_line, 23);
        cm.move_down();
        assert_eq!(cm.cursor_absolute_line, 23);
    }

    #[test]
    fn test_visual_modes() {
        let mut cm = CopyModeState::new();
        cm.enter(5, 5, 80, 24, 100);

        // Enter char visual
        cm.toggle_visual_char();
        assert_eq!(cm.visual_mode, VisualMode::Char);
        assert!(cm.selection_anchor.is_some());

        // Toggle off
        cm.toggle_visual_char();
        assert_eq!(cm.visual_mode, VisualMode::None);
        assert!(cm.selection_anchor.is_none());

        // Enter line visual
        cm.toggle_visual_line();
        assert_eq!(cm.visual_mode, VisualMode::Line);

        // Switch to block visual
        cm.toggle_visual_block();
        assert_eq!(cm.visual_mode, VisualMode::Block);
    }

    #[test]
    fn test_screen_cursor_pos() {
        let mut cm = CopyModeState::new();
        cm.enter(5, 10, 80, 24, 100);
        // scroll_offset=0 means viewport starts at line 100

        // Cursor at absolute line 110, viewport top at 100
        assert_eq!(cm.screen_cursor_pos(0), Some((5, 10)));

        // Cursor above viewport
        cm.cursor_absolute_line = 50;
        assert_eq!(cm.screen_cursor_pos(0), None);

        // Scroll up to make it visible
        assert_eq!(cm.screen_cursor_pos(50), Some((5, 0)));
    }

    #[test]
    fn test_compute_selection() {
        let mut cm = CopyModeState::new();
        cm.enter(5, 5, 80, 24, 100);

        // No selection without visual mode
        assert!(cm.compute_selection(0).is_none());

        // Enter visual char mode
        cm.toggle_visual_char();
        cm.move_right();
        cm.move_right();
        cm.move_down();

        let sel = cm.compute_selection(0).unwrap();
        assert_eq!(sel.mode, SelectionMode::Normal);
        // Anchor at (5, 5), cursor at (7, 6)
        assert_eq!(sel.start, (5, 5));
        assert_eq!(sel.end, (7, 6));
    }

    #[test]
    fn test_marks() {
        let mut cm = CopyModeState::new();
        cm.enter(10, 5, 80, 24, 100);

        cm.set_mark('a');
        cm.move_down();
        cm.move_right();

        assert!(cm.goto_mark('a'));
        assert_eq!(cm.cursor_col, 10);
        assert_eq!(cm.cursor_absolute_line, 105);

        assert!(!cm.goto_mark('b')); // non-existent mark
    }

    #[test]
    fn test_word_motions() {
        let mut cm = CopyModeState::new();
        cm.enter(0, 0, 80, 24, 0);

        let line = "hello world foo";
        cm.move_word_forward(line, "");
        assert_eq!(cm.cursor_col, 6); // start of "world"

        cm.move_word_end(line, "");
        assert_eq!(cm.cursor_col, 10); // end of "world"

        cm.move_word_backward(line, "");
        assert_eq!(cm.cursor_col, 6); // back to start of "world"
    }

    #[test]
    fn test_page_motions() {
        let mut cm = CopyModeState::new();
        cm.enter(0, 12, 80, 24, 200);
        // Absolute line = 212

        cm.half_page_up();
        assert_eq!(cm.cursor_absolute_line, 200); // 212 - 12

        cm.page_down();
        assert_eq!(cm.cursor_absolute_line, 223); // max_line = 200+24-1 = 223

        cm.goto_top();
        assert_eq!(cm.cursor_absolute_line, 0);

        cm.goto_bottom();
        assert_eq!(cm.cursor_absolute_line, 223);
    }

    #[test]
    fn test_search_state() {
        let mut cm = CopyModeState::new();
        cm.enter(0, 0, 80, 24, 0);

        cm.start_search(SearchDirection::Forward);
        assert!(cm.is_searching);

        cm.search_input('h');
        cm.search_input('e');
        assert_eq!(cm.search_query, "he");

        cm.search_backspace();
        assert_eq!(cm.search_query, "h");

        cm.cancel_search();
        assert!(!cm.is_searching);
        assert!(cm.search_query.is_empty());
    }

    #[test]
    fn test_required_scroll_offset() {
        let mut cm = CopyModeState::new();
        cm.enter(0, 12, 80, 24, 100);
        // Cursor at line 112, viewport top at line 100 (offset=0)

        // Cursor is visible, no scroll needed
        assert_eq!(cm.required_scroll_offset(0), None);

        // Move cursor above viewport
        cm.cursor_absolute_line = 50;
        let offset = cm.required_scroll_offset(0).unwrap();
        assert_eq!(offset, 50); // scrollback_len - cursor_line = 100 - 50
    }
}

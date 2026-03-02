//! Type definitions for the vi-style copy mode state machine.

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

    /// Total number of lines (scrollback + screen)
    pub(super) fn total_lines(&self) -> usize {
        self.scrollback_len + self.rows
    }

    /// Maximum valid absolute line index
    pub(super) fn max_line(&self) -> usize {
        self.total_lines().saturating_sub(1)
    }

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

    /// Update the scrollback length (call when terminal state changes)
    pub fn update_dimensions(&mut self, cols: usize, rows: usize, scrollback_len: usize) {
        self.cols = cols;
        self.rows = rows;
        self.scrollback_len = scrollback_len;
        // Clamp cursor to valid range
        self.cursor_col = self.cursor_col.min(cols.saturating_sub(1));
        self.cursor_absolute_line = self.cursor_absolute_line.min(self.max_line());
    }
}

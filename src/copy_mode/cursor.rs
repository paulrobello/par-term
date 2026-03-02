//! Cursor movement methods for the copy mode state machine.

use super::types::CopyModeState;

impl CopyModeState {
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
    // Viewport helpers
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
}

//! Word and line navigation helpers for the copy mode state machine.

use super::types::CopyModeState;
use crate::smart_selection::is_word_char;

impl CopyModeState {
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
}

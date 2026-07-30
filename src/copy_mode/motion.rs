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
        // `cursor_col` is clamped to the grid width, which exceeds the character
        // count whenever the row holds wide characters — `Grid::row_text` drops
        // their spacer cells. `col` only decreases below, so this seed clamp is
        // what keeps every `chars[..]` access in bounds.
        let mut col = self.cursor_col.min(chars.len().saturating_sub(1));

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
        // Seed clamp keeps `chars[..]` in bounds; see `move_word_backward`.
        let mut col = self.cursor_col.min(chars.len().saturating_sub(1));

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

#[cfg(test)]
mod tests {
    use super::CopyModeState;

    /// A CJK row: `Grid::row_text` drops the wide-char spacer cells, so the
    /// text has far fewer characters than the grid has columns.
    const CJK_LINE: &str = "日本語 test";
    const ACCENTED_LINE: &str = "café au lait";
    const EMOJI_LINE: &str = "😀 😁 done";

    fn at_line_end(cols: usize) -> CopyModeState {
        let mut cm = CopyModeState::new();
        cm.enter(0, 0, cols, 24, 0);
        cm.move_to_line_end();
        cm
    }

    #[test]
    fn word_backward_from_line_end_on_non_ascii_line() {
        // `$` parks the cursor at column 79 while these lines hold at most a
        // dozen characters, which used to index the char vector out of bounds.
        for line in [CJK_LINE, ACCENTED_LINE, EMOJI_LINE] {
            let mut cm = at_line_end(80);
            cm.move_word_backward(line, "");
            assert!(
                cm.cursor_col < line.chars().count(),
                "{line:?} left cursor at {}",
                cm.cursor_col
            );

            let mut cm = at_line_end(80);
            cm.move_big_word_backward(line);
            assert!(
                cm.cursor_col < line.chars().count(),
                "{line:?} left cursor at {}",
                cm.cursor_col
            );
        }
    }

    #[test]
    fn word_backward_lands_on_last_word_start() {
        let mut cm = at_line_end(80);
        cm.move_word_backward(CJK_LINE, "");
        // "日本語 test" — characters 4..8 are "test".
        assert_eq!(cm.cursor_col, 4);

        let mut cm = at_line_end(80);
        cm.move_big_word_backward(ACCENTED_LINE);
        // "café au lait" — characters 8..12 are "lait".
        assert_eq!(cm.cursor_col, 8);
    }

    #[test]
    fn word_backward_handles_empty_and_short_lines() {
        let mut cm = at_line_end(80);
        cm.move_word_backward("", "");
        assert_eq!(cm.cursor_col, 0);

        let mut cm = at_line_end(80);
        cm.move_big_word_backward("é");
        assert_eq!(cm.cursor_col, 0);
    }
}

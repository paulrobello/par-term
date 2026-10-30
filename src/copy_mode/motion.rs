//! Word and line navigation helpers for the copy mode state machine.
//!
//! Vim's `w`/`b`/`e` and `W`/`B`/`E` motions differ only in which characters
//! belong to a word: alphanumerics plus `word_chars` for words, anything that
//! is not whitespace for WORDs. Each motion is therefore one scanner
//! parameterized by that predicate (AUDIT.md ARC-006).

use super::types::CopyModeState;
use crate::smart_selection::is_word_char;

/// Start of the next word after `col`, `count` times.
fn scan_forward(
    chars: &[char],
    mut col: usize,
    count: usize,
    in_word: impl Fn(char) -> bool,
) -> usize {
    for _ in 0..count {
        if col >= chars.len() {
            break;
        }
        // Skip the rest of the current word, then the gap after it.
        while col < chars.len() && in_word(chars[col]) {
            col += 1;
        }
        while col < chars.len() && !in_word(chars[col]) {
            col += 1;
        }
    }
    col
}

/// Start of the previous word before `col`, `count` times.
///
/// `col` must already be clamped below `chars.len()` (or be 0); it only
/// decreases, which keeps every index in bounds.
fn scan_backward(
    chars: &[char],
    mut col: usize,
    count: usize,
    in_word: impl Fn(char) -> bool,
) -> usize {
    for _ in 0..count {
        if col == 0 {
            break;
        }
        col = col.saturating_sub(1);
        // Skip the gap backward, then back up to the word's first character.
        while col > 0 && !in_word(chars[col]) {
            col -= 1;
        }
        while col > 0 && in_word(chars[col - 1]) {
            col -= 1;
        }
    }
    col
}

/// End of the current or next word after `col`, `count` times.
fn scan_end(chars: &[char], mut col: usize, count: usize, in_word: impl Fn(char) -> bool) -> usize {
    let last = chars.len().saturating_sub(1);
    for _ in 0..count {
        if col >= last {
            break;
        }
        col += 1;
        // Skip the gap, then advance to the word's last character.
        while col < chars.len() && !in_word(chars[col]) {
            col += 1;
        }
        while col < last && in_word(chars[col + 1]) {
            col += 1;
        }
    }
    col
}

fn is_big_word_char(ch: char) -> bool {
    !ch.is_whitespace()
}

impl CopyModeState {
    // ========================================================================
    // Word motions
    // ========================================================================

    /// Clamp a forward motion's result to the grid.
    fn clamp_to_grid(&self, col: usize) -> usize {
        col.min(self.cols.saturating_sub(1))
    }

    /// Seed column for a backward motion.
    ///
    /// `cursor_col` is clamped to the grid width, which exceeds the character
    /// count whenever the row holds wide characters — `Grid::row_text` drops
    /// their spacer cells. The backward scan only decreases its column, so this
    /// seed clamp is what keeps every `chars[..]` access in bounds.
    fn backward_seed(&self, chars: &[char]) -> usize {
        self.cursor_col.min(chars.len().saturating_sub(1))
    }

    /// Move forward to start of next word
    pub fn move_word_forward(&mut self, line_text: &str, word_chars: &str) {
        let count = self.effective_count();
        let chars: Vec<char> = line_text.chars().collect();
        let col = scan_forward(&chars, self.cursor_col, count, |c| {
            is_word_char(c, word_chars)
        });
        self.cursor_col = self.clamp_to_grid(col);
    }

    /// Move backward to start of previous word
    pub fn move_word_backward(&mut self, line_text: &str, word_chars: &str) {
        let count = self.effective_count();
        let chars: Vec<char> = line_text.chars().collect();
        let seed = self.backward_seed(&chars);
        self.cursor_col = scan_backward(&chars, seed, count, |c| is_word_char(c, word_chars));
    }

    /// Move forward to end of current/next word
    pub fn move_word_end(&mut self, line_text: &str, word_chars: &str) {
        let count = self.effective_count();
        let chars: Vec<char> = line_text.chars().collect();
        let col = scan_end(&chars, self.cursor_col, count, |c| {
            is_word_char(c, word_chars)
        });
        self.cursor_col = self.clamp_to_grid(col);
    }

    /// Move forward to start of next WORD (whitespace-delimited)
    pub fn move_big_word_forward(&mut self, line_text: &str) {
        let count = self.effective_count();
        let chars: Vec<char> = line_text.chars().collect();
        let col = scan_forward(&chars, self.cursor_col, count, is_big_word_char);
        self.cursor_col = self.clamp_to_grid(col);
    }

    /// Move backward to start of previous WORD (whitespace-delimited)
    pub fn move_big_word_backward(&mut self, line_text: &str) {
        let count = self.effective_count();
        let chars: Vec<char> = line_text.chars().collect();
        let seed = self.backward_seed(&chars);
        self.cursor_col = scan_backward(&chars, seed, count, is_big_word_char);
    }

    /// Move forward to end of current/next WORD (whitespace-delimited)
    pub fn move_big_word_end(&mut self, line_text: &str) {
        let count = self.effective_count();
        let chars: Vec<char> = line_text.chars().collect();
        let col = scan_end(&chars, self.cursor_col, count, is_big_word_char);
        self.cursor_col = self.clamp_to_grid(col);
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

    fn at_col(cols: usize, col: usize) -> CopyModeState {
        let mut cm = CopyModeState::new();
        cm.enter(col, 0, cols, 24, 0);
        cm
    }

    /// Characterization of every motion on a line where word and WORD
    /// boundaries differ: `foo.bar  baz-qux end`.
    #[test]
    fn forward_and_end_motions_on_both_classes() {
        const LINE: &str = "foo.bar  baz-qux end";
        // Punctuation is a non-word character here (not vim's third class),
        // so `w` steps over `.` and `-` along with whitespace.
        // (start col, word_forward, word_end, WORD_forward, WORD_end)
        let cases = [
            (0, 4, 2, 9, 6),
            (3, 4, 6, 9, 6),
            (4, 9, 6, 9, 6),
            (9, 13, 11, 17, 15),
            (17, 20, 19, 20, 19),
        ];
        for (start, word_fwd, word_end, big_fwd, big_end) in cases {
            let mut cm = at_col(80, start);
            cm.move_word_forward(LINE, "");
            assert_eq!(cm.cursor_col, word_fwd, "w from {start}");

            let mut cm = at_col(80, start);
            cm.move_word_end(LINE, "");
            assert_eq!(cm.cursor_col, word_end, "e from {start}");

            let mut cm = at_col(80, start);
            cm.move_big_word_forward(LINE);
            assert_eq!(cm.cursor_col, big_fwd, "W from {start}");

            let mut cm = at_col(80, start);
            cm.move_big_word_end(LINE);
            assert_eq!(cm.cursor_col, big_end, "E from {start}");
        }
    }

    #[test]
    fn backward_motions_on_both_classes() {
        const LINE: &str = "foo.bar  baz-qux end";
        // (start col, word_backward, WORD_backward)
        let cases = [(19, 17, 17), (12, 9, 9), (9, 4, 0), (4, 0, 0), (0, 0, 0)];
        for (start, word_back, big_back) in cases {
            let mut cm = at_col(80, start);
            cm.move_word_backward(LINE, "");
            assert_eq!(cm.cursor_col, word_back, "b from {start}");

            let mut cm = at_col(80, start);
            cm.move_big_word_backward(LINE);
            assert_eq!(cm.cursor_col, big_back, "B from {start}");
        }
    }

    #[test]
    fn forward_motions_past_the_text_on_a_wide_char_row() {
        // `$` parks the cursor past the last character of a CJK row; forward
        // motions must not index out of bounds and clamp to the grid.
        for line in [CJK_LINE, EMOJI_LINE] {
            let mut cm = at_line_end(80);
            cm.move_word_forward(line, "");
            assert_eq!(cm.cursor_col, 79, "{line:?}");

            let mut cm = at_line_end(80);
            cm.move_word_end(line, "");
            assert_eq!(cm.cursor_col, 79, "{line:?}");

            let mut cm = at_line_end(80);
            cm.move_big_word_forward(line);
            assert_eq!(cm.cursor_col, 79, "{line:?}");

            let mut cm = at_line_end(80);
            cm.move_big_word_end(line);
            assert_eq!(cm.cursor_col, 79, "{line:?}");
        }
    }

    #[test]
    fn forward_motion_clamps_to_the_last_grid_column() {
        let mut cm = at_col(10, 0);
        cm.move_big_word_forward("abc defghijklmnop");
        assert_eq!(cm.cursor_col, 4);
        cm.move_big_word_forward("abc defghijklmnop");
        assert_eq!(cm.cursor_col, 9, "past-the-end clamps to cols - 1");
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

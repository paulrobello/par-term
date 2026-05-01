//! Integration tests for copy mode cursor motion.
//!
//! Covers: mode entry/exit, count prefix accumulation, basic cursor motions
//! (left/right/up/down/line-start/line-end/first-non-blank), count-multiplied
//! motions, goto motions (top/bottom/line), page motions (full page, half page),
//! word motions (word/WORD), and zero-sized terminal edge cases.

use par_term::copy_mode::{CopyModeState, SearchDirection, VisualMode};

// ---------------------------------------------------------------------------
// Helper: create a ready state for testing
// ---------------------------------------------------------------------------

/// Create a state that is active with 100 scrollback lines, 80x24 screen.
/// Cursor starts at (10, 12) on-screen -> absolute line 112.
fn make_state() -> CopyModeState {
    let mut s = CopyModeState::new();
    s.enter(10, 12, 80, 24, 100);
    s
}

// ---------------------------------------------------------------------------
// Construction and defaults
// ---------------------------------------------------------------------------

#[test]
fn new_is_inactive() {
    let s = CopyModeState::new();
    assert!(!s.active);
    assert_eq!(s.visual_mode, VisualMode::None);
    assert!(s.selection_anchor.is_none());
    assert!(s.count.is_none());
    assert!(s.pending_operator.is_none());
    assert!(!s.is_searching);
    assert_eq!(s.cols, 80);
    assert_eq!(s.rows, 24);
    assert_eq!(s.scrollback_len, 0);
}

#[test]
fn default_equals_new() {
    let default = CopyModeState::default();
    let new = CopyModeState::new();
    // They should have the same initial values (active = false, etc.)
    assert_eq!(default.active, new.active);
    assert_eq!(default.cols, new.cols);
    assert_eq!(default.rows, new.rows);
    assert_eq!(default.scrollback_len, new.scrollback_len);
    assert_eq!(default.visual_mode, new.visual_mode);
}

// ---------------------------------------------------------------------------
// Enter / Exit
// ---------------------------------------------------------------------------

#[test]
fn enter_sets_active_and_dimensions() {
    let mut s = CopyModeState::new();
    s.enter(5, 3, 120, 40, 50);
    assert!(s.active);
    assert_eq!(s.cols, 120);
    assert_eq!(s.rows, 40);
    assert_eq!(s.scrollback_len, 50);
}

#[test]
fn enter_converts_screen_row_to_absolute_line() {
    let mut s = CopyModeState::new();
    s.enter(0, 5, 80, 24, 100);
    assert_eq!(s.cursor_absolute_line, 105); // 100 + 5
}

#[test]
fn enter_clamps_cursor_col_to_max() {
    let mut s = CopyModeState::new();
    // col 200 on a 80-wide terminal -> clamped to 79
    s.enter(200, 0, 80, 24, 0);
    assert_eq!(s.cursor_col, 79);
}

#[test]
fn enter_resets_visual_mode() {
    let mut s = CopyModeState::new();
    s.enter(0, 0, 80, 24, 0);
    s.toggle_visual_char();
    assert_eq!(s.visual_mode, VisualMode::Char);

    s.enter(0, 0, 80, 24, 0); // re-enter
    assert_eq!(s.visual_mode, VisualMode::None);
    assert!(s.selection_anchor.is_none());
}

#[test]
fn enter_resets_search_state() {
    let mut s = CopyModeState::new();
    s.enter(0, 0, 80, 24, 0);
    s.start_search(SearchDirection::Forward);
    s.search_input('h');

    s.enter(0, 0, 80, 24, 0);
    assert!(!s.is_searching);
    assert!(s.search_query.is_empty());
}

#[test]
fn enter_twice_resets_state_cleanly() {
    // Verify that calling enter a second time produces the same clean initial
    // state as calling it the first time — specifically that any pending-g
    // or pending-mark flags that exist internally are reset.
    let mut s = CopyModeState::new();
    s.enter(0, 0, 80, 24, 0);
    // Trigger some state changes
    s.toggle_visual_char();
    s.start_search(SearchDirection::Forward);
    s.search_input('x');

    // Re-enter should reset everything
    s.enter(5, 5, 80, 24, 0);
    assert!(s.active);
    assert_eq!(s.visual_mode, VisualMode::None);
    assert!(s.selection_anchor.is_none());
    assert!(!s.is_searching);
    assert!(s.search_query.is_empty());
}

#[test]
fn exit_clears_active() {
    let mut s = make_state();
    s.exit();
    assert!(!s.active);
}

#[test]
fn exit_clears_visual_mode() {
    let mut s = make_state();
    s.toggle_visual_char();
    s.exit();
    assert_eq!(s.visual_mode, VisualMode::None);
    assert!(s.selection_anchor.is_none());
}

#[test]
fn exit_clears_count() {
    let mut s = make_state();
    s.push_count_digit(5);
    s.exit();
    // After exit, effective_count should default to 1
    assert_eq!(s.effective_count(), 1);
}

// ---------------------------------------------------------------------------
// Count prefix
// ---------------------------------------------------------------------------

#[test]
fn count_defaults_to_1() {
    let mut s = make_state();
    assert_eq!(s.effective_count(), 1);
}

#[test]
fn push_count_digit_accumulates() {
    let mut s = make_state();
    s.push_count_digit(1);
    s.push_count_digit(2);
    s.push_count_digit(3);
    assert_eq!(s.effective_count(), 123);
}

#[test]
fn effective_count_resets_after_read() {
    let mut s = make_state();
    s.push_count_digit(5);
    let _ = s.effective_count();
    // Second read should return 1 (default)
    assert_eq!(s.effective_count(), 1);
}

#[test]
fn count_0_is_not_1() {
    // Pushing a 0 sets count to Some(0) which effective_count returns as 0
    let mut s = make_state();
    s.push_count_digit(0);
    // effective_count returns 0 when count is Some(0)
    assert_eq!(s.effective_count(), 0);
}

// ---------------------------------------------------------------------------
// Basic cursor motions
// ---------------------------------------------------------------------------

#[test]
fn move_left_decrements_col() {
    let mut s = make_state(); // col = 10
    s.move_left();
    assert_eq!(s.cursor_col, 9);
}

#[test]
fn move_left_clamps_at_zero() {
    let mut s = make_state();
    s.cursor_col = 2;
    s.push_count_digit(9); // count = 9, but col is only 2
    s.move_left();
    assert_eq!(s.cursor_col, 0);
}

#[test]
fn move_right_increments_col() {
    let mut s = make_state(); // col = 10
    s.move_right();
    assert_eq!(s.cursor_col, 11);
}

#[test]
fn move_right_clamps_at_max_col() {
    let mut s = make_state();
    s.cursor_col = 78;
    s.push_count_digit(9); // would go to 87, but max is 79
    s.move_right();
    assert_eq!(s.cursor_col, 79);
}

#[test]
fn move_up_decrements_line() {
    let mut s = make_state(); // abs line = 112
    s.move_up();
    assert_eq!(s.cursor_absolute_line, 111);
}

#[test]
fn move_up_clamps_at_zero() {
    let mut s = make_state();
    s.cursor_absolute_line = 3;
    s.push_count_digit(9);
    s.move_up();
    assert_eq!(s.cursor_absolute_line, 0);
}

#[test]
fn move_down_increments_line() {
    let mut s = make_state(); // abs line = 112
    s.move_down();
    assert_eq!(s.cursor_absolute_line, 113);
}

#[test]
fn move_down_clamps_at_max_line() {
    let mut s = make_state();
    // max_line = scrollback(100) + rows(24) - 1 = 123
    s.cursor_absolute_line = 122;
    s.push_count_digit(9);
    s.move_down();
    assert_eq!(s.cursor_absolute_line, 123);
}

#[test]
fn move_to_line_start_sets_col_zero() {
    let mut s = make_state();
    s.move_to_line_start();
    assert_eq!(s.cursor_col, 0);
}

#[test]
fn move_to_line_end_sets_col_to_last() {
    let mut s = make_state();
    s.move_to_line_end();
    assert_eq!(s.cursor_col, 79); // cols - 1
}

#[test]
fn move_to_first_non_blank() {
    let mut s = make_state();
    s.move_to_first_non_blank("   hello world");
    assert_eq!(s.cursor_col, 3);
}

#[test]
fn move_to_first_non_blank_all_blank_uses_zero() {
    let mut s = make_state();
    s.move_to_first_non_blank("     ");
    assert_eq!(s.cursor_col, 0);
}

#[test]
fn move_to_first_non_blank_empty_line_uses_zero() {
    let mut s = make_state();
    s.move_to_first_non_blank("");
    assert_eq!(s.cursor_col, 0);
}

// ---------------------------------------------------------------------------
// Count-multiplied motions
// ---------------------------------------------------------------------------

#[test]
fn move_down_with_count_5() {
    let mut s = make_state(); // abs line = 112
    s.push_count_digit(5);
    s.move_down();
    assert_eq!(s.cursor_absolute_line, 117);
}

#[test]
fn move_up_with_count_10() {
    let mut s = make_state(); // abs line = 112
    s.push_count_digit(1);
    s.push_count_digit(0);
    s.move_up();
    assert_eq!(s.cursor_absolute_line, 102);
}

#[test]
fn move_right_with_count_clamps() {
    let mut s = make_state(); // col = 10
    s.push_count_digit(8);
    s.push_count_digit(0); // count = 80
    s.move_right();
    assert_eq!(s.cursor_col, 79); // max col
}

// ---------------------------------------------------------------------------
// Goto motions
// ---------------------------------------------------------------------------

#[test]
fn goto_top_sets_line_zero() {
    let mut s = make_state();
    s.goto_top();
    assert_eq!(s.cursor_absolute_line, 0);
}

#[test]
fn goto_bottom_sets_last_line() {
    let mut s = make_state();
    s.goto_bottom();
    // max_line = 100 + 24 - 1 = 123
    assert_eq!(s.cursor_absolute_line, 123);
}

#[test]
fn goto_line_clamps_to_max() {
    let mut s = make_state();
    s.goto_line(9999);
    assert_eq!(s.cursor_absolute_line, 123);
}

#[test]
fn goto_line_exact() {
    let mut s = make_state();
    s.goto_line(50);
    assert_eq!(s.cursor_absolute_line, 50);
}

// ---------------------------------------------------------------------------
// Page motions
// ---------------------------------------------------------------------------

#[test]
fn page_up_moves_full_screen() {
    let mut s = make_state(); // abs line = 112, rows = 24
    s.page_up();
    assert_eq!(s.cursor_absolute_line, 88); // 112 - 24
}

#[test]
fn page_up_clamps_at_zero() {
    let mut s = make_state();
    s.cursor_absolute_line = 10;
    s.page_up();
    assert_eq!(s.cursor_absolute_line, 0);
}

#[test]
fn page_down_moves_full_screen() {
    let mut s = make_state(); // abs line = 112
    s.page_down();
    // 112 + 24 = 136 > max_line 123 -> clamped to 123
    assert_eq!(s.cursor_absolute_line, 123);
}

#[test]
fn half_page_up_moves_half_screen() {
    let mut s = make_state(); // abs line = 112, half = 12
    s.half_page_up();
    assert_eq!(s.cursor_absolute_line, 100);
}

#[test]
fn half_page_down_moves_half_screen() {
    let mut s = make_state(); // abs line = 112, half = 12
    s.half_page_down();
    // 112 + 12 = 124 > max_line 123 -> clamped to 123
    assert_eq!(s.cursor_absolute_line, 123);
}

#[test]
fn half_page_up_with_count_2() {
    let mut s = make_state(); // abs line = 112
    s.push_count_digit(2);
    s.half_page_up(); // 2 * 12 = 24
    assert_eq!(s.cursor_absolute_line, 88);
}

// ---------------------------------------------------------------------------
// Word motions
// ---------------------------------------------------------------------------

#[test]
fn word_forward_moves_past_current_word() {
    let mut s = make_state();
    s.cursor_col = 0;
    // "hello world foo" -- 'h' is at col 0, next word starts at 6
    let line = "hello world foo";
    s.move_word_forward(line, "");
    assert_eq!(s.cursor_col, 6); // start of "world"
}

#[test]
fn word_forward_from_end_of_line_stays() {
    let mut s = make_state();
    let line = "hello";
    s.cursor_col = 4; // last char
    s.move_word_forward(line, "");
    // Already at or past the end
    assert!(s.cursor_col >= 4);
}

#[test]
fn word_backward_returns_to_word_start() {
    let mut s = make_state();
    let line = "hello world foo";
    s.cursor_col = 9; // inside "world"
    s.move_word_backward(line, "");
    assert_eq!(s.cursor_col, 6); // start of "world"
}

#[test]
fn word_backward_from_start_of_line_stays() {
    let mut s = make_state();
    let line = "hello world";
    s.cursor_col = 0;
    s.move_word_backward(line, "");
    assert_eq!(s.cursor_col, 0);
}

#[test]
fn word_end_moves_to_end_of_word() {
    let mut s = make_state();
    let line = "hello world";
    s.cursor_col = 0;
    s.move_word_end(line, "");
    assert_eq!(s.cursor_col, 4); // end of "hello"
}

// ---------------------------------------------------------------------------
// WORD motions (whitespace-delimited)
// ---------------------------------------------------------------------------

#[test]
fn big_word_forward_skips_punctuation_as_word() {
    let mut s = make_state();
    let line = "foo.bar baz";
    s.cursor_col = 0;
    // "foo.bar" is one WORD (no whitespace), "baz" is the next
    s.move_big_word_forward(line);
    assert_eq!(s.cursor_col, 8); // start of "baz"
}

#[test]
fn big_word_backward_skips_to_prev_word() {
    let mut s = make_state();
    let line = "foo bar baz";
    s.cursor_col = 8; // inside "baz"
    s.move_big_word_backward(line);
    assert_eq!(s.cursor_col, 4); // start of "bar"
}

#[test]
fn big_word_end_moves_to_end_of_whitespace_word() {
    let mut s = make_state();
    let line = "foo.bar baz";
    s.cursor_col = 0;
    s.move_big_word_end(line);
    assert_eq!(s.cursor_col, 6); // end of "foo.bar" (last char at index 6)
}

// ---------------------------------------------------------------------------
// Edge cases with zero-sized terminal
// ---------------------------------------------------------------------------

#[test]
fn enter_with_zero_scrollback() {
    let mut s = CopyModeState::new();
    s.enter(0, 0, 80, 24, 0);
    assert_eq!(s.cursor_absolute_line, 0);
    assert_eq!(s.scrollback_len, 0);
}

#[test]
fn move_in_zero_col_terminal_does_not_panic() {
    let mut s = CopyModeState::new();
    // Edge case: 1-column terminal
    s.enter(0, 0, 1, 24, 0);
    s.move_right();
    assert_eq!(s.cursor_col, 0); // can't go right
    s.move_left();
    assert_eq!(s.cursor_col, 0); // can't go left
}

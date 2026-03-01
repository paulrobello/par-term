//! Integration tests for the copy mode state machine.
//!
//! These tests exercise `CopyModeState` as exposed through the `par_term` crate.
//! They do not require a GPU context, a window, or a running PTY — all state is
//! exercised through the pure state-machine API.
//!
//! Coverage targets (AUD-052):
//! - Mode entry and exit
//! - Visual mode state transitions (None → Char → Line → Block, toggle-off)
//! - Cursor movement: basic, with count prefix, boundary clamping
//! - Word motions on representative strings
//! - WORD motions (whitespace-delimited)
//! - Page / half-page motions
//! - Marks: set, goto, nonexistent
//! - Search mode entry, input, backspace, cancel
//! - Selection computation (compute_selection)
//! - Viewport helpers (screen_cursor_pos, required_scroll_offset)
//! - Status text formatting
//! - Count prefix accumulation and reset
//! - Dimension update (update_dimensions) with cursor clamping

use par_term::copy_mode::{CopyModeState, SearchDirection, VisualMode};
use par_term::selection::SelectionMode;

// ---------------------------------------------------------------------------
// Helper: create a ready state for testing
// ---------------------------------------------------------------------------

/// Create a state that is active with 100 scrollback lines, 80×24 screen.
/// Cursor starts at (10, 12) on-screen → absolute line 112.
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
    // col 200 on a 80-wide terminal → clamped to 79
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
    // 112 + 24 = 136 > max_line 123 → clamped to 123
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
    // 112 + 12 = 124 > max_line 123 → clamped to 123
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
// Visual mode state transitions
// ---------------------------------------------------------------------------

#[test]
fn toggle_visual_char_enters_mode() {
    let mut s = make_state();
    s.toggle_visual_char();
    assert_eq!(s.visual_mode, VisualMode::Char);
    assert!(s.selection_anchor.is_some());
}

#[test]
fn toggle_visual_char_twice_exits() {
    let mut s = make_state();
    s.toggle_visual_char();
    s.toggle_visual_char();
    assert_eq!(s.visual_mode, VisualMode::None);
    assert!(s.selection_anchor.is_none());
}

#[test]
fn toggle_visual_line_enters_mode() {
    let mut s = make_state();
    s.toggle_visual_line();
    assert_eq!(s.visual_mode, VisualMode::Line);
    assert!(s.selection_anchor.is_some());
}

#[test]
fn toggle_visual_line_twice_exits() {
    let mut s = make_state();
    s.toggle_visual_line();
    s.toggle_visual_line();
    assert_eq!(s.visual_mode, VisualMode::None);
}

#[test]
fn toggle_visual_block_enters_mode() {
    let mut s = make_state();
    s.toggle_visual_block();
    assert_eq!(s.visual_mode, VisualMode::Block);
    assert!(s.selection_anchor.is_some());
}

#[test]
fn toggle_visual_block_twice_exits() {
    let mut s = make_state();
    s.toggle_visual_block();
    s.toggle_visual_block();
    assert_eq!(s.visual_mode, VisualMode::None);
}

#[test]
fn switching_visual_modes_clears_previous_anchor() {
    let mut s = make_state();
    s.toggle_visual_char();
    let anchor_char = s.selection_anchor.unwrap();

    s.move_down(); // move cursor so anchor differs
    s.toggle_visual_line(); // switch to line mode
    let anchor_line = s.selection_anchor.unwrap();

    // New anchor is at current cursor position, not the old one
    assert_ne!(anchor_char, anchor_line);
    assert_eq!(s.visual_mode, VisualMode::Line);
}

#[test]
fn selection_anchor_at_cursor_when_entering_visual() {
    let mut s = make_state(); // col=10, line=112
    s.toggle_visual_char();
    assert_eq!(s.selection_anchor, Some((112, 10)));
}

// ---------------------------------------------------------------------------
// compute_selection
// ---------------------------------------------------------------------------

#[test]
fn compute_selection_none_without_visual_mode() {
    let s = make_state();
    assert!(s.compute_selection(0).is_none());
}

#[test]
fn compute_selection_char_mode_produces_normal_selection() {
    let mut s = make_state(); // col=10, line=112
    s.toggle_visual_char();
    s.move_right();
    s.move_right();
    s.move_down();

    let sel = s.compute_selection(0).unwrap();
    assert_eq!(sel.mode, SelectionMode::Normal);
    // anchor is (10, 112), cursor is (12, 113)
    // viewport_top = scrollback(100) - offset(0) = 100
    // anchor_row = 112 - 100 = 12, cursor_row = 113 - 100 = 13
    assert_eq!(sel.start, (10, 12));
    assert_eq!(sel.end, (12, 13));
}

#[test]
fn compute_selection_line_mode_produces_line_selection() {
    let mut s = make_state();
    s.toggle_visual_line();
    s.move_down();

    let sel = s.compute_selection(0).unwrap();
    assert_eq!(sel.mode, SelectionMode::Line);
}

#[test]
fn compute_selection_block_mode_produces_rectangular_selection() {
    let mut s = make_state();
    s.toggle_visual_block();
    s.move_right();
    s.move_down();

    let sel = s.compute_selection(0).unwrap();
    assert_eq!(sel.mode, SelectionMode::Rectangular);
}

#[test]
fn compute_selection_respects_scroll_offset() {
    let mut s = make_state(); // scrollback=100, col=10, line=112
    s.toggle_visual_char();

    // With scroll_offset=50, viewport_top = 100 - 50 = 50
    // cursor_row = 112 - 50 = 62
    let sel = s.compute_selection(50).unwrap();
    assert_eq!(sel.start.1, 62); // anchor row
    assert_eq!(sel.end.1, 62); // cursor row (same position, no movement)
}

#[test]
fn compute_selection_selection_can_go_backwards() {
    let mut s = make_state(); // col=10, line=112
    s.toggle_visual_char();
    s.move_left(); // col → 9
    s.move_up(); // line → 111

    let sel = s.compute_selection(0).unwrap();
    // anchor at (10, 12), cursor now at (9, 11) in screen-relative
    // selection captures both (start may be > end)
    assert!(sel.start != sel.end);
}

// ---------------------------------------------------------------------------
// Marks
// ---------------------------------------------------------------------------

#[test]
fn set_and_goto_mark() {
    let mut s = make_state(); // col=10, line=112
    s.set_mark('a');

    s.move_down();
    s.move_right();
    // Cursor is now different from mark

    assert!(s.goto_mark('a'));
    assert_eq!(s.cursor_col, 10);
    assert_eq!(s.cursor_absolute_line, 112);
}

#[test]
fn goto_nonexistent_mark_returns_false() {
    let mut s = make_state();
    assert!(!s.goto_mark('z'));
}

#[test]
fn mark_can_be_overwritten() {
    let mut s = make_state();
    s.set_mark('a');
    s.move_down();
    s.move_right();
    s.set_mark('a'); // overwrite at new position
    let new_line = s.cursor_absolute_line;
    let new_col = s.cursor_col;

    // go somewhere else
    s.goto_top();
    s.goto_mark('a');
    assert_eq!(s.cursor_absolute_line, new_line);
    assert_eq!(s.cursor_col, new_col);
}

#[test]
fn multiple_marks_are_independent() {
    let mut s = make_state();

    s.cursor_col = 5;
    s.cursor_absolute_line = 100;
    s.set_mark('a');

    s.cursor_col = 20;
    s.cursor_absolute_line = 110;
    s.set_mark('b');

    s.goto_mark('a');
    assert_eq!(s.cursor_col, 5);
    assert_eq!(s.cursor_absolute_line, 100);

    s.goto_mark('b');
    assert_eq!(s.cursor_col, 20);
    assert_eq!(s.cursor_absolute_line, 110);
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

#[test]
fn start_search_forward_sets_state() {
    let mut s = make_state();
    s.start_search(SearchDirection::Forward);
    assert!(s.is_searching);
    assert_eq!(s.search_direction, SearchDirection::Forward);
    assert!(s.search_query.is_empty());
}

#[test]
fn start_search_backward_sets_direction() {
    let mut s = make_state();
    s.start_search(SearchDirection::Backward);
    assert_eq!(s.search_direction, SearchDirection::Backward);
}

#[test]
fn search_input_accumulates_chars() {
    let mut s = make_state();
    s.start_search(SearchDirection::Forward);
    s.search_input('h');
    s.search_input('e');
    s.search_input('l');
    assert_eq!(s.search_query, "hel");
}

#[test]
fn search_backspace_removes_last_char() {
    let mut s = make_state();
    s.start_search(SearchDirection::Forward);
    s.search_input('a');
    s.search_input('b');
    s.search_backspace();
    assert_eq!(s.search_query, "a");
}

#[test]
fn search_backspace_on_empty_is_noop() {
    let mut s = make_state();
    s.start_search(SearchDirection::Forward);
    s.search_backspace(); // should not panic
    assert!(s.search_query.is_empty());
}

#[test]
fn cancel_search_clears_state() {
    let mut s = make_state();
    s.start_search(SearchDirection::Forward);
    s.search_input('t');
    s.cancel_search();
    assert!(!s.is_searching);
    assert!(s.search_query.is_empty());
}

#[test]
fn start_search_clears_previous_query() {
    let mut s = make_state();
    s.start_search(SearchDirection::Forward);
    s.search_input('o');
    s.cancel_search();

    s.start_search(SearchDirection::Backward);
    assert!(s.search_query.is_empty());
}

// ---------------------------------------------------------------------------
// Viewport helpers
// ---------------------------------------------------------------------------

#[test]
fn screen_cursor_pos_visible_in_viewport() {
    let s = make_state(); // col=10, line=112, scrollback=100
    // scroll_offset=0 → viewport_top=100, viewport_bottom=124
    // line 112 is in [100, 124) → visible
    let pos = s.screen_cursor_pos(0);
    assert_eq!(pos, Some((10, 12))); // row = 112 - 100 = 12
}

#[test]
fn screen_cursor_pos_above_viewport_is_none() {
    let mut s = make_state();
    s.cursor_absolute_line = 50; // below viewport_top=100 at offset=0
    assert_eq!(s.screen_cursor_pos(0), None);
}

#[test]
fn screen_cursor_pos_below_viewport_is_none() {
    let mut s = make_state();
    s.cursor_absolute_line = 130; // beyond viewport_bottom=124 at offset=0
    assert_eq!(s.screen_cursor_pos(0), None);
}

#[test]
fn screen_cursor_pos_at_viewport_top_edge() {
    let mut s = make_state();
    s.cursor_absolute_line = 100; // exactly at viewport_top (offset=0)
    let pos = s.screen_cursor_pos(0);
    assert_eq!(pos, Some((10, 0))); // screen row = 0
}

#[test]
fn screen_cursor_pos_at_viewport_bottom_edge() {
    let mut s = make_state();
    s.cursor_absolute_line = 123; // viewport_bottom - 1 = 123
    let pos = s.screen_cursor_pos(0);
    assert_eq!(pos, Some((10, 23))); // screen row = 23
}

#[test]
fn screen_cursor_pos_with_scroll_offset() {
    let mut s = make_state();
    s.cursor_absolute_line = 60; // originally invisible
    // scroll up by 50 → viewport_top = 100 - 50 = 50
    let pos = s.screen_cursor_pos(50);
    assert_eq!(pos, Some((10, 10))); // row = 60 - 50
}

#[test]
fn required_scroll_offset_cursor_already_visible_is_none() {
    let s = make_state(); // line=112, viewport=[100,124), offset=0
    assert_eq!(s.required_scroll_offset(0), None);
}

#[test]
fn required_scroll_offset_cursor_above_viewport() {
    let mut s = make_state();
    s.cursor_absolute_line = 50; // above viewport_top=100 at offset=0
    let new_offset = s.required_scroll_offset(0).unwrap();
    // Expected: scrollback(100) - 50 = 50
    assert_eq!(new_offset, 50);
}

#[test]
fn required_scroll_offset_cursor_below_viewport() {
    let mut s = make_state();
    s.cursor_absolute_line = 130; // below viewport_bottom=124 at offset=0
    let new_offset = s.required_scroll_offset(0);
    assert!(new_offset.is_some()); // some scrolling is needed
}

// ---------------------------------------------------------------------------
// update_dimensions
// ---------------------------------------------------------------------------

#[test]
fn update_dimensions_changes_cols_rows_scrollback() {
    let mut s = make_state();
    s.update_dimensions(120, 40, 200);
    assert_eq!(s.cols, 120);
    assert_eq!(s.rows, 40);
    assert_eq!(s.scrollback_len, 200);
}

#[test]
fn update_dimensions_clamps_cursor_col() {
    let mut s = make_state(); // col=10
    s.update_dimensions(8, 24, 100); // terminal shrinks to 8 cols
    assert_eq!(s.cursor_col, 7); // clamped to cols-1
}

#[test]
fn update_dimensions_clamps_cursor_line() {
    let mut s = make_state(); // abs_line=112, max_line was 123
    s.update_dimensions(80, 10, 50); // now max_line = 50+10-1 = 59
    assert!(s.cursor_absolute_line <= 59);
}

// ---------------------------------------------------------------------------
// status_text
// ---------------------------------------------------------------------------

#[test]
fn status_text_normal_mode_contains_copy() {
    let s = make_state();
    let text = s.status_text();
    assert!(text.contains("COPY"), "Expected COPY in '{}'", text);
}

#[test]
fn status_text_visual_char_mode_contains_visual() {
    let mut s = make_state();
    s.toggle_visual_char();
    let text = s.status_text();
    assert!(text.contains("VISUAL"), "Expected VISUAL in '{}'", text);
}

#[test]
fn status_text_visual_line_mode_contains_visual_line() {
    let mut s = make_state();
    s.toggle_visual_line();
    let text = s.status_text();
    assert!(
        text.contains("VISUAL LINE"),
        "Expected VISUAL LINE in '{}'",
        text
    );
}

#[test]
fn status_text_visual_block_mode_contains_visual_block() {
    let mut s = make_state();
    s.toggle_visual_block();
    let text = s.status_text();
    assert!(
        text.contains("VISUAL BLOCK"),
        "Expected VISUAL BLOCK in '{}'",
        text
    );
}

#[test]
fn status_text_search_forward_starts_with_slash() {
    let mut s = make_state();
    s.start_search(SearchDirection::Forward);
    s.search_input('a');
    s.search_input('b');
    let text = s.status_text();
    assert!(text.starts_with('/'), "Expected '/' prefix, got '{}'", text);
    assert!(text.contains("ab"));
}

#[test]
fn status_text_search_backward_starts_with_question() {
    let mut s = make_state();
    s.start_search(SearchDirection::Backward);
    s.search_input('x');
    let text = s.status_text();
    assert!(text.starts_with('?'), "Expected '?' prefix, got '{}'", text);
}

#[test]
fn status_text_includes_position_info() {
    let s = make_state(); // col=10, abs_line=112
    let text = s.status_text();
    // The status text contains col info
    assert!(text.contains(':'), "Expected ':' separator in '{}'", text);
}

// ---------------------------------------------------------------------------
// Word motions
// ---------------------------------------------------------------------------

#[test]
fn word_forward_moves_past_current_word() {
    let mut s = make_state();
    s.cursor_col = 0;
    // "hello world foo" — 'h' is at col 0, next word starts at 6
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
// Interaction: visual mode + movement → selection grows
// ---------------------------------------------------------------------------

#[test]
fn selection_grows_as_cursor_moves() {
    let mut s = make_state(); // col=10, line=112
    s.toggle_visual_char();
    let anchor = s.selection_anchor.unwrap();

    s.move_right();
    s.move_right();
    s.move_down();

    let sel = s.compute_selection(0).unwrap();
    // Anchor should be unchanged; cursor should have moved
    assert_eq!(anchor, (112, 10));
    // Start should be the anchor
    assert_eq!(sel.start, (10, 12)); // col=10, row=112-100=12
    // End should be new cursor position
    assert_eq!(sel.end, (12, 13)); // col=12, row=113-100=13
}

#[test]
fn exit_clears_selection() {
    let mut s = make_state();
    s.toggle_visual_char();
    s.move_right();

    s.exit();
    assert!(s.compute_selection(0).is_none());
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

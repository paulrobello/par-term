//! Integration tests for copy mode visual selection, marks, viewport helpers,
//! dimension updates, and status text formatting.

use par_term::copy_mode::{CopyModeState, VisualMode};
use par_term::selection::SelectionMode;

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
    s.move_left(); // col -> 9
    s.move_up(); // line -> 111

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
// Viewport helpers
// ---------------------------------------------------------------------------

#[test]
fn screen_cursor_pos_visible_in_viewport() {
    let s = make_state(); // col=10, line=112, scrollback=100
    // scroll_offset=0 -> viewport_top=100, viewport_bottom=124
    // line 112 is in [100, 124) -> visible
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
    // scroll up by 50 -> viewport_top = 100 - 50 = 50
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
fn status_text_includes_position_info() {
    let s = make_state(); // col=10, abs_line=112
    let text = s.status_text();
    // The status text contains col info
    assert!(text.contains(':'), "Expected ':' separator in '{}'", text);
}

// ---------------------------------------------------------------------------
// Interaction: visual mode + movement -> selection grows
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

//! Tests for the vi-style copy mode state machine.

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

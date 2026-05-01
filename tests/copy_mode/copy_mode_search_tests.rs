//! Integration tests for copy mode search functionality.
//!
//! Covers: forward/backward search entry, character input, backspace,
//! cancel, query clearing on re-entry, and status text display during search.

use par_term::copy_mode::{CopyModeState, SearchDirection};

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

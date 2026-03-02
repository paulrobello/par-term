//! Tests for tab bar context menu lifecycle and state management.
//!
//! These tests verify the context menu open/close/rename state machine.

use par_term::tab::TabId;
use par_term::tab_bar_ui::TabBarUI;

// ============================================================================
// Context Menu Lifecycle Tests (L-15)
// ============================================================================

#[test]
fn test_context_menu_initially_closed() {
    let tab_bar = TabBarUI::new();
    assert!(
        !tab_bar.is_context_menu_open(),
        "Context menu should start closed"
    );
    assert!(tab_bar.test_context_menu_tab().is_none());
}

#[test]
fn test_context_menu_opens_for_tab() {
    let mut tab_bar = TabBarUI::new();
    let tab_id: TabId = 5;

    tab_bar.test_open_context_menu(tab_id);

    assert!(
        tab_bar.is_context_menu_open(),
        "Context menu should be open"
    );
    assert_eq!(
        tab_bar.test_context_menu_tab(),
        Some(tab_id),
        "Context menu should be open for the correct tab"
    );
}

#[test]
fn test_context_menu_closes_after_action() {
    let mut tab_bar = TabBarUI::new();
    let tab_id: TabId = 10;

    tab_bar.test_open_context_menu(tab_id);
    assert!(tab_bar.is_context_menu_open());

    // Simulate action taken → close menu
    tab_bar.test_close_context_menu();

    assert!(
        !tab_bar.is_context_menu_open(),
        "Context menu should be closed after action"
    );
    assert!(tab_bar.test_context_menu_tab().is_none());
}

#[test]
fn test_context_menu_switches_between_tabs() {
    let mut tab_bar = TabBarUI::new();
    let tab_a: TabId = 1;
    let tab_b: TabId = 2;

    tab_bar.test_open_context_menu(tab_a);
    assert_eq!(tab_bar.test_context_menu_tab(), Some(tab_a));

    // Opening on a different tab replaces the previous context
    tab_bar.test_open_context_menu(tab_b);
    assert_eq!(
        tab_bar.test_context_menu_tab(),
        Some(tab_b),
        "Context menu should switch to the new tab"
    );
    assert!(tab_bar.is_context_menu_open());
}

#[test]
fn test_context_menu_rename_state() {
    let mut tab_bar = TabBarUI::new();
    let tab_id: TabId = 3;

    // Rename mode is off initially
    assert!(!tab_bar.is_renaming());

    tab_bar.test_open_context_menu(tab_id);
    assert!(
        !tab_bar.is_renaming(),
        "Rename mode should be off after opening menu"
    );

    // Activate rename mode
    tab_bar.test_set_renaming(true);
    assert!(
        tab_bar.is_renaming(),
        "Rename mode should be on after activation"
    );

    // Closing menu resets rename mode
    tab_bar.test_close_context_menu();
    assert!(
        !tab_bar.is_renaming(),
        "Rename mode should be off after menu close"
    );
}

#[test]
fn test_context_menu_independent_of_drag_state() {
    let mut tab_bar = TabBarUI::new();
    let tab_id: TabId = 4;
    let drag_tab: TabId = 99;

    // Both drag and context menu can coexist in state
    tab_bar.test_set_drag_state(Some(drag_tab), true);
    tab_bar.test_open_context_menu(tab_id);

    assert!(tab_bar.is_dragging());
    assert!(tab_bar.is_context_menu_open());

    // Closing context menu doesn't affect drag
    tab_bar.test_close_context_menu();
    assert!(!tab_bar.is_context_menu_open());
    assert!(
        tab_bar.is_dragging(),
        "Drag state should be unaffected by menu close"
    );
}

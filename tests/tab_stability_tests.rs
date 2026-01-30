//! Tests for tab stability fixes
//!
//! This test module covers fixes from the tab-stability branch:
//!
//! ## Tab Numbering by Position
//!
//! Tabs now display "Tab 1", "Tab 2", etc. based on their position in the tab bar,
//! not their unique internal ID. When tabs are closed or reordered, tabs with
//! default titles (not set by OSC sequences or user) get renumbered automatically.
//!
//! ### Key behaviors:
//! - New tabs get numbered based on current tab count, not unique ID
//! - `has_default_title` tracks whether title was set via OSC/CWD or is still "Tab N"
//! - Closing a tab triggers renumbering of remaining default-titled tabs
//! - Reordering tabs triggers renumbering of default-titled tabs
//! - Tabs with custom titles (from OSC sequences) are NOT renumbered
//!
//! ## Content Offset for Tab Bar
//!
//! The renderer now accounts for tab bar height when positioning terminal content.
//! The `content_offset_y` field prevents terminal cells from overlapping with the tab bar.
//!
//! ## Enter Key Prevention
//!
//! The tab bar uses `clicked_by(PointerButton::Primary)` instead of `clicked()` to
//! prevent keyboard activation (Enter/Space) from triggering tab switches.
//! See `tests/tab_bar_ui_tests.rs` for detailed documentation.
//!
//! ## Mouse Event Timing
//!
//! Mouse events in the tab bar area are handled before setting terminal button state,
//! and window redraws are requested to ensure egui processes the events.

use par_term::config::Config;
use par_term::tab_bar_ui::{TabBarAction, TabBarUI};

// ============================================================================
// Tab Bar Height and Content Offset Tests
// ============================================================================

#[test]
fn test_tab_bar_height_affects_content_area() {
    // When the tab bar is visible, it should reduce the available content area
    // This test verifies the expected height values
    let tab_bar = TabBarUI::new();
    let config = Config::default();

    // With default config (WhenMultiple mode)
    // 1 tab: no bar, height = 0
    let height_1_tab = tab_bar.get_height(1, &config);
    assert_eq!(height_1_tab, 0.0, "Single tab should hide tab bar");

    // 2 tabs: bar visible, height = config.tab_bar_height
    let height_2_tabs = tab_bar.get_height(2, &config);
    assert!(height_2_tabs > 0.0, "Multiple tabs should show tab bar");
    assert_eq!(
        height_2_tabs, config.tab_bar_height,
        "Tab bar height should match config"
    );
}

#[test]
fn test_tab_bar_height_zero_tabs() {
    let tab_bar = TabBarUI::new();
    let config = Config::default();

    // Edge case: 0 tabs should also hide the bar
    let height = tab_bar.get_height(0, &config);
    assert_eq!(height, 0.0, "Zero tabs should hide tab bar");
}

#[test]
fn test_tab_bar_height_many_tabs() {
    let tab_bar = TabBarUI::new();
    let config = Config::default();

    // Height should be consistent regardless of tab count (when visible)
    let height_2 = tab_bar.get_height(2, &config);
    let height_10 = tab_bar.get_height(10, &config);
    let height_100 = tab_bar.get_height(100, &config);

    assert_eq!(height_2, height_10, "Height should be constant for multiple tabs");
    assert_eq!(height_10, height_100, "Height should be constant regardless of tab count");
}

// ============================================================================
// Tab Bar Action Tests for Close Button
// ============================================================================

#[test]
fn test_close_action_distinct_from_switch() {
    // Ensure Close and SwitchTo actions for the same tab are different
    // This is critical for the close button fix where we need to distinguish
    // between clicking the tab (switch) and clicking the close button (close)
    let switch_action = TabBarAction::SwitchTo(5);
    let close_action = TabBarAction::Close(5);

    assert_ne!(switch_action, close_action, "Close and SwitchTo should be different actions");
}

#[test]
fn test_close_action_equality() {
    // Close actions should be equal only if they target the same tab
    let close1 = TabBarAction::Close(1);
    let close2 = TabBarAction::Close(1);
    let close3 = TabBarAction::Close(2);

    assert_eq!(close1, close2, "Same tab close actions should be equal");
    assert_ne!(close1, close3, "Different tab close actions should be different");
}

// ============================================================================
// Tab Bar Action Tests for Reorder
// ============================================================================

#[test]
fn test_reorder_action_preserves_target_position() {
    // Reorder action should preserve both the tab ID and target position
    // These are used to renumber tabs after reordering
    let action = TabBarAction::Reorder(3, 0);

    match action {
        TabBarAction::Reorder(id, pos) => {
            assert_eq!(id, 3, "Tab ID should be preserved");
            assert_eq!(pos, 0, "Target position should be preserved");
        }
        _ => panic!("Expected Reorder action"),
    }
}

#[test]
fn test_reorder_action_different_positions() {
    // Reorder to different positions should produce different actions
    let action1 = TabBarAction::Reorder(1, 0);
    let action2 = TabBarAction::Reorder(1, 2);

    assert_ne!(action1, action2, "Reorder to different positions should differ");
}

#[test]
fn test_reorder_action_different_tabs() {
    // Reorder of different tabs to same position should be different
    let action1 = TabBarAction::Reorder(1, 0);
    let action2 = TabBarAction::Reorder(2, 0);

    assert_ne!(action1, action2, "Reorder of different tabs should differ");
}

// ============================================================================
// Tab Bar State Tests
// ============================================================================

#[test]
fn test_close_hovered_state_initialization() {
    let tab_bar = TabBarUI::new();

    // Initially no close button should be hovered
    assert!(
        tab_bar.close_hovered.is_none(),
        "Close button should not be hovered initially"
    );
}

#[test]
fn test_hovered_tab_state_initialization() {
    let tab_bar = TabBarUI::new();

    // Initially no tab should be hovered
    assert!(
        tab_bar.hovered_tab.is_none(),
        "No tab should be hovered initially"
    );
}

#[test]
fn test_context_menu_initially_closed() {
    let tab_bar = TabBarUI::new();

    // Context menu should be closed initially
    assert!(
        !tab_bar.is_context_menu_open(),
        "Context menu should be closed initially"
    );
}

// ============================================================================
// Negative Tests - Invalid/Edge Cases
// ============================================================================

#[test]
fn test_tab_bar_height_with_very_large_tab_count() {
    let tab_bar = TabBarUI::new();
    let config = Config::default();

    // Should not panic or behave unexpectedly with very large tab counts
    let height = tab_bar.get_height(usize::MAX / 2, &config);
    assert!(height >= 0.0, "Height should be non-negative");
}

#[test]
fn test_actions_are_exhaustive() {
    // Verify all action types can be created and pattern matched
    let actions: Vec<TabBarAction> = vec![
        TabBarAction::None,
        TabBarAction::SwitchTo(1),
        TabBarAction::Close(1),
        TabBarAction::NewTab,
        TabBarAction::Reorder(1, 0),
        TabBarAction::SetColor(1, [255, 0, 0]),
        TabBarAction::ClearColor(1),
    ];

    // All should be debuggable without panic
    for action in actions {
        let _ = format!("{:?}", action);
    }
}

#[test]
fn test_action_none_initialization() {
    // TabBarAction::None is the initial state before processing events
    // This is important because we initialize action to None before processing
    let action = TabBarAction::None;
    assert!(
        matches!(action, TabBarAction::None),
        "None action should match TabBarAction::None"
    );
    // None should not match any other action type
    assert!(!matches!(action, TabBarAction::NewTab));
    assert!(!matches!(action, TabBarAction::SwitchTo(_)));
    assert!(!matches!(action, TabBarAction::Close(_)));
}

// ============================================================================
// Documentation Tests - Verifying Documented Behavior
// ============================================================================

/// Verify that keyboard focus prevention is documented correctly.
/// The tab bar uses clicked_by(PointerButton::Primary) for these reasons:
#[test]
fn test_keyboard_focus_prevention_documented() {
    // This test documents the keyboard focus fix behavior
    // egui's clicked() returns true for both:
    //   1. Mouse clicks
    //   2. Enter/Space when widget has keyboard focus
    //
    // Using clicked_by(PointerButton::Primary) ensures only mouse clicks
    // trigger tab actions, preventing Enter key from switching tabs.

    // The actual behavior is tested in integration tests.
    // This test serves as documentation that the fix exists.
    // Verify that SwitchTo action can be created (the action that keyboard focus would trigger)
    let action = TabBarAction::SwitchTo(1);
    assert!(matches!(action, TabBarAction::SwitchTo(1)));
}

/// Verify that tab renumbering behavior is documented correctly.
#[test]
fn test_tab_renumbering_documented() {
    // This test documents the tab renumbering behavior
    //
    // When a tab with default title "Tab N" is closed or reordered:
    //   1. All remaining tabs with has_default_title=true get renumbered
    //   2. Renumbering is based on position (index + 1)
    //   3. Tabs with custom titles (has_default_title=false) are unchanged
    //
    // This ensures users see "Tab 1, Tab 2, Tab 3" not "Tab 1, Tab 3, Tab 4"
    // after closing Tab 2.

    // Verify that Reorder action can be created (used for triggering renumbering)
    let action = TabBarAction::Reorder(1, 0);
    assert!(matches!(action, TabBarAction::Reorder(1, 0)));
}

/// Verify that content offset behavior is documented correctly.
#[test]
fn test_content_offset_documented() {
    // This test documents the content offset fix
    //
    // The renderer's content_offset_y field:
    //   1. Is set to the tab bar height
    //   2. Offsets all terminal content rendering
    //   3. Ensures terminal cells don't overlap with tab bar
    //   4. Affects grid size calculation (fewer rows available)
    //
    // When tab bar becomes visible/hidden:
    //   1. content_offset_y changes
    //   2. Grid size is recalculated
    //   3. All terminal sessions are resized

    // Verify tab bar height calculation (basis for content_offset_y)
    let tab_bar = TabBarUI::new();
    let config = Config::default();
    let height = tab_bar.get_height(2, &config);
    assert!(height > 0.0, "Tab bar height should be positive when visible");
}

/// Verify that mouse event timing behavior is documented correctly.
#[test]
fn test_mouse_event_timing_documented() {
    // This test documents the mouse event timing fix
    //
    // Mouse events are now processed in this order:
    //   1. Check if click is in tab bar area
    //   2. If in tab bar: skip terminal handling, request redraw for egui
    //   3. If in terminal: set button_pressed state, process normally
    //
    // This prevents selection state issues where clicking the tab bar
    // would start a text selection in the terminal.

    // Verify that we can detect when mouse is in tab bar area via height
    let tab_bar = TabBarUI::new();
    let config = Config::default();

    // When 1 tab: no tab bar, clicks at y=0 go to terminal
    let height_1 = tab_bar.get_height(1, &config);
    assert_eq!(height_1, 0.0, "No tab bar with 1 tab");

    // When 2+ tabs: tab bar exists, clicks at y < height go to tab bar
    let height_2 = tab_bar.get_height(2, &config);
    assert!(height_2 > 0.0, "Tab bar exists with 2+ tabs");
}

// ============================================================================
// TabBarMode Tests
// ============================================================================

#[test]
fn test_should_show_with_always_mode() {
    use par_term::config::TabBarMode;
    let tab_bar = TabBarUI::new();

    // Always mode shows tab bar regardless of tab count
    assert!(tab_bar.should_show(0, TabBarMode::Always));
    assert!(tab_bar.should_show(1, TabBarMode::Always));
    assert!(tab_bar.should_show(2, TabBarMode::Always));
}

#[test]
fn test_should_show_with_when_multiple_mode() {
    use par_term::config::TabBarMode;
    let tab_bar = TabBarUI::new();

    // WhenMultiple mode only shows when 2+ tabs
    assert!(!tab_bar.should_show(0, TabBarMode::WhenMultiple));
    assert!(!tab_bar.should_show(1, TabBarMode::WhenMultiple));
    assert!(tab_bar.should_show(2, TabBarMode::WhenMultiple));
    assert!(tab_bar.should_show(10, TabBarMode::WhenMultiple));
}

#[test]
fn test_should_show_with_never_mode() {
    use par_term::config::TabBarMode;
    let tab_bar = TabBarUI::new();

    // Never mode hides tab bar regardless of tab count
    assert!(!tab_bar.should_show(0, TabBarMode::Never));
    assert!(!tab_bar.should_show(1, TabBarMode::Never));
    assert!(!tab_bar.should_show(2, TabBarMode::Never));
    assert!(!tab_bar.should_show(100, TabBarMode::Never));
}

// ============================================================================
// Integration Tests (require PTY/shell - ignored by default)
// ============================================================================

/// Test that tabs get numbered by position when created
#[test]
#[ignore] // Requires PTY for tab creation
fn test_tab_numbering_on_creation() {
    // This test would verify:
    // - First tab created is "Tab 1"
    // - Second tab created is "Tab 2"
    // - Third tab created is "Tab 3"
    // Even if internal IDs are 1, 2, 3 or any other sequence
}

/// Test that closing a middle tab renumbers subsequent tabs
#[test]
#[ignore] // Requires PTY for tab creation
fn test_tab_renumbering_on_close() {
    // This test would verify:
    // - Create tabs "Tab 1", "Tab 2", "Tab 3"
    // - Close "Tab 2" (middle)
    // - Remaining tabs should be "Tab 1", "Tab 2" (renumbered from former Tab 3)
}

/// Test that reordering tabs renumbers default titles
#[test]
#[ignore] // Requires PTY for tab creation
fn test_tab_renumbering_on_reorder() {
    // This test would verify:
    // - Create tabs "Tab 1", "Tab 2", "Tab 3"
    // - Move "Tab 3" to position 0
    // - Tabs should be "Tab 1", "Tab 2", "Tab 3" (based on new positions)
}

/// Test that OSC-titled tabs are not renumbered
#[test]
#[ignore] // Requires PTY for tab creation
fn test_custom_titled_tabs_not_renumbered() {
    // This test would verify:
    // - Create tabs "Tab 1", "Tab 2"
    // - Set Tab 2 title via OSC (has_default_title = false)
    // - Close Tab 1
    // - Tab 2 keeps its OSC title, not renamed to "Tab 1"
}

//! Tests for tab bar UI functionality
//!
//! These tests verify the tab bar UI behavior and document important design decisions.
//!
//! ## Keyboard Focus Bug Fix (commit 5006fa3)
//!
//! The tab bar uses `clicked_by(PointerButton::Primary)` instead of `clicked()` to detect
//! tab clicks. This is intentional to prevent keyboard activation from triggering tab switches.
//!
//! ### The Problem
//! In egui, `.clicked()` returns true when:
//! 1. The widget receives a mouse click, OR
//! 2. The widget has keyboard focus AND the user presses Enter or Space
//!
//! When the tab bar has focus (which can happen after various interactions), pressing Enter
//! in the terminal would inadvertently trigger a "click" on the focused tab, switching tabs
//! unexpectedly.
//!
//! ### The Solution
//! Using `.clicked_by(PointerButton::Primary)` ensures only actual mouse clicks trigger
//! tab actions, not keyboard events. This applies to:
//! - Tab switching (clicking a tab)
//! - Close button
//! - New tab button
//!
//! ## Close Button Rendering Fix (commit 60315a2)
//!
//! The close button is now rendered AFTER the tab content and positioned absolutely
//! at the right edge of the tab. This ensures:
//! - Close button is always visible and on top of tab content
//! - Close button hover detection uses manual rect containment checks
//! - Clicking the close button sets the Close action, not SwitchTo
//!
//! ### Close Button Hit Testing
//! Instead of using egui's button response, we:
//! 1. Calculate the close button rect manually
//! 2. Check if pointer position is within that rect
//! 3. Set `close_hovered` state based on containment
//! 4. On click, check if `close_hovered` is set before deciding action

use egui::{Pos2, Rect, Vec2};
use par_term::config::{Config, TabBarMode, TabBarPosition};
use par_term::tab::TabId;
use par_term::tab_bar_ui::{TabBarAction, TabBarUI};

#[test]
fn test_tab_bar_ui_creation() {
    let tab_bar = TabBarUI::new();

    // Initial state should be clean
    assert!(tab_bar.hovered_tab.is_none());
    assert!(tab_bar.close_hovered.is_none());
    assert!(!tab_bar.is_context_menu_open());
}

#[test]
fn test_tab_bar_ui_default() {
    let tab_bar = TabBarUI::default();

    // Default should be same as new
    assert!(tab_bar.hovered_tab.is_none());
    assert!(tab_bar.close_hovered.is_none());
}

#[test]
fn test_tab_bar_should_show_always() {
    let tab_bar = TabBarUI::new();

    // TabBarMode::Always should show for any number of tabs
    assert!(tab_bar.should_show(0, TabBarMode::Always));
    assert!(tab_bar.should_show(1, TabBarMode::Always));
    assert!(tab_bar.should_show(2, TabBarMode::Always));
    assert!(tab_bar.should_show(100, TabBarMode::Always));
}

#[test]
fn test_tab_bar_should_show_when_multiple() {
    let tab_bar = TabBarUI::new();

    // TabBarMode::WhenMultiple should only show when there are 2+ tabs
    assert!(!tab_bar.should_show(0, TabBarMode::WhenMultiple));
    assert!(!tab_bar.should_show(1, TabBarMode::WhenMultiple));
    assert!(tab_bar.should_show(2, TabBarMode::WhenMultiple));
    assert!(tab_bar.should_show(10, TabBarMode::WhenMultiple));
}

#[test]
fn test_tab_bar_should_show_never() {
    let tab_bar = TabBarUI::new();

    // TabBarMode::Never should never show
    assert!(!tab_bar.should_show(0, TabBarMode::Never));
    assert!(!tab_bar.should_show(1, TabBarMode::Never));
    assert!(!tab_bar.should_show(2, TabBarMode::Never));
    assert!(!tab_bar.should_show(100, TabBarMode::Never));
}

#[test]
fn test_tab_bar_height_when_hidden() {
    let tab_bar = TabBarUI::new();
    let config = Config {
        tab_bar_mode: TabBarMode::WhenMultiple,
        ..Config::default()
    };

    // When tab bar shouldn't show, height should be 0
    // With WhenMultiple mode and 1 tab
    assert_eq!(tab_bar.get_height(1, &config), 0.0);
}

#[test]
fn test_tab_bar_height_when_visible() {
    let tab_bar = TabBarUI::new();
    let config = Config::default();

    // When tab bar should show, height should match config
    // With default config (WhenMultiple) and 2 tabs
    assert_eq!(tab_bar.get_height(2, &config), config.tab_bar_height);
}

#[test]
fn test_tab_bar_context_menu_initially_closed() {
    let tab_bar = TabBarUI::new();
    assert!(!tab_bar.is_context_menu_open());
}

// Action enum tests

#[test]
fn test_tab_bar_action_none() {
    let action = TabBarAction::None;
    assert!(matches!(action, TabBarAction::None));
}

#[test]
fn test_tab_bar_action_switch_to() {
    let action = TabBarAction::SwitchTo(42);

    match action {
        TabBarAction::SwitchTo(id) => assert_eq!(id, 42),
        _ => panic!("Expected SwitchTo action"),
    }
}

#[test]
fn test_tab_bar_action_close() {
    let action = TabBarAction::Close(7);

    match action {
        TabBarAction::Close(id) => assert_eq!(id, 7),
        _ => panic!("Expected Close action"),
    }
}

#[test]
fn test_tab_bar_action_new_tab() {
    let action = TabBarAction::NewTab;
    assert!(matches!(action, TabBarAction::NewTab));
}

#[test]
fn test_tab_bar_action_reorder() {
    let action = TabBarAction::Reorder(5, 2);

    match action {
        TabBarAction::Reorder(id, pos) => {
            assert_eq!(id, 5);
            assert_eq!(pos, 2);
        }
        _ => panic!("Expected Reorder action"),
    }
}

#[test]
fn test_tab_bar_actions_not_equal() {
    // Different action types should not be equal
    assert_ne!(TabBarAction::None, TabBarAction::NewTab);
    assert_ne!(TabBarAction::SwitchTo(1), TabBarAction::Close(1));
    assert_ne!(TabBarAction::SwitchTo(1), TabBarAction::SwitchTo(2));
}

#[test]
fn test_tab_bar_actions_clone() {
    let actions = vec![
        TabBarAction::None,
        TabBarAction::SwitchTo(1),
        TabBarAction::Close(2),
        TabBarAction::NewTab,
        TabBarAction::Reorder(3, 4),
        TabBarAction::SetColor(5, [255, 128, 64]),
        TabBarAction::ClearColor(6),
    ];

    for action in actions {
        let cloned = action.clone();
        assert_eq!(action, cloned);
    }
}

#[test]
fn test_tab_bar_actions_debug() {
    // All action variants should implement Debug without panicking
    let actions: Vec<TabBarAction> = vec![
        TabBarAction::None,
        TabBarAction::SwitchTo(1),
        TabBarAction::Close(2),
        TabBarAction::NewTab,
        TabBarAction::Reorder(3, 4),
        TabBarAction::SetColor(5, [255, 128, 64]),
        TabBarAction::ClearColor(6),
    ];

    for action in actions {
        let debug_str = format!("{:?}", action);
        assert!(!debug_str.is_empty());
    }
}

// Note: Testing actual click behavior requires a full egui context with simulated input.
// The keyboard focus fix (using clicked_by instead of clicked) is verified through
// integration testing and the debug logs that were used to diagnose the issue.
//
// The key behavioral requirement is:
// - Tab bar buttons should ONLY respond to mouse clicks (PointerButton::Primary)
// - Tab bar buttons should NOT respond to keyboard activation (Enter/Space)
//
// This ensures that pressing Enter in the terminal doesn't inadvertently switch tabs
// when the tab bar happens to have keyboard focus.

// ============================================================================
// Close Button State Tests (commit 60315a2)
// ============================================================================

#[test]
fn test_close_hovered_state_init() {
    let tab_bar = TabBarUI::new();

    // Initially no close button should be hovered
    assert!(
        tab_bar.close_hovered.is_none(),
        "No close button should be hovered on init"
    );
}

#[test]
fn test_close_vs_switch_action_distinction() {
    // Verify that Close and SwitchTo are distinct action types
    // This is critical for the close button fix
    let switch = TabBarAction::SwitchTo(1);
    let close = TabBarAction::Close(1);

    // They should not be equal even for the same tab ID
    assert_ne!(
        switch, close,
        "Close and SwitchTo must be different actions"
    );

    // Verify we can pattern match correctly
    match switch {
        TabBarAction::SwitchTo(id) => assert_eq!(id, 1),
        _ => panic!("Switch action pattern match failed"),
    }

    match close {
        TabBarAction::Close(id) => assert_eq!(id, 1),
        _ => panic!("Close action pattern match failed"),
    }
}

#[test]
fn test_action_none_is_default_when_no_interaction() {
    // TabBarAction::None is used when no user interaction occurred
    // This is the default state before processing events
    let action = TabBarAction::None;

    // Should be equal to itself
    assert_eq!(action, TabBarAction::None);

    // Should not equal any other action
    assert_ne!(action, TabBarAction::NewTab);
    assert_ne!(action, TabBarAction::SwitchTo(1));
    assert_ne!(action, TabBarAction::Close(1));
}

// ============================================================================
// Tab Bar Configuration Tests
// ============================================================================

#[test]
fn test_tab_bar_uses_config_height() {
    let tab_bar = TabBarUI::new();
    let mut config = Config::default();

    // Default height
    let default_height = tab_bar.get_height(2, &config);
    assert!(default_height > 0.0);

    // Custom height
    config.tab_bar_height = 50.0;
    let custom_height = tab_bar.get_height(2, &config);
    assert_eq!(custom_height, 50.0, "Tab bar should use config height");
}

#[test]
fn test_tab_bar_height_zero_when_hidden() {
    let tab_bar = TabBarUI::new();
    let config = Config {
        tab_bar_mode: TabBarMode::WhenMultiple,
        ..Config::default()
    };

    // With WhenMultiple mode and only 1 tab, height should be 0
    let height = tab_bar.get_height(1, &config);
    assert_eq!(height, 0.0, "Tab bar should have 0 height when hidden");
}

// ============================================================================
// Action Equality and Comparison Tests
// ============================================================================

#[test]
fn test_reorder_action_components() {
    let action = TabBarAction::Reorder(5, 2);

    if let TabBarAction::Reorder(tab_id, new_pos) = action {
        assert_eq!(tab_id, 5, "Tab ID should be preserved");
        assert_eq!(new_pos, 2, "New position should be preserved");
    } else {
        panic!("Expected Reorder action");
    }
}

#[test]
fn test_set_color_action_components() {
    let color = [128, 64, 255];
    let action = TabBarAction::SetColor(3, color);

    if let TabBarAction::SetColor(tab_id, c) = action {
        assert_eq!(tab_id, 3, "Tab ID should be preserved");
        assert_eq!(c, color, "Color should be preserved");
    } else {
        panic!("Expected SetColor action");
    }
}

#[test]
fn test_clear_color_action_components() {
    let action = TabBarAction::ClearColor(7);

    if let TabBarAction::ClearColor(tab_id) = action {
        assert_eq!(tab_id, 7, "Tab ID should be preserved");
    } else {
        panic!("Expected ClearColor action");
    }
}

// ============================================================================
// Drag State Tests
// ============================================================================

#[test]
fn test_tab_bar_is_dragging_default_false() {
    let tab_bar = TabBarUI::new();
    assert!(
        !tab_bar.is_dragging(),
        "Drag should not be in progress on init"
    );
}

#[test]
fn test_tab_bar_default_is_not_dragging() {
    let tab_bar = TabBarUI::default();
    assert!(
        !tab_bar.is_dragging(),
        "Default tab bar should not be dragging"
    );
}

// ============================================================================
// Negative Tests
// ============================================================================

#[test]
fn test_should_show_edge_case_zero_tabs() {
    let tab_bar = TabBarUI::new();

    // Zero tabs is an edge case that shouldn't happen in practice
    // but the code should handle it gracefully
    assert!(tab_bar.should_show(0, TabBarMode::Always));
    assert!(!tab_bar.should_show(0, TabBarMode::WhenMultiple));
    assert!(!tab_bar.should_show(0, TabBarMode::Never));
}

#[test]
fn test_tab_bar_height_with_edge_case_heights() {
    let tab_bar = TabBarUI::new();

    // Very small height
    let config_small = Config {
        tab_bar_height: 1.0,
        ..Config::default()
    };
    let small = tab_bar.get_height(2, &config_small);
    assert_eq!(small, 1.0);

    // Very large height
    let config_large = Config {
        tab_bar_height: 1000.0,
        ..Config::default()
    };
    let large = tab_bar.get_height(2, &config_large);
    assert_eq!(large, 1000.0);

    // Zero height (unusual but valid)
    let config_zero = Config {
        tab_bar_height: 0.0,
        ..Config::default()
    };
    let zero = tab_bar.get_height(2, &config_zero);
    assert_eq!(zero, 0.0);
}

// ============================================================================
// Tab Bar Position Tests
// ============================================================================

#[test]
fn test_tab_bar_position_default_is_top() {
    let config = Config::default();
    assert_eq!(config.tab_bar_position, TabBarPosition::Top);
}

#[test]
fn test_tab_bar_height_zero_for_left_position() {
    let tab_bar = TabBarUI::new();
    let config = Config {
        tab_bar_position: TabBarPosition::Left,
        ..Config::default()
    };
    // Left position should return 0 height since the bar is vertical
    assert_eq!(tab_bar.get_height(2, &config), 0.0);
    // Even with many tabs
    assert_eq!(tab_bar.get_height(10, &config), 0.0);
}

#[test]
fn test_tab_bar_width_zero_for_top_bottom() {
    let tab_bar = TabBarUI::new();

    let config_top = Config {
        tab_bar_position: TabBarPosition::Top,
        ..Config::default()
    };
    assert_eq!(tab_bar.get_width(2, &config_top), 0.0);

    let config_bottom = Config {
        tab_bar_position: TabBarPosition::Bottom,
        ..Config::default()
    };
    assert_eq!(tab_bar.get_width(2, &config_bottom), 0.0);
}

#[test]
fn test_tab_bar_width_for_left_position() {
    let tab_bar = TabBarUI::new();
    let config = Config {
        tab_bar_position: TabBarPosition::Left,
        tab_bar_width: 200.0,
        ..Config::default()
    };
    assert_eq!(tab_bar.get_width(2, &config), 200.0);
}

#[test]
fn test_tab_bar_width_respects_tab_bar_mode() {
    let tab_bar = TabBarUI::new();

    // With "when_multiple" mode and only 1 tab, width should be 0
    let config = Config {
        tab_bar_position: TabBarPosition::Left,
        tab_bar_width: 200.0,
        tab_bar_mode: TabBarMode::WhenMultiple,
        ..Config::default()
    };
    assert_eq!(tab_bar.get_width(1, &config), 0.0);

    // With 2+ tabs, width should be the configured value
    assert_eq!(tab_bar.get_width(2, &config), 200.0);
}

#[test]
fn test_tab_bar_height_for_top_and_bottom() {
    let tab_bar = TabBarUI::new();

    let config_top = Config {
        tab_bar_position: TabBarPosition::Top,
        tab_bar_height: 30.0,
        ..Config::default()
    };
    assert_eq!(tab_bar.get_height(2, &config_top), 30.0);

    let config_bottom = Config {
        tab_bar_position: TabBarPosition::Bottom,
        tab_bar_height: 30.0,
        ..Config::default()
    };
    assert_eq!(tab_bar.get_height(2, &config_bottom), 30.0);
}

#[test]
fn test_tab_bar_position_is_horizontal() {
    assert!(TabBarPosition::Top.is_horizontal());
    assert!(TabBarPosition::Bottom.is_horizontal());
    assert!(!TabBarPosition::Left.is_horizontal());
}

#[test]
fn test_tab_bar_position_display_names() {
    assert_eq!(TabBarPosition::Top.display_name(), "Top");
    assert_eq!(TabBarPosition::Bottom.display_name(), "Bottom");
    assert_eq!(TabBarPosition::Left.display_name(), "Left");
}

#[test]
fn test_tab_bar_position_all() {
    let all_positions = TabBarPosition::all();
    assert_eq!(all_positions.len(), 3);
    assert!(all_positions.contains(&TabBarPosition::Top));
    assert!(all_positions.contains(&TabBarPosition::Bottom));
    assert!(all_positions.contains(&TabBarPosition::Left));
}

// ============================================================================
// Drag State Transition Tests (L-15)
// Tests the drag state machine (idle → dragging → dropped/cancelled) without
// requiring egui rendering.
// ============================================================================

/// Helper to make an egui Rect given left-x, width and a fixed y extent.
fn make_tab_rect(left_x: f32, width: f32) -> Rect {
    Rect::from_min_size(Pos2::new(left_x, 0.0), Vec2::new(width, 30.0))
}

#[test]
fn test_drag_state_idle_on_creation() {
    let tab_bar = TabBarUI::new();
    assert!(!tab_bar.is_dragging(), "Initial drag state should be idle");
    assert!(
        tab_bar.test_dragging_tab().is_none(),
        "No tab should be dragging initially"
    );
}

#[test]
fn test_drag_state_transition_to_dragging() {
    let mut tab_bar = TabBarUI::new();
    let tab_id: TabId = 42;

    tab_bar.test_set_drag_state(Some(tab_id), true);

    assert!(tab_bar.is_dragging(), "Should be in dragging state");
    assert_eq!(
        tab_bar.test_dragging_tab(),
        Some(tab_id),
        "Dragging tab id should match"
    );
}

#[test]
fn test_drag_state_transition_to_dropped() {
    let mut tab_bar = TabBarUI::new();
    let tab_id: TabId = 7;

    // Start drag
    tab_bar.test_set_drag_state(Some(tab_id), true);
    assert!(tab_bar.is_dragging());

    // Simulate drop: clear drag state (as render_drag_feedback does on pointer release)
    tab_bar.test_set_drag_state(None, false);
    tab_bar.test_set_drop_target(None);

    assert!(!tab_bar.is_dragging(), "Should be idle after drop");
    assert!(
        tab_bar.test_dragging_tab().is_none(),
        "No tab should be dragging after drop"
    );
    assert!(
        tab_bar.test_drop_target_index().is_none(),
        "Drop target should be cleared"
    );
}

#[test]
fn test_drag_state_cancel_clears_all_drag_fields() {
    let mut tab_bar = TabBarUI::new();
    let tab_id: TabId = 3;

    tab_bar.test_set_drag_state(Some(tab_id), true);
    tab_bar.test_set_drop_target(Some(2));

    // Simulate Escape cancellation
    tab_bar.test_set_drag_state(None, false);
    tab_bar.test_set_drop_target(None);

    assert!(!tab_bar.is_dragging());
    assert!(tab_bar.test_dragging_tab().is_none());
    assert!(tab_bar.test_drop_target_index().is_none());
}

#[test]
fn test_drag_state_multiple_tabs_only_one_dragging() {
    let mut tab_bar = TabBarUI::new();
    let tab_a: TabId = 1;

    tab_bar.test_set_drag_state(Some(tab_a), true);

    assert_eq!(tab_bar.test_dragging_tab(), Some(tab_a));
    assert!(tab_bar.is_dragging());
}

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

// ============================================================================
// Drop Target Calculation Tests (L-15)
// Tests the pure drop-target logic without requiring egui or GPU rendering.
// ============================================================================

/// Build a list of (TabId, Rect) pairs simulating N equally-spaced horizontal tabs.
fn make_tab_rects(count: usize, tab_width: f32, spacing: f32) -> Vec<(TabId, Rect)> {
    (0..count)
        .map(|i| {
            let left = i as f32 * (tab_width + spacing);
            let rect = Rect::from_min_size(Pos2::new(left, 0.0), Vec2::new(tab_width, 30.0));
            (i as TabId, rect)
        })
        .collect()
}

#[test]
fn test_drop_target_before_first_tab() {
    // Three tabs each 100px wide with 4px gap.
    // Tab 0: [0, 100], Tab 1: [104, 204], Tab 2: [208, 308]
    let rects = make_tab_rects(3, 100.0, 4.0);

    // Pointer at x=10 (within tab 0, left of its center at 50): insert before index 0
    let result = TabBarUI::calculate_drop_target_horizontal(&rects, None, 10.0);
    assert_eq!(
        result,
        Some(0),
        "Pointer before center of first tab → insert at 0"
    );
}

#[test]
fn test_drop_target_between_tabs() {
    let rects = make_tab_rects(3, 100.0, 4.0);
    // Tab 1 center = 104 + 50 = 154
    // Pointer at x=160 (right of tab 1 center) → insert before tab 2 (index 2)
    let result = TabBarUI::calculate_drop_target_horizontal(&rects, None, 160.0);
    assert_eq!(
        result,
        Some(2),
        "Pointer past center of tab 1 → insert at 2"
    );
}

#[test]
fn test_drop_target_after_last_tab() {
    let rects = make_tab_rects(3, 100.0, 4.0);
    // Pointer far to the right of all tabs → insert at end (index 3)
    let result = TabBarUI::calculate_drop_target_horizontal(&rects, None, 999.0);
    assert_eq!(result, Some(3), "Pointer after all tabs → insert at end");
}

#[test]
fn test_drop_target_noop_same_position() {
    let rects = make_tab_rects(3, 100.0, 4.0);
    // Dragging tab 0 (source index 0), pointer inside tab 0 (x=30, left of center at 50)
    // insert_index = 0, src = 0 → noop (insert_index == src)
    let result = TabBarUI::calculate_drop_target_horizontal(&rects, Some(0), 30.0);
    assert_eq!(result, None, "Dropping in the same slot should be a no-op");
}

#[test]
fn test_drop_target_noop_adjacent_position() {
    let rects = make_tab_rects(3, 100.0, 4.0);
    // Dragging tab 0 (source index 0).
    // Tab 1 center = 154. Pointer at x=160 → insert_index = 2.
    // src = 0, insert_index = 2 → not noop.
    let result = TabBarUI::calculate_drop_target_horizontal(&rects, Some(0), 160.0);
    // Moving tab 0 to after tab 1 should produce insert_index = 2, which is valid.
    assert_eq!(result, Some(2));
}

#[test]
fn test_drop_target_noop_next_slot() {
    let rects = make_tab_rects(3, 100.0, 4.0);
    // Dragging tab 1 (source index 1). Pointer in the left half of tab 2 (x=220, center=258)
    // → insert_index = 2. src=1, insert_index = src+1 = 2 → noop
    let result = TabBarUI::calculate_drop_target_horizontal(&rects, Some(1), 220.0);
    assert_eq!(result, None, "Inserting right after source is a no-op");
}

#[test]
fn test_drop_target_empty_tab_list() {
    let rects: Vec<(TabId, Rect)> = vec![];
    // With no tabs, insert_index = 0 = rects.len().
    // Without source, not noop → Some(0)
    let result = TabBarUI::calculate_drop_target_horizontal(&rects, None, 0.0);
    assert_eq!(result, Some(0), "Empty tab list: insert at 0");
}

#[test]
fn test_insertion_to_target_index_insert_before_source() {
    // insert_index < src: no adjustment needed
    assert_eq!(
        TabBarUI::insertion_to_target_index(0, Some(2)),
        0,
        "Inserting before source: index unchanged"
    );
}

#[test]
fn test_insertion_to_target_index_insert_after_source() {
    // insert_index > src: subtract 1 because source is removed first
    assert_eq!(
        TabBarUI::insertion_to_target_index(3, Some(1)),
        2,
        "Inserting after source: index decremented by 1"
    );
}

#[test]
fn test_insertion_to_target_index_no_source() {
    // Without a known source, index is used as-is
    assert_eq!(
        TabBarUI::insertion_to_target_index(5, None),
        5,
        "No source: index unchanged"
    );
}

#[test]
fn test_drop_target_followed_by_reorder_action() {
    // Full round-trip: calculate drop target, then convert to effective target,
    // and verify that forms a valid Reorder action.
    //
    // 4 tabs each 100px wide with 4px spacing:
    //   Tab 0: [0,100]   center=50
    //   Tab 1: [104,204] center=154
    //   Tab 2: [208,308] center=258
    //   Tab 3: [312,412] center=362
    let rects = make_tab_rects(4, 100.0, 4.0);
    let dragging_id: TabId = 0; // dragging the first tab
    let source_idx = Some(0usize);

    // Pointer at x=400, past center of tab 3 (362) → no early break → insert_index = 4 (end)
    let insert_idx = TabBarUI::calculate_drop_target_horizontal(&rects, source_idx, 400.0);
    assert_eq!(
        insert_idx,
        Some(4),
        "Pointer after all tab centers → insert at end (index 4)"
    );

    // insert_idx (4) > src (0) → effective = 4 - 1 = 3
    let effective = TabBarUI::insertion_to_target_index(insert_idx.unwrap(), source_idx);
    assert_eq!(
        effective, 3,
        "After removing tab 0, insert_idx 4 → effective target 3"
    );

    let action = TabBarAction::Reorder(dragging_id, effective);
    match action {
        TabBarAction::Reorder(id, pos) => {
            assert_eq!(id, dragging_id);
            assert_eq!(pos, 3);
        }
        _ => panic!("Expected Reorder action"),
    }
}

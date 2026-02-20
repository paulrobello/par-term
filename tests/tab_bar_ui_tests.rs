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

use par_term::config::{Config, TabBarMode, TabBarPosition};
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

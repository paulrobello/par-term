//! Integration tests for the ProfileDrawerUI component.
//!
//! Covers: initial state, toggle behavior, toggle button geometry (collapsed/expanded),
//! hit testing, selection/hover state, width adjustments, state consistency
//! across toggles, and ProfileDrawerAction variants.

use par_term::profile_drawer_ui::{ProfileDrawerAction, ProfileDrawerUI};
use uuid::Uuid;

// ============================================================================
// ProfileDrawerUI Tests
// ============================================================================

#[test]
fn test_profile_drawer_ui_creation() {
    let drawer = ProfileDrawerUI::new();

    // Initial state should be collapsed
    assert!(!drawer.expanded);
    assert!(drawer.selected.is_none());
    assert!(drawer.hovered.is_none());
    assert!(drawer.width > 0.0);
}

#[test]
fn test_profile_drawer_ui_default() {
    let drawer = ProfileDrawerUI::default();

    // Default should be same as new
    assert!(!drawer.expanded);
    assert!(drawer.selected.is_none());
    assert!(drawer.hovered.is_none());
}

#[test]
fn test_profile_drawer_toggle() {
    let mut drawer = ProfileDrawerUI::new();

    // Initially collapsed
    assert!(!drawer.expanded);

    // Toggle to expanded
    drawer.toggle();
    assert!(drawer.expanded);

    // Toggle back to collapsed
    drawer.toggle();
    assert!(!drawer.expanded);
}

#[test]
fn test_profile_drawer_toggle_button_rect_collapsed() {
    let drawer = ProfileDrawerUI::new();
    let window_width = 800.0;
    let window_height = 600.0;

    let (x, y, w, h) = drawer.get_toggle_button_rect(window_width, window_height);

    // When collapsed, button should be at right edge of window
    assert!(
        x > window_width - 20.0,
        "Button x should be near right edge"
    );
    assert!(x < window_width, "Button x should be within window");

    // Button should be vertically centered
    let expected_y = (window_height - h) / 2.0;
    assert!(
        (y - expected_y).abs() < 0.01,
        "Button should be vertically centered"
    );

    // Button should have positive dimensions
    assert!(w > 0.0);
    assert!(h > 0.0);
}

#[test]
fn test_profile_drawer_toggle_button_rect_expanded() {
    let mut drawer = ProfileDrawerUI::new();
    drawer.expanded = true;
    drawer.width = 220.0; // Default width

    let window_width = 800.0;
    let window_height = 600.0;

    let (x, y, w, h) = drawer.get_toggle_button_rect(window_width, window_height);

    // When expanded, button should be at left edge of drawer (right of content)
    let expected_x = window_width - drawer.width - w - 2.0;
    assert!(
        (x - expected_x).abs() < 0.01,
        "Button x should be at left edge of drawer"
    );

    // Button should be vertically centered
    let expected_y = (window_height - h) / 2.0;
    assert!(
        (y - expected_y).abs() < 0.01,
        "Button should be vertically centered"
    );
}

#[test]
fn test_profile_drawer_is_point_in_toggle_button() {
    let drawer = ProfileDrawerUI::new();
    let window_width = 800.0;
    let window_height = 600.0;

    let (x, y, w, h) = drawer.get_toggle_button_rect(window_width, window_height);

    // Point inside button
    let center_x = x + w / 2.0;
    let center_y = y + h / 2.0;
    assert!(drawer.is_point_in_toggle_button(center_x, center_y, window_width, window_height));

    // Point at top-left corner
    assert!(drawer.is_point_in_toggle_button(x, y, window_width, window_height));

    // Point at bottom-right corner
    assert!(drawer.is_point_in_toggle_button(x + w, y + h, window_width, window_height));

    // Point outside button (left of button)
    assert!(!drawer.is_point_in_toggle_button(x - 10.0, center_y, window_width, window_height));

    // Point outside button (above button)
    assert!(!drawer.is_point_in_toggle_button(center_x, y - 10.0, window_width, window_height));

    // Point outside button (below button)
    assert!(!drawer.is_point_in_toggle_button(center_x, y + h + 10.0, window_width, window_height));
}

#[test]
fn test_profile_drawer_is_point_in_toggle_button_expanded() {
    let mut drawer = ProfileDrawerUI::new();
    drawer.expanded = true;
    drawer.width = 220.0;

    let window_width = 800.0;
    let window_height = 600.0;

    let (x, y, w, h) = drawer.get_toggle_button_rect(window_width, window_height);

    // Point inside button when expanded
    let center_x = x + w / 2.0;
    let center_y = y + h / 2.0;
    assert!(drawer.is_point_in_toggle_button(center_x, center_y, window_width, window_height));

    // Point in the drawer area (not button)
    let drawer_x = window_width - drawer.width / 2.0;
    assert!(!drawer.is_point_in_toggle_button(drawer_x, center_y, window_width, window_height));
}

#[test]
fn test_profile_drawer_selection_state() {
    let mut drawer = ProfileDrawerUI::new();
    let profile_id = Uuid::new_v4();

    // Initially no selection
    assert!(drawer.selected.is_none());

    // Select a profile
    drawer.selected = Some(profile_id);
    assert_eq!(drawer.selected, Some(profile_id));

    // Clear selection
    drawer.selected = None;
    assert!(drawer.selected.is_none());
}

#[test]
fn test_profile_drawer_hover_state() {
    let mut drawer = ProfileDrawerUI::new();
    let profile_id = Uuid::new_v4();

    // Initially no hover
    assert!(drawer.hovered.is_none());

    // Hover a profile
    drawer.hovered = Some(profile_id);
    assert_eq!(drawer.hovered, Some(profile_id));

    // Clear hover
    drawer.hovered = None;
    assert!(drawer.hovered.is_none());
}

#[test]
fn test_profile_drawer_width_adjustment() {
    let mut drawer = ProfileDrawerUI::new();
    let initial_width = drawer.width;

    // Width should be adjustable
    drawer.width = 300.0;
    assert_eq!(drawer.width, 300.0);
    assert_ne!(drawer.width, initial_width);

    // Width affects toggle button position when expanded
    drawer.expanded = true;
    let (x1, _, _, _) = drawer.get_toggle_button_rect(800.0, 600.0);

    drawer.width = 400.0;
    let (x2, _, _, _) = drawer.get_toggle_button_rect(800.0, 600.0);

    // Wider drawer means button is further left
    assert!(x2 < x1);
}

// ============================================================================
// ProfileDrawerAction Tests
// ============================================================================

#[test]
fn test_profile_drawer_action_none() {
    let action = ProfileDrawerAction::None;
    assert!(matches!(action, ProfileDrawerAction::None));
}

#[test]
fn test_profile_drawer_action_open_profile() {
    let profile_id = Uuid::new_v4();
    let action = ProfileDrawerAction::OpenProfile(profile_id);

    match action {
        ProfileDrawerAction::OpenProfile(id) => assert_eq!(id, profile_id),
        _ => panic!("Expected OpenProfile action"),
    }
}

#[test]
fn test_profile_drawer_action_manage_profiles() {
    let action = ProfileDrawerAction::ManageProfiles;
    assert!(matches!(action, ProfileDrawerAction::ManageProfiles));
}

#[test]
fn test_profile_drawer_actions_equality() {
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    // Same type, same id
    assert_eq!(
        ProfileDrawerAction::OpenProfile(id1),
        ProfileDrawerAction::OpenProfile(id1)
    );

    // Same type, different id
    assert_ne!(
        ProfileDrawerAction::OpenProfile(id1),
        ProfileDrawerAction::OpenProfile(id2)
    );

    // Different types
    assert_ne!(
        ProfileDrawerAction::None,
        ProfileDrawerAction::ManageProfiles
    );
    assert_ne!(
        ProfileDrawerAction::OpenProfile(id1),
        ProfileDrawerAction::ManageProfiles
    );
}

#[test]
fn test_profile_drawer_actions_clone() {
    let id = Uuid::new_v4();
    let actions = vec![
        ProfileDrawerAction::None,
        ProfileDrawerAction::OpenProfile(id),
        ProfileDrawerAction::ManageProfiles,
    ];

    for action in actions {
        let cloned = action.clone();
        assert_eq!(action, cloned);
    }
}

#[test]
fn test_profile_drawer_actions_debug() {
    let id = Uuid::new_v4();
    let actions = vec![
        ProfileDrawerAction::None,
        ProfileDrawerAction::OpenProfile(id),
        ProfileDrawerAction::ManageProfiles,
    ];

    for action in actions {
        let debug_str = format!("{:?}", action);
        assert!(!debug_str.is_empty());
    }
}

// ============================================================================
// Toggle Button Geometry Edge Cases
// ============================================================================

#[test]
fn test_toggle_button_rect_small_window() {
    let drawer = ProfileDrawerUI::new();
    let window_width = 400.0;
    let window_height = 300.0;

    let (x, y, w, h) = drawer.get_toggle_button_rect(window_width, window_height);

    // Button should still be positioned correctly
    assert!(x >= 0.0);
    assert!(y >= 0.0);
    assert!(x + w <= window_width);
    assert!(y + h <= window_height);
}

#[test]
fn test_toggle_button_rect_large_window() {
    let drawer = ProfileDrawerUI::new();
    let window_width = 3840.0; // 4K
    let window_height = 2160.0;

    let (x, y, _w, h) = drawer.get_toggle_button_rect(window_width, window_height);

    // Button should be at right edge
    assert!(x > window_width - 50.0);

    // Button should be vertically centered
    let expected_y = (window_height - h) / 2.0;
    assert!((y - expected_y).abs() < 0.01);
}

#[test]
fn test_toggle_button_rect_with_various_drawer_widths() {
    let mut drawer = ProfileDrawerUI::new();
    drawer.expanded = true;

    let window_width = 800.0;
    let window_height = 600.0;

    // Test with minimum drawer width
    drawer.width = 180.0;
    let (x1, _, _, _) = drawer.get_toggle_button_rect(window_width, window_height);

    // Test with maximum drawer width
    drawer.width = 400.0;
    let (x2, _, _, _) = drawer.get_toggle_button_rect(window_width, window_height);

    // Wider drawer should move button further left
    assert!(x2 < x1);
}

// ============================================================================
// Drawer State Consistency Tests
// ============================================================================

#[test]
fn test_drawer_selection_persists_after_toggle() {
    let mut drawer = ProfileDrawerUI::new();
    let profile_id = Uuid::new_v4();

    drawer.selected = Some(profile_id);

    // Toggle drawer
    drawer.toggle();
    assert!(drawer.expanded);
    assert_eq!(drawer.selected, Some(profile_id));

    // Toggle back
    drawer.toggle();
    assert!(!drawer.expanded);
    assert_eq!(drawer.selected, Some(profile_id));
}

#[test]
fn test_drawer_hover_persists_after_toggle() {
    let mut drawer = ProfileDrawerUI::new();
    let profile_id = Uuid::new_v4();

    drawer.hovered = Some(profile_id);

    // Toggle drawer
    drawer.toggle();
    assert_eq!(drawer.hovered, Some(profile_id));
}

#[test]
fn test_drawer_width_persists_after_toggle() {
    let mut drawer = ProfileDrawerUI::new();
    drawer.width = 300.0;

    drawer.toggle();
    assert_eq!(drawer.width, 300.0);

    drawer.toggle();
    assert_eq!(drawer.width, 300.0);
}

#[test]
fn test_profile_drawer_default_width() {
    let drawer = ProfileDrawerUI::new();
    // Default width should be 220.0 (as defined in the struct)
    assert_eq!(drawer.width, 220.0);
}

#[test]
fn test_profile_drawer_minimum_width_constraint() {
    let mut drawer = ProfileDrawerUI::new();
    drawer.expanded = true;

    // Even with a very small width, toggle button should be calculable
    drawer.width = 50.0;
    let (x, _, _, _) = drawer.get_toggle_button_rect(800.0, 600.0);
    assert!(x >= 0.0);
}

#[test]
fn test_profile_drawer_clear_selection() {
    let mut drawer = ProfileDrawerUI::new();
    let id = Uuid::new_v4();

    drawer.selected = Some(id);
    assert!(drawer.selected.is_some());

    drawer.selected = None;
    assert!(drawer.selected.is_none());
}

#[test]
fn test_profile_drawer_multiple_toggles() {
    let mut drawer = ProfileDrawerUI::new();

    for i in 0..10 {
        drawer.toggle();
        assert_eq!(drawer.expanded, i % 2 == 0);
    }
}

#[test]
fn test_profile_drawer_action_all_variants() {
    let actions = vec![
        ProfileDrawerAction::None,
        ProfileDrawerAction::OpenProfile(Uuid::new_v4()),
        ProfileDrawerAction::ManageProfiles,
    ];

    // Ensure all variants can be matched
    for action in actions {
        match action {
            ProfileDrawerAction::None => {}
            ProfileDrawerAction::OpenProfile(_) => {}
            ProfileDrawerAction::ManageProfiles => {}
        }
    }
}

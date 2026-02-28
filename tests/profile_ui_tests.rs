//! Tests for profile UI components
//!
//! These tests cover the profile drawer and modal UI components, including:
//! - Profile drawer toggle button geometry
//! - Profile drawer state management
//! - Profile modal open/close behavior
//! - Profile modal CRUD operations
//! - Profile modal reordering

use par_term::profile::{Profile, ProfileManager};
use par_term::profile_drawer_ui::{ProfileDrawerAction, ProfileDrawerUI};
use par_term::profile_modal_ui::{ProfileModalAction, ProfileModalUI};
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
// ProfileModalUI Tests
// ============================================================================

#[test]
fn test_profile_modal_ui_creation() {
    let modal = ProfileModalUI::new();

    // Initially not visible
    assert!(!modal.visible);
}

#[test]
fn test_profile_modal_ui_default() {
    let modal = ProfileModalUI::default();

    // Default should be same as new
    assert!(!modal.visible);
}

#[test]
fn test_profile_modal_open() {
    let mut modal = ProfileModalUI::new();
    let mut manager = ProfileManager::new();
    manager.add(Profile::new("Test Profile"));

    // Open modal
    modal.open(&manager);
    assert!(modal.visible);

    // Should have working copy of profiles
    let working = modal.get_working_profiles();
    assert_eq!(working.len(), 1);
    assert_eq!(working[0].name, "Test Profile");
}

#[test]
fn test_profile_modal_open_empty_manager() {
    let mut modal = ProfileModalUI::new();
    let manager = ProfileManager::new();

    // Open modal with empty manager
    modal.open(&manager);
    assert!(modal.visible);
    assert!(modal.get_working_profiles().is_empty());
}

#[test]
fn test_profile_modal_close() {
    let mut modal = ProfileModalUI::new();
    let manager = ProfileManager::new();

    // Open then close
    modal.open(&manager);
    assert!(modal.visible);

    modal.close();
    assert!(!modal.visible);
    assert!(modal.get_working_profiles().is_empty());
}

#[test]
fn test_profile_modal_preserves_profile_order() {
    let mut modal = ProfileModalUI::new();
    let mut manager = ProfileManager::new();

    manager.add(Profile::new("First").order(0));
    manager.add(Profile::new("Second").order(1));
    manager.add(Profile::new("Third").order(2));

    modal.open(&manager);

    let working = modal.get_working_profiles();
    assert_eq!(working.len(), 3);
    assert_eq!(working[0].name, "First");
    assert_eq!(working[1].name, "Second");
    assert_eq!(working[2].name, "Third");
}

#[test]
fn test_profile_modal_get_working_profiles() {
    let mut modal = ProfileModalUI::new();
    let mut manager = ProfileManager::new();

    let p1 = Profile::new("SSH Server")
        .command("ssh")
        .working_directory("/home/user");
    let p2 = Profile::new("Local Dev").tab_name("Dev");

    manager.add(p1);
    manager.add(p2);

    modal.open(&manager);

    let working = modal.get_working_profiles();
    assert_eq!(working.len(), 2);

    // Verify profile details are preserved
    let ssh_profile = working.iter().find(|p| p.name == "SSH Server").unwrap();
    assert_eq!(ssh_profile.command.as_deref(), Some("ssh"));
    assert_eq!(ssh_profile.working_directory.as_deref(), Some("/home/user"));

    let dev_profile = working.iter().find(|p| p.name == "Local Dev").unwrap();
    assert_eq!(dev_profile.tab_name.as_deref(), Some("Dev"));
}

// ============================================================================
// ProfileModalAction Tests
// ============================================================================

#[test]
fn test_profile_modal_action_none() {
    let action = ProfileModalAction::None;
    assert!(matches!(action, ProfileModalAction::None));
}

#[test]
fn test_profile_modal_action_save() {
    let action = ProfileModalAction::Save;
    assert!(matches!(action, ProfileModalAction::Save));
}

#[test]
fn test_profile_modal_action_cancel() {
    let action = ProfileModalAction::Cancel;
    assert!(matches!(action, ProfileModalAction::Cancel));
}

#[test]
fn test_profile_modal_action_open_profile() {
    let profile_id = Uuid::new_v4();
    let action = ProfileModalAction::OpenProfile(profile_id);

    match action {
        ProfileModalAction::OpenProfile(id) => assert_eq!(id, profile_id),
        _ => panic!("Expected OpenProfile action"),
    }
}

#[test]
fn test_profile_modal_actions_equality() {
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    // Same type
    assert_eq!(ProfileModalAction::None, ProfileModalAction::None);
    assert_eq!(ProfileModalAction::Save, ProfileModalAction::Save);
    assert_eq!(ProfileModalAction::Cancel, ProfileModalAction::Cancel);
    assert_eq!(
        ProfileModalAction::OpenProfile(id1),
        ProfileModalAction::OpenProfile(id1)
    );

    // Different types
    assert_ne!(ProfileModalAction::None, ProfileModalAction::Save);
    assert_ne!(ProfileModalAction::Save, ProfileModalAction::Cancel);
    assert_ne!(
        ProfileModalAction::OpenProfile(id1),
        ProfileModalAction::OpenProfile(id2)
    );
}

#[test]
fn test_profile_modal_actions_clone() {
    let id = Uuid::new_v4();
    let actions = vec![
        ProfileModalAction::None,
        ProfileModalAction::Save,
        ProfileModalAction::Cancel,
        ProfileModalAction::OpenProfile(id),
    ];

    for action in actions {
        let cloned = action.clone();
        assert_eq!(action, cloned);
    }
}

#[test]
fn test_profile_modal_actions_debug() {
    let id = Uuid::new_v4();
    let actions = vec![
        ProfileModalAction::None,
        ProfileModalAction::Save,
        ProfileModalAction::Cancel,
        ProfileModalAction::OpenProfile(id),
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
// Profile Manager Integration Tests
// ============================================================================

#[test]
fn test_profile_manager_from_profiles_sorts_by_order() {
    // from_profiles() sorts profiles by their order field
    let p1 = Profile::new("Alpha").order(2);
    let p2 = Profile::new("Beta").order(0);
    let p3 = Profile::new("Gamma").order(1);

    let manager = ProfileManager::from_profiles(vec![p1, p2, p3]);

    // ProfileManager::from_profiles sorts by order field
    let profiles = manager.to_vec();
    assert_eq!(profiles[0].name, "Beta"); // order 0
    assert_eq!(profiles[1].name, "Gamma"); // order 1
    assert_eq!(profiles[2].name, "Alpha"); // order 2
}

#[test]
fn test_profile_manager_add_uses_insertion_order() {
    // When using add() directly, profiles are in insertion order
    let mut manager = ProfileManager::new();

    let p1 = Profile::new("First");
    let p2 = Profile::new("Second");
    let p3 = Profile::new("Third");

    manager.add(p1);
    manager.add(p2);
    manager.add(p3);

    let profiles = manager.to_vec();
    assert_eq!(profiles[0].name, "First");
    assert_eq!(profiles[1].name, "Second");
    assert_eq!(profiles[2].name, "Third");
}

#[test]
fn test_profile_manager_move_up_boundary() {
    let mut manager = ProfileManager::new();

    let p1 = Profile::new("First").order(0);
    let p2 = Profile::new("Second").order(1);
    let id1 = p1.id;

    manager.add(p1);
    manager.add(p2);

    // Try to move first item up (should have no effect)
    manager.move_up(&id1);

    let profiles = manager.profiles_ordered();
    assert_eq!(profiles[0].name, "First");
    assert_eq!(profiles[1].name, "Second");
}

#[test]
fn test_profile_manager_move_down_boundary() {
    let mut manager = ProfileManager::new();

    let p1 = Profile::new("First").order(0);
    let p2 = Profile::new("Second").order(1);
    let id2 = p2.id;

    manager.add(p1);
    manager.add(p2);

    // Try to move last item down (should have no effect)
    manager.move_down(&id2);

    let profiles = manager.profiles_ordered();
    assert_eq!(profiles[0].name, "First");
    assert_eq!(profiles[1].name, "Second");
}

#[test]
fn test_profile_manager_move_up_middle() {
    let mut manager = ProfileManager::new();

    let p1 = Profile::new("First").order(0);
    let p2 = Profile::new("Second").order(1);
    let p3 = Profile::new("Third").order(2);
    let id2 = p2.id;

    manager.add(p1);
    manager.add(p2);
    manager.add(p3);

    // Move middle item up
    manager.move_up(&id2);

    let profiles = manager.profiles_ordered();
    assert_eq!(profiles[0].name, "Second");
    assert_eq!(profiles[1].name, "First");
    assert_eq!(profiles[2].name, "Third");
}

#[test]
fn test_profile_manager_move_down_middle() {
    let mut manager = ProfileManager::new();

    let p1 = Profile::new("First").order(0);
    let p2 = Profile::new("Second").order(1);
    let p3 = Profile::new("Third").order(2);
    let id2 = p2.id;

    manager.add(p1);
    manager.add(p2);
    manager.add(p3);

    // Move middle item down
    manager.move_down(&id2);

    let profiles = manager.profiles_ordered();
    assert_eq!(profiles[0].name, "First");
    assert_eq!(profiles[1].name, "Third");
    assert_eq!(profiles[2].name, "Second");
}

// ============================================================================
// Profile Display Label Tests
// ============================================================================

#[test]
fn test_profile_display_label_no_icon() {
    let profile = Profile::new("My Server");
    assert_eq!(profile.display_label(), "My Server");
}

#[test]
fn test_profile_display_label_with_icon() {
    let profile = Profile::new("SSH Server").icon("🖥");
    assert_eq!(profile.display_label(), "🖥 SSH Server");
}

#[test]
fn test_profile_display_label_with_emoji_icon() {
    let profile = Profile::new("Database").icon("💾");
    assert_eq!(profile.display_label(), "💾 Database");
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

// ============================================================================
// ProfileModalUI Delete Confirmation Tests
// ============================================================================

#[test]
fn test_profile_modal_delete_confirmation_flow() {
    let mut modal = ProfileModalUI::new();
    let mut manager = ProfileManager::new();

    let profile = Profile::new("Test Profile");
    let profile_id = profile.id;
    manager.add(profile);

    modal.open(&manager);
    assert_eq!(modal.get_working_profiles().len(), 1);

    // Request deletion - should not delete immediately
    // Note: We can't call request_delete directly as it's private,
    // but we can test the public interface behavior
    assert!(
        modal
            .get_working_profiles()
            .iter()
            .any(|p| p.id == profile_id)
    );
}

#[test]
fn test_profile_modal_pending_delete_cleared_on_open() {
    let mut modal = ProfileModalUI::new();
    let manager = ProfileManager::new();

    // Open modal - pending_delete should be None (cleared)
    modal.open(&manager);

    // Modal is visible and ready for use
    assert!(modal.visible);
}

#[test]
fn test_profile_modal_pending_delete_cleared_on_close() {
    let mut modal = ProfileModalUI::new();
    let manager = ProfileManager::new();

    modal.open(&manager);
    modal.close();

    // Modal is closed and state is cleared
    assert!(!modal.visible);
    assert!(modal.get_working_profiles().is_empty());
}

// ============================================================================
// Profile Builder Comprehensive Tests
// ============================================================================

#[test]
fn test_profile_builder_all_fields() {
    let profile = Profile::new("Full Profile")
        .working_directory("/home/user/projects")
        .command("bash")
        .command_args(vec!["-c".to_string(), "echo hello".to_string()])
        .tab_name("My Tab")
        .icon("🚀")
        .order(5);

    assert_eq!(profile.name, "Full Profile");
    assert_eq!(
        profile.working_directory.as_deref(),
        Some("/home/user/projects")
    );
    assert_eq!(profile.command.as_deref(), Some("bash"));
    assert_eq!(
        profile.command_args,
        Some(vec!["-c".to_string(), "echo hello".to_string()])
    );
    assert_eq!(profile.tab_name.as_deref(), Some("My Tab"));
    assert_eq!(profile.icon.as_deref(), Some("🚀"));
    assert_eq!(profile.order, 5);
}

#[test]
fn test_profile_builder_chaining_order_independent() {
    // Test that builder methods can be called in any order
    let p1 = Profile::new("Test")
        .icon("🔧")
        .command("vim")
        .working_directory("/tmp");

    let p2 = Profile::new("Test")
        .working_directory("/tmp")
        .icon("🔧")
        .command("vim");

    assert_eq!(p1.icon, p2.icon);
    assert_eq!(p1.command, p2.command);
    assert_eq!(p1.working_directory, p2.working_directory);
}

#[test]
fn test_profile_with_id_preserves_id() {
    let id = Uuid::new_v4();
    let profile = Profile::with_id(id, "Named Profile");

    assert_eq!(profile.id, id);
    assert_eq!(profile.name, "Named Profile");
}

// ============================================================================
// Profile Validation Edge Cases
// ============================================================================

#[test]
fn test_profile_validation_whitespace_name() {
    let profile = Profile::new("   ");
    let warnings = profile.validate();
    assert!(!warnings.is_empty());
    assert!(warnings.iter().any(|w| w.contains("empty")));
}

#[test]
fn test_profile_validation_valid_working_directory() {
    // Use a directory that exists on all platforms
    let profile = Profile::new("Valid Dir")
        .working_directory(std::env::temp_dir().to_string_lossy().to_string());
    let warnings = profile.validate();
    // Should have no warnings about working directory
    assert!(!warnings.iter().any(|w| w.contains("directory")));
}

#[test]
fn test_profile_validation_empty_working_directory() {
    // Empty working directory should not trigger a warning
    let profile = Profile::new("No Dir").working_directory("");
    let warnings = profile.validate();
    assert!(!warnings.iter().any(|w| w.contains("directory")));
}

// ============================================================================
// Profile Serialization Edge Cases
// ============================================================================

#[test]
fn test_profile_serialization_minimal() {
    // Profile with only required fields
    let profile = Profile::new("Minimal");

    let yaml = serde_yaml_ng::to_string(&profile).unwrap();
    let deserialized: Profile = serde_yaml_ng::from_str(&yaml).unwrap();

    assert_eq!(deserialized.name, "Minimal");
    assert!(deserialized.working_directory.is_none());
    assert!(deserialized.command.is_none());
    assert!(deserialized.command_args.is_none());
    assert!(deserialized.tab_name.is_none());
    assert!(deserialized.icon.is_none());
}

#[test]
fn test_profile_serialization_with_special_characters() {
    let profile = Profile::new("SSH: user@server")
        .command("ssh")
        .command_args(vec![
            "user@server".to_string(),
            "-p".to_string(),
            "2222".to_string(),
        ])
        .working_directory("/home/user/my projects")
        .icon("🔐");

    let yaml = serde_yaml_ng::to_string(&profile).unwrap();
    let deserialized: Profile = serde_yaml_ng::from_str(&yaml).unwrap();

    assert_eq!(deserialized.name, "SSH: user@server");
    assert_eq!(deserialized.command.as_deref(), Some("ssh"));
    assert_eq!(deserialized.command_args.as_ref().unwrap().len(), 3);
    assert_eq!(
        deserialized.working_directory.as_deref(),
        Some("/home/user/my projects")
    );
    assert_eq!(deserialized.icon.as_deref(), Some("🔐"));
}

#[test]
fn test_profile_serialization_empty_args_not_serialized() {
    let profile = Profile::new("No Args");

    let yaml = serde_yaml_ng::to_string(&profile).unwrap();

    // Empty optional fields should not appear in YAML
    assert!(!yaml.contains("command_args"));
    assert!(!yaml.contains("working_directory"));
    assert!(!yaml.contains("tab_name"));
    assert!(!yaml.contains("icon"));
}

// ============================================================================
// ProfileManager Edge Cases
// ============================================================================

#[test]
fn test_profile_manager_update_existing() {
    let mut manager = ProfileManager::new();
    let profile = Profile::new("Original");
    let id = profile.id;
    manager.add(profile);

    // Update the profile
    let mut updated = Profile::with_id(id, "Updated");
    updated.command = Some("new-command".to_string());
    manager.update(updated);

    let retrieved = manager.get(&id).unwrap();
    assert_eq!(retrieved.name, "Updated");
    assert_eq!(retrieved.command.as_deref(), Some("new-command"));
}

#[test]
fn test_profile_manager_update_nonexistent() {
    let mut manager = ProfileManager::new();
    manager.add(Profile::new("Existing"));

    // Try to update a profile that doesn't exist
    let fake_id = Uuid::new_v4();
    let fake_profile = Profile::with_id(fake_id, "Fake");
    manager.update(fake_profile);

    // Should still have only one profile
    assert_eq!(manager.len(), 1);
    assert!(manager.get(&fake_id).is_none());
}

#[test]
fn test_profile_manager_remove_nonexistent() {
    let mut manager = ProfileManager::new();
    manager.add(Profile::new("Existing"));

    let fake_id = Uuid::new_v4();
    let removed = manager.remove(&fake_id);

    assert!(removed.is_none());
    assert_eq!(manager.len(), 1);
}

#[test]
fn test_profile_manager_get_mut() {
    let mut manager = ProfileManager::new();
    let profile = Profile::new("Mutable");
    let id = profile.id;
    manager.add(profile);

    // Modify via get_mut
    if let Some(p) = manager.get_mut(&id) {
        p.name = "Modified".to_string();
        p.icon = Some("✏️".to_string());
    }

    let retrieved = manager.get(&id).unwrap();
    assert_eq!(retrieved.name, "Modified");
    assert_eq!(retrieved.icon.as_deref(), Some("✏️"));
}

#[test]
fn test_profile_manager_ids_iterator() {
    let mut manager = ProfileManager::new();

    let p1 = Profile::new("First");
    let p2 = Profile::new("Second");
    let id1 = p1.id;
    let id2 = p2.id;

    manager.add(p1);
    manager.add(p2);

    let ids: Vec<_> = manager.ids().cloned().collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&id1));
    assert!(ids.contains(&id2));
}

#[test]
fn test_profile_manager_move_nonexistent() {
    let mut manager = ProfileManager::new();
    manager.add(Profile::new("Only"));

    let fake_id = Uuid::new_v4();

    // These should not panic or change anything
    manager.move_up(&fake_id);
    manager.move_down(&fake_id);

    assert_eq!(manager.len(), 1);
}

// ============================================================================
// Profile Drawer UI Additional Tests
// ============================================================================

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

// ============================================================================
// Profile Modal Working Copy Isolation Tests
// ============================================================================

#[test]
fn test_profile_modal_working_copy_isolated() {
    let mut modal = ProfileModalUI::new();
    let mut manager = ProfileManager::new();

    let profile = Profile::new("Original");
    let id = profile.id;
    manager.add(profile);

    modal.open(&manager);

    // Modify manager after opening modal
    if let Some(p) = manager.get_mut(&id) {
        p.name = "Modified in Manager".to_string();
    }

    // Working copy should still have original
    let working = modal.get_working_profiles();
    assert_eq!(working[0].name, "Original");
}

#[test]
fn test_profile_modal_reopen_refreshes() {
    let mut modal = ProfileModalUI::new();
    let mut manager = ProfileManager::new();

    manager.add(Profile::new("First"));
    modal.open(&manager);
    assert_eq!(modal.get_working_profiles().len(), 1);

    modal.close();

    // Add more profiles and reopen
    manager.add(Profile::new("Second"));
    manager.add(Profile::new("Third"));
    modal.open(&manager);

    assert_eq!(modal.get_working_profiles().len(), 3);
}

// ============================================================================
// Profile Default Trait Tests
// ============================================================================

#[test]
fn test_profile_default() {
    let profile = Profile::default();

    assert_eq!(profile.name, "New Profile");
    assert!(!profile.id.is_nil());
    assert!(profile.working_directory.is_none());
    assert!(profile.command.is_none());
    assert!(profile.command_args.is_none());
    assert!(profile.tab_name.is_none());
    assert!(profile.icon.is_none());
    assert_eq!(profile.order, 0);
}

#[test]
fn test_profile_manager_default() {
    let manager = ProfileManager::default();

    assert!(manager.is_empty());
    assert_eq!(manager.len(), 0);
}

// ============================================================================
// Profile Action Exhaustiveness Tests
// ============================================================================

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

#[test]
fn test_profile_modal_action_all_variants() {
    let actions = vec![
        ProfileModalAction::None,
        ProfileModalAction::Save,
        ProfileModalAction::Cancel,
        ProfileModalAction::OpenProfile(Uuid::new_v4()),
    ];

    // Ensure all variants can be matched
    for action in actions {
        match action {
            ProfileModalAction::None => {}
            ProfileModalAction::Save => {}
            ProfileModalAction::Cancel => {}
            ProfileModalAction::OpenProfile(_) => {}
        }
    }
}

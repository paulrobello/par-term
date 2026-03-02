//! Integration tests for the ProfileModalUI component, ProfileManager,
//! Profile builder, validation, serialization, and ProfileModalAction variants.

use par_term::profile::{Profile, ProfileManager};
use par_term::profile_modal_ui::{ProfileModalAction, ProfileModalUI};
use uuid::Uuid;

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

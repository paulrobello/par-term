//! Integration tests for the custom actions system.
//!
//! Covers: action type variants (ShellCommand, NewTab, InsertText, KeySequence),
//! keybinding accessors and serialization, action keybinding generation,
//! config persistence for actions, and key sequence parsing.

mod common;

use par_term::config::{Config, CustomActionConfig};
use par_term::keybindings::parser::parse_key_sequence;
use std::collections::HashMap;
use std::fs;

// ============================================================================
// Custom Action Type Tests
// ============================================================================

#[test]
fn test_custom_action_shell_command() {
    let action = CustomActionConfig::ShellCommand {
        id: "test_action".to_string(),
        title: "Test Action".to_string(),
        command: "echo".to_string(),
        args: vec!["hello".to_string()],
        notify_on_success: false,
        timeout_secs: 30,
        capture_output: false,
        keybinding: None,
        prefix_char: None,
        keybinding_enabled: true,
        description: None,
    };

    assert_eq!(action.id(), "test_action");
    assert_eq!(action.title(), "Test Action");
    assert!(action.is_shell_command());
    assert!(!action.is_insert_text());
    assert!(!action.is_key_sequence());
}

#[test]
fn test_custom_action_insert_text() {
    let action = CustomActionConfig::InsertText {
        id: "test_action".to_string(),
        title: "Test Action".to_string(),
        text: "echo 'test'".to_string(),
        variables: HashMap::new(),
        keybinding: None,
        prefix_char: None,
        keybinding_enabled: true,
        description: None,
    };

    assert_eq!(action.id(), "test_action");
    assert!(action.is_insert_text());
    assert!(!action.is_shell_command());
    assert!(!action.is_key_sequence());
}

#[test]
fn test_custom_action_new_tab() {
    let action = CustomActionConfig::NewTab {
        id: "test_action".to_string(),
        title: "Open Test Tab".to_string(),
        command: Some("cargo test".to_string()),
        keybinding: None,
        prefix_char: None,
        keybinding_enabled: true,
        description: None,
    };

    assert_eq!(action.id(), "test_action");
    assert!(action.is_new_tab());
    assert!(!action.is_shell_command());
    assert!(!action.is_insert_text());
    assert!(!action.is_key_sequence());
}

#[test]
fn test_custom_action_key_sequence() {
    let action = CustomActionConfig::KeySequence {
        id: "test_action".to_string(),
        title: "Test Action".to_string(),
        keys: "Ctrl+C".to_string(),
        keybinding: None,
        prefix_char: None,
        keybinding_enabled: true,
        description: None,
    };

    assert_eq!(action.id(), "test_action");
    assert!(action.is_key_sequence());
    assert!(!action.is_shell_command());
    assert!(!action.is_insert_text());
}

#[test]
fn test_action_serialization() {
    let action = CustomActionConfig::ShellCommand {
        id: "test".to_string(),
        title: "Test Action".to_string(),
        command: "npm".to_string(),
        args: vec!["test".to_string()],
        notify_on_success: true,
        timeout_secs: 30,
        capture_output: false,
        keybinding: Some("Ctrl+Shift+R".to_string()),
        prefix_char: Some('r'),
        keybinding_enabled: true,
        description: Some("Run tests".to_string()),
    };

    // Serialize
    let yaml = serde_yaml_ng::to_string(&action).unwrap();

    // Deserialize
    let deserialized: CustomActionConfig = serde_yaml_ng::from_str(&yaml).unwrap();

    assert_eq!(deserialized.id(), action.id());
    assert_eq!(deserialized.title(), action.title());
}

#[test]
fn test_action_types_serialization_roundtrip() {
    let actions = vec![
        CustomActionConfig::ShellCommand {
            id: "shell".to_string(),
            title: "Shell".to_string(),
            command: "echo".to_string(),
            args: vec!["test".to_string()],
            notify_on_success: false,
            timeout_secs: 30,
            capture_output: false,
            keybinding: Some("Ctrl+Shift+S".to_string()),
            prefix_char: None,
            keybinding_enabled: true,
            description: None,
        },
        CustomActionConfig::InsertText {
            id: "insert".to_string(),
            title: "Insert".to_string(),
            text: "hello".to_string(),
            variables: HashMap::new(),
            keybinding: None,
            prefix_char: None,
            keybinding_enabled: true,
            description: None,
        },
        CustomActionConfig::NewTab {
            id: "tab".to_string(),
            title: "Tab".to_string(),
            command: Some("lazygit".to_string()),
            keybinding: Some("Ctrl+Shift+T".to_string()),
            prefix_char: None,
            keybinding_enabled: true,
            description: None,
        },
        CustomActionConfig::KeySequence {
            id: "keys".to_string(),
            title: "Keys".to_string(),
            keys: "Ctrl+C".to_string(),
            keybinding: Some("Ctrl+Shift+K".to_string()),
            prefix_char: None,
            keybinding_enabled: false,
            description: None,
        },
    ];

    for action in actions {
        // Serialize
        let yaml = serde_yaml_ng::to_string(&action).unwrap();

        // Deserialize
        let deserialized: CustomActionConfig = serde_yaml_ng::from_str(&yaml).unwrap();

        assert_eq!(deserialized.id(), action.id());
        assert_eq!(deserialized.title(), action.title());
        assert_eq!(deserialized.is_shell_command(), action.is_shell_command());
        assert_eq!(deserialized.is_new_tab(), action.is_new_tab());
        assert_eq!(deserialized.is_insert_text(), action.is_insert_text());
        assert_eq!(deserialized.is_key_sequence(), action.is_key_sequence());
    }
}

#[test]
fn test_new_tab_command_optional_deserialization() {
    let yaml = r#"
type: new_tab
id: open_shell
title: Open Shell
keybinding_enabled: true
"#;

    let action: CustomActionConfig = serde_yaml_ng::from_str(yaml).unwrap();
    match action {
        CustomActionConfig::NewTab { command, .. } => assert_eq!(command, None),
        other => panic!("expected NewTab, got {:?}", other),
    }
}

// ============================================================================
// Action Keybinding Accessor Tests
// ============================================================================

#[test]
fn test_action_keybinding_accessors() {
    let mut action = CustomActionConfig::ShellCommand {
        id: "test".to_string(),
        title: "Test".to_string(),
        command: "echo".to_string(),
        args: vec![],
        notify_on_success: false,
        timeout_secs: 30,
        capture_output: false,
        keybinding: None,
        prefix_char: None,
        keybinding_enabled: true,
        description: None,
    };

    assert!(action.keybinding().is_none());
    assert!(action.keybinding_enabled());
    assert!(action.prefix_char().is_none());

    action.set_keybinding(Some("Ctrl+Shift+T".to_string()));
    assert_eq!(action.keybinding(), Some("Ctrl+Shift+T"));
    action.set_prefix_char(Some('g'));
    assert_eq!(action.prefix_char(), Some('g'));
    assert_eq!(action.normalized_prefix_char(), Some('g'));

    action.set_keybinding_enabled(false);
    assert!(!action.keybinding_enabled());
}

#[test]
fn test_action_keybinding_serialization_roundtrip() {
    let action = CustomActionConfig::ShellCommand {
        id: "test".to_string(),
        title: "Test".to_string(),
        command: "echo".to_string(),
        args: vec![],
        notify_on_success: false,
        timeout_secs: 30,
        capture_output: false,
        keybinding: Some("Ctrl+Shift+T".to_string()),
        prefix_char: Some('T'),
        keybinding_enabled: true,
        description: None,
    };

    // Serialize
    let yaml = serde_yaml_ng::to_string(&action).unwrap();
    assert!(yaml.contains("keybinding"));

    // Deserialize
    let deserialized: CustomActionConfig = serde_yaml_ng::from_str(&yaml).unwrap();
    assert_eq!(deserialized.keybinding(), Some("Ctrl+Shift+T"));
    assert_eq!(deserialized.prefix_char(), Some('T'));
    assert_eq!(deserialized.normalized_prefix_char(), Some('t'));
    assert!(deserialized.keybinding_enabled());
}

// ============================================================================
// Action Keybinding Generation Tests
// ============================================================================

#[test]
fn test_generate_action_keybindings() {
    let mut config = Config::default();
    let initial_count = config.keybindings.len();

    // Add action with keybinding
    config.actions.push(CustomActionConfig::ShellCommand {
        id: "run_tests".to_string(),
        title: "Run Tests".to_string(),
        command: "cargo".to_string(),
        args: vec!["test".to_string()],
        notify_on_success: false,
        timeout_secs: 30,
        capture_output: false,
        keybinding: Some("Ctrl+Shift+R".to_string()),
        prefix_char: None,
        keybinding_enabled: true,
        description: None,
    });

    // Generate keybindings
    config.generate_snippet_action_keybindings();

    // Check that keybinding was generated
    assert_eq!(config.keybindings.len(), initial_count + 1);
    assert_eq!(config.keybindings.last().unwrap().key, "Ctrl+Shift+R");
    assert_eq!(
        config.keybindings.last().unwrap().action,
        "action:run_tests"
    );
}

#[test]
fn test_generate_action_keybindings_no_duplicates() {
    let mut config = Config::default();

    config.actions.push(CustomActionConfig::ShellCommand {
        id: "run_tests".to_string(),
        title: "Run Tests".to_string(),
        command: "cargo".to_string(),
        args: vec!["test".to_string()],
        notify_on_success: false,
        timeout_secs: 30,
        capture_output: false,
        keybinding: Some("Ctrl+Shift+R".to_string()),
        prefix_char: None,
        keybinding_enabled: true,
        description: None,
    });

    // Generate keybindings twice
    config.generate_snippet_action_keybindings();
    let count_after_first = config.keybindings.len();

    config.generate_snippet_action_keybindings();
    let count_after_second = config.keybindings.len();

    // Should not add duplicates
    assert_eq!(count_after_first, count_after_second);
}

#[test]
fn test_generate_action_keybindings_disabled() {
    let mut config = Config::default();
    let initial_count = config.keybindings.len();

    // Add action with keybinding but disabled
    config.actions.push(CustomActionConfig::ShellCommand {
        id: "run_tests".to_string(),
        title: "Run Tests".to_string(),
        command: "cargo".to_string(),
        args: vec!["test".to_string()],
        notify_on_success: false,
        timeout_secs: 30,
        capture_output: false,
        keybinding: Some("Ctrl+Shift+R".to_string()),
        prefix_char: None,
        keybinding_enabled: false,
        description: None,
    });

    // Generate keybindings
    config.generate_snippet_action_keybindings();

    // Should not generate keybinding when keybinding_enabled is false
    assert_eq!(config.keybindings.len(), initial_count);
}

#[test]
fn test_generate_action_keybindings_update_existing() {
    let mut config = Config::default();

    config.actions.push(CustomActionConfig::ShellCommand {
        id: "run_tests".to_string(),
        title: "Run Tests".to_string(),
        command: "cargo".to_string(),
        args: vec!["test".to_string()],
        notify_on_success: false,
        timeout_secs: 30,
        capture_output: false,
        keybinding: Some("Ctrl+Shift+R".to_string()),
        prefix_char: None,
        keybinding_enabled: true,
        description: None,
    });

    // Generate keybindings first time
    config.generate_snippet_action_keybindings();
    assert_eq!(config.keybindings.last().unwrap().key, "Ctrl+Shift+R");

    // Update action keybinding
    config.actions[0].set_keybinding(Some("Ctrl+Shift+X".to_string()));

    // Generate keybindings again - should update existing
    config.generate_snippet_action_keybindings();

    let action_keybindings: Vec<_> = config
        .keybindings
        .iter()
        .filter(|kb| kb.action == "action:run_tests")
        .collect();

    assert_eq!(action_keybindings.len(), 1);
    assert_eq!(action_keybindings[0].key, "Ctrl+Shift+X");
}

#[test]
fn test_generate_action_keybindings_remove_when_cleared() {
    let mut config = Config::default();
    let initial_count = config.keybindings.len();

    config.actions.push(CustomActionConfig::ShellCommand {
        id: "run_tests".to_string(),
        title: "Run Tests".to_string(),
        command: "cargo".to_string(),
        args: vec!["test".to_string()],
        notify_on_success: false,
        timeout_secs: 30,
        capture_output: false,
        keybinding: Some("Ctrl+Shift+R".to_string()),
        prefix_char: None,
        keybinding_enabled: true,
        description: None,
    });

    // Generate keybindings
    config.generate_snippet_action_keybindings();
    assert_eq!(config.keybindings.len(), initial_count + 1);

    // Clear keybinding from action
    config.actions[0].set_keybinding(None);

    // Generate keybindings again - should remove the stale keybinding
    config.generate_snippet_action_keybindings();

    assert_eq!(config.keybindings.len(), initial_count);
    assert!(
        !config
            .keybindings
            .iter()
            .any(|kb| kb.action == "action:run_tests")
    );
}

// ============================================================================
// Config Persistence for Actions Tests
// ============================================================================

#[test]
fn test_config_persistence_actions() {
    let (_temp_dir, config_dir) = common::setup_config_dir();

    // Create config with actions
    let mut config = Config::default();
    config.actions.push(CustomActionConfig::ShellCommand {
        id: "test".to_string(),
        title: "Test".to_string(),
        command: "echo".to_string(),
        args: vec![],
        notify_on_success: false,
        timeout_secs: 30,
        capture_output: false,
        keybinding: None,
        prefix_char: Some('x'),
        keybinding_enabled: true,
        description: None,
    });

    // Save config
    let config_path = config_dir.join("config.yaml");
    let yaml = serde_yaml_ng::to_string(&config).unwrap();
    fs::write(&config_path, yaml).unwrap();

    // Load config
    let loaded_yaml = fs::read_to_string(&config_path).unwrap();
    let loaded_config: Config = serde_yaml_ng::from_str(&loaded_yaml).unwrap();

    assert_eq!(loaded_config.actions.len(), 1);
    assert_eq!(loaded_config.actions[0].id(), "test");
    assert_eq!(loaded_config.actions[0].title(), "Test");
}

// ============================================================================
// Key Sequence Parsing Tests
// ============================================================================

#[test]
fn test_key_sequence_parsing_single_key() {
    let seqs = parse_key_sequence("Enter").unwrap();
    assert_eq!(seqs.len(), 1);
    assert_eq!(seqs[0], b"\r");
}

#[test]
fn test_key_sequence_parsing_ctrl_combo() {
    let seqs = parse_key_sequence("Ctrl+C").unwrap();
    assert_eq!(seqs.len(), 1);
    assert_eq!(seqs[0], vec![0x03]); // ETX
}

#[test]
fn test_key_sequence_parsing_multi_keys() {
    let seqs = parse_key_sequence("Up Up Down Down").unwrap();
    assert_eq!(seqs.len(), 4);
    assert_eq!(seqs[0], b"\x1b[A");
    assert_eq!(seqs[1], b"\x1b[A");
    assert_eq!(seqs[2], b"\x1b[B");
    assert_eq!(seqs[3], b"\x1b[B");
}

#[test]
fn test_key_sequence_parsing_invalid() {
    assert!(parse_key_sequence("NotAKey").is_err());
    assert!(parse_key_sequence("").is_err());
}

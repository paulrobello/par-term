//! Tests for snippet_actions module.

use super::key_sequence::extract_prefix_action_char;
use super::workflow::glob_match;
use crate::config::snippets::CustomActionConfig;
use std::collections::HashMap;
use winit::keyboard::{Key, KeyCode, PhysicalKey};

#[test]
fn prefix_action_matching_is_case_insensitive_for_letters() {
    use super::prefix_action_for_char;

    let actions = vec![CustomActionConfig::InsertText {
        id: "git-status".to_string(),
        title: "Git Status".to_string(),
        text: "git status".to_string(),
        variables: HashMap::new(),
        keybinding: None,
        prefix_char: Some('G'),
        keybinding_enabled: true,
        description: None,
    }];

    assert_eq!(
        prefix_action_for_char(&actions, 'g'),
        Some("git-status".to_string())
    );
    assert_eq!(
        prefix_action_for_char(&actions, 'G'),
        Some("git-status".to_string())
    );
}

#[test]
fn prefix_action_matching_keeps_symbol_bindings_exact() {
    use super::prefix_action_for_char;

    let actions = vec![CustomActionConfig::KeySequence {
        id: "split".to_string(),
        title: "Split".to_string(),
        keys: "Ctrl+C".to_string(),
        keybinding: None,
        prefix_char: Some('%'),
        keybinding_enabled: true,
        description: None,
    }];

    assert_eq!(
        prefix_action_for_char(&actions, '%'),
        Some("split".to_string())
    );
    assert_eq!(prefix_action_for_char(&actions, '5'), None);
}

#[test]
fn extract_prefix_action_char_prefers_event_text() {
    assert_eq!(
        extract_prefix_action_char(
            Some("r"),
            &Key::Named(winit::keyboard::NamedKey::Enter),
            PhysicalKey::Code(KeyCode::KeyR),
        ),
        Some('r')
    );
}

#[test]
fn extract_prefix_action_char_falls_back_to_physical_key() {
    assert_eq!(
        extract_prefix_action_char(
            None,
            &Key::Named(winit::keyboard::NamedKey::Enter),
            PhysicalKey::Code(KeyCode::KeyR),
        ),
        Some('r')
    );
}

#[test]
fn test_glob_match_exact() {
    assert!(glob_match("main", "main"));
    assert!(!glob_match("main", "master"));
}

#[test]
fn test_glob_match_wildcard() {
    assert!(glob_match("feat/*", "feat/login"));
    assert!(glob_match("*", "anything"));
    assert!(!glob_match("feat/*", "fix/bug"));
}

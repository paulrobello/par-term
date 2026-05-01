//! Tests for snippet_actions module.

use super::key_sequence::extract_prefix_action_char;
use super::workflow::glob_match;
use crate::config::snippets::CustomActionConfig;
use std::collections::HashMap;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, KeyCode, KeyLocation, PhysicalKey};

fn make_key_event(logical_key: Key, physical_key: PhysicalKey, text: Option<&str>) -> KeyEvent {
    // SEC-006: Construct a KeyEvent for tests by setting each field individually.
    //
    // `KeyEvent` in winit 0.30.x does not expose a public constructor. We use
    // `std::mem::MaybeUninit` as a safe intermediate: the backing store is
    // allocated but not zeroed, and each field is written via `std::ptr::write`
    // before the value is read.
    //
    // SAFETY: `KeyEvent` comprises primitive types (bool), enums with a zero
    // discriminant (ElementState), Option<SmolStr>, Key, PhysicalKey, and
    // KeyLocation -- none of which are invalid when their bytes are set via
    // `std::ptr::write`. All six public fields are written before the value
    // is assumed-initialized via `assume_init()`. No raw pointers, NonNull,
    // or NonZero* fields exist in KeyEvent.
    //
    // If winit adds a public constructor in a future release, prefer that
    // over this workaround. See also winit 0.30.13 `KeyEvent` definition.
    unsafe {
        let mut event: std::mem::MaybeUninit<KeyEvent> = std::mem::MaybeUninit::uninit();
        let ptr = event.as_mut_ptr();
        std::ptr::write(std::ptr::addr_of_mut!((*ptr).physical_key), physical_key);
        std::ptr::write(std::ptr::addr_of_mut!((*ptr).logical_key), logical_key);
        std::ptr::write(std::ptr::addr_of_mut!((*ptr).text), text.map(Into::into));
        std::ptr::write(
            std::ptr::addr_of_mut!((*ptr).location),
            KeyLocation::Standard,
        );
        std::ptr::write(std::ptr::addr_of_mut!((*ptr).state), ElementState::Pressed);
        std::ptr::write(std::ptr::addr_of_mut!((*ptr).repeat), false);
        event.assume_init()
    }
}

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
    let event = make_key_event(
        Key::Named(winit::keyboard::NamedKey::Enter),
        PhysicalKey::Code(KeyCode::KeyR),
        Some("r"),
    );

    assert_eq!(extract_prefix_action_char(&event), Some('r'));
}

#[test]
fn extract_prefix_action_char_falls_back_to_physical_key() {
    let event = make_key_event(
        Key::Named(winit::keyboard::NamedKey::Enter),
        PhysicalKey::Code(KeyCode::KeyR),
        None,
    );

    assert_eq!(extract_prefix_action_char(&event), Some('r'));
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

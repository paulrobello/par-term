//! Integration tests for par-term-keybindings.
//!
//! These tests exercise the full parse → registry → lookup pipeline.
//! They test `KeybindingRegistry`, `parse_key_combo`, `key_combo_to_bytes`,
//! and `parse_key_sequence` as an integrated system.
//!
//! Tests that require constructing `winit::KeyEvent` (which has private fields)
//! are written by manually constructing `KeybindingMatcher` state through the
//! public parser API and the `#[cfg(test)]` visibility of struct fields — see
//! the matcher unit tests in `src/matcher.rs` for those lower-level cases.

use par_term_config::KeyBinding;
use par_term_keybindings::{
    KeyCombo,
    key_combo_to_bytes,
    parse_key_sequence,
    parser::{ParsedKey, parse_key_combo},
};
use winit::keyboard::NamedKey;

// ---------------------------------------------------------------------------
// Registry construction
// ---------------------------------------------------------------------------

#[test]
fn registry_empty_on_new() {
    let registry = par_term_keybindings::KeybindingRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn registry_from_empty_config() {
    let registry = par_term_keybindings::KeybindingRegistry::from_config(&[]);
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn registry_counts_valid_bindings() {
    let bindings = vec![
        KeyBinding {
            key: "Ctrl+A".to_string(),
            action: "action_a".to_string(),
        },
        KeyBinding {
            key: "Ctrl+B".to_string(),
            action: "action_b".to_string(),
        },
        KeyBinding {
            key: "Ctrl+C".to_string(),
            action: "action_c".to_string(),
        },
    ];

    let registry = par_term_keybindings::KeybindingRegistry::from_config(&bindings);
    assert_eq!(registry.len(), 3);
    assert!(!registry.is_empty());
}

#[test]
fn registry_skips_invalid_bindings() {
    let bindings = vec![
        KeyBinding {
            key: "Ctrl+A".to_string(),
            action: "valid_action".to_string(),
        },
        KeyBinding {
            key: "NotAKey".to_string(),
            action: "invalid_action".to_string(),
        },
        KeyBinding {
            key: "Ctrl+Shift".to_string(), // no key — ends with modifier
            action: "also_invalid".to_string(),
        },
        KeyBinding {
            key: "F5".to_string(),
            action: "another_valid".to_string(),
        },
    ];

    let registry = par_term_keybindings::KeybindingRegistry::from_config(&bindings);
    assert_eq!(registry.len(), 2); // only Ctrl+A and F5
}

#[test]
fn registry_skips_all_invalid() {
    let bindings = vec![
        KeyBinding {
            key: "BadKey1".to_string(),
            action: "action_a".to_string(),
        },
        KeyBinding {
            key: "BadKey2".to_string(),
            action: "action_b".to_string(),
        },
    ];

    let registry = par_term_keybindings::KeybindingRegistry::from_config(&bindings);
    assert!(registry.is_empty());
}

// ---------------------------------------------------------------------------
// Parser — modifier combinations
// ---------------------------------------------------------------------------

#[test]
fn parse_ctrl_only() {
    let combo = parse_key_combo("Ctrl+X").unwrap();
    assert!(combo.modifiers.ctrl);
    assert!(!combo.modifiers.alt);
    assert!(!combo.modifiers.shift);
    assert!(!combo.modifiers.super_key);
    assert_eq!(combo.key, ParsedKey::Character('X'));
}

#[test]
fn parse_alt_only() {
    let combo = parse_key_combo("Alt+X").unwrap();
    assert!(!combo.modifiers.ctrl);
    assert!(combo.modifiers.alt);
    assert!(!combo.modifiers.shift);
    assert!(!combo.modifiers.super_key);
}

#[test]
fn parse_shift_only() {
    let combo = parse_key_combo("Shift+X").unwrap();
    assert!(!combo.modifiers.ctrl);
    assert!(!combo.modifiers.alt);
    assert!(combo.modifiers.shift);
    assert!(!combo.modifiers.super_key);
}

#[test]
fn parse_super_only() {
    let combo = parse_key_combo("Super+X").unwrap();
    assert!(!combo.modifiers.ctrl);
    assert!(!combo.modifiers.alt);
    assert!(!combo.modifiers.shift);
    assert!(combo.modifiers.super_key);
}

#[test]
fn parse_all_modifiers() {
    let combo = parse_key_combo("Ctrl+Alt+Shift+Super+A").unwrap();
    assert!(combo.modifiers.ctrl);
    assert!(combo.modifiers.alt);
    assert!(combo.modifiers.shift);
    assert!(combo.modifiers.super_key);
    assert_eq!(combo.key, ParsedKey::Character('A'));
}

#[test]
fn parse_cmd_or_ctrl() {
    let combo = parse_key_combo("CmdOrCtrl+A").unwrap();
    assert!(combo.modifiers.cmd_or_ctrl);
    assert!(!combo.modifiers.ctrl);
    assert!(!combo.modifiers.super_key);
    assert_eq!(combo.key, ParsedKey::Character('A'));
}

// ---------------------------------------------------------------------------
// Parser — modifier aliases
// ---------------------------------------------------------------------------

#[test]
fn parse_control_alias() {
    let combo = parse_key_combo("Control+Z").unwrap();
    assert!(combo.modifiers.ctrl);
    assert_eq!(combo.key, ParsedKey::Character('Z'));
}

#[test]
fn parse_option_alias() {
    let combo = parse_key_combo("Option+Z").unwrap();
    assert!(combo.modifiers.alt);
    assert_eq!(combo.key, ParsedKey::Character('Z'));
}

#[test]
fn parse_cmd_alias() {
    let combo = parse_key_combo("Cmd+Z").unwrap();
    assert!(combo.modifiers.super_key);
}

#[test]
fn parse_command_alias() {
    let combo = parse_key_combo("Command+Z").unwrap();
    assert!(combo.modifiers.super_key);
}

#[test]
fn parse_meta_alias() {
    let combo = parse_key_combo("Meta+Z").unwrap();
    assert!(combo.modifiers.super_key);
}

#[test]
fn parse_win_alias() {
    let combo = parse_key_combo("Win+Z").unwrap();
    assert!(combo.modifiers.super_key);
}

// ---------------------------------------------------------------------------
// Parser — key aliases
// ---------------------------------------------------------------------------

#[test]
fn parse_return_as_enter() {
    let combo = parse_key_combo("Return").unwrap();
    assert_eq!(combo.key, ParsedKey::Named(NamedKey::Enter));
}

#[test]
fn parse_esc_alias() {
    let combo = parse_key_combo("Esc").unwrap();
    assert_eq!(combo.key, ParsedKey::Named(NamedKey::Escape));
}

#[test]
fn parse_del_alias() {
    let combo = parse_key_combo("Del").unwrap();
    assert_eq!(combo.key, ParsedKey::Named(NamedKey::Delete));
}

#[test]
fn parse_ins_alias() {
    let combo = parse_key_combo("Ins").unwrap();
    assert_eq!(combo.key, ParsedKey::Named(NamedKey::Insert));
}

#[test]
fn parse_pgup_alias() {
    let combo = parse_key_combo("PgUp").unwrap();
    assert_eq!(combo.key, ParsedKey::Named(NamedKey::PageUp));
}

#[test]
fn parse_pgdn_alias() {
    let combo = parse_key_combo("PgDn").unwrap();
    assert_eq!(combo.key, ParsedKey::Named(NamedKey::PageDown));
}

#[test]
fn parse_arrow_up_alias() {
    let combo = parse_key_combo("Up").unwrap();
    assert_eq!(combo.key, ParsedKey::Named(NamedKey::ArrowUp));
}

#[test]
fn parse_arrow_down_alias() {
    let combo = parse_key_combo("Down").unwrap();
    assert_eq!(combo.key, ParsedKey::Named(NamedKey::ArrowDown));
}

#[test]
fn parse_arrow_left_alias() {
    let combo = parse_key_combo("Left").unwrap();
    assert_eq!(combo.key, ParsedKey::Named(NamedKey::ArrowLeft));
}

#[test]
fn parse_arrow_right_alias() {
    let combo = parse_key_combo("Right").unwrap();
    assert_eq!(combo.key, ParsedKey::Named(NamedKey::ArrowRight));
}

// ---------------------------------------------------------------------------
// Parser — case insensitivity
// ---------------------------------------------------------------------------

#[test]
fn parse_ctrl_lowercase() {
    let combo = parse_key_combo("ctrl+a").unwrap();
    assert!(combo.modifiers.ctrl);
    // Characters are always uppercased
    assert_eq!(combo.key, ParsedKey::Character('A'));
}

#[test]
fn parse_mixed_case_modifiers() {
    let combo1 = parse_key_combo("CTRL+SHIFT+A").unwrap();
    let combo2 = parse_key_combo("ctrl+shift+a").unwrap();
    let combo3 = parse_key_combo("Ctrl+Shift+A").unwrap();
    assert_eq!(combo1, combo2);
    assert_eq!(combo2, combo3);
}

#[test]
fn parse_character_uppercase_normalized() {
    // Parser normalises characters to uppercase
    let combo_lower = parse_key_combo("Ctrl+a").unwrap();
    let combo_upper = parse_key_combo("Ctrl+A").unwrap();
    assert_eq!(combo_lower, combo_upper);
}

// ---------------------------------------------------------------------------
// Parser — all 12 function keys
// ---------------------------------------------------------------------------

#[test]
fn parse_all_function_keys() {
    let expected = [
        ("F1", NamedKey::F1),
        ("F2", NamedKey::F2),
        ("F3", NamedKey::F3),
        ("F4", NamedKey::F4),
        ("F5", NamedKey::F5),
        ("F6", NamedKey::F6),
        ("F7", NamedKey::F7),
        ("F8", NamedKey::F8),
        ("F9", NamedKey::F9),
        ("F10", NamedKey::F10),
        ("F11", NamedKey::F11),
        ("F12", NamedKey::F12),
    ];

    for (key_str, expected_named) in &expected {
        let combo = parse_key_combo(key_str)
            .unwrap_or_else(|_| panic!("Failed to parse {}", key_str));
        assert_eq!(
            combo.key,
            ParsedKey::Named(*expected_named),
            "Failed for {}",
            key_str
        );
        assert!(!combo.modifiers.ctrl);
    }
}

// ---------------------------------------------------------------------------
// Parser — physical key syntax
// ---------------------------------------------------------------------------

#[test]
fn parse_physical_key_bracket_syntax() {
    let combo = parse_key_combo("Ctrl+[KeyA]").unwrap();
    assert!(combo.modifiers.ctrl);
    assert!(matches!(combo.key, ParsedKey::Physical(_)));
}

#[test]
fn parse_physical_key_case_insensitive() {
    let upper = parse_key_combo("Ctrl+[KEYA]").unwrap();
    let lower = parse_key_combo("Ctrl+[keya]").unwrap();
    let mixed = parse_key_combo("Ctrl+[KeyA]").unwrap();
    assert_eq!(upper, lower);
    assert_eq!(lower, mixed);
}

#[test]
fn parse_physical_key_digit() {
    let combo = parse_key_combo("[Digit0]").unwrap();
    assert!(matches!(combo.key, ParsedKey::Physical(_)));
}

#[test]
fn parse_physical_key_navigation() {
    let up = parse_key_combo("[ArrowUp]").unwrap();
    let down = parse_key_combo("[ArrowDown]").unwrap();
    let left = parse_key_combo("[ArrowLeft]").unwrap();
    let right = parse_key_combo("[ArrowRight]").unwrap();
    // All should produce Physical variants
    assert!(matches!(up.key, ParsedKey::Physical(_)));
    assert!(matches!(down.key, ParsedKey::Physical(_)));
    assert!(matches!(left.key, ParsedKey::Physical(_)));
    assert!(matches!(right.key, ParsedKey::Physical(_)));
}

#[test]
fn parse_physical_key_unknown_fails() {
    assert!(parse_key_combo("Ctrl+[Banana]").is_err());
    assert!(parse_key_combo("[UnknownKey]").is_err());
}

// ---------------------------------------------------------------------------
// Parser — error cases
// ---------------------------------------------------------------------------

#[test]
fn parse_empty_string_is_error() {
    assert!(parse_key_combo("").is_err());
}

#[test]
fn parse_whitespace_only_is_error() {
    // Trimming leaves an empty string after split, or an unknown key
    let result = parse_key_combo("   ");
    assert!(result.is_err());
}

#[test]
fn parse_modifier_only_is_error() {
    assert!(parse_key_combo("Ctrl").is_err());
    assert!(parse_key_combo("Alt").is_err());
    assert!(parse_key_combo("Shift").is_err());
    assert!(parse_key_combo("Super").is_err());
}

#[test]
fn parse_modifier_chain_no_key_is_error() {
    assert!(parse_key_combo("Ctrl+Shift").is_err());
    assert!(parse_key_combo("Ctrl+Alt").is_err());
    assert!(parse_key_combo("Ctrl+Alt+Shift").is_err());
}

#[test]
fn parse_unknown_key_is_error() {
    assert!(parse_key_combo("Ctrl+Banana").is_err());
    assert!(parse_key_combo("FooKey").is_err());
    assert!(parse_key_combo("Ctrl+F13").is_err());
}

#[test]
fn parse_error_implements_display() {
    let err = parse_key_combo("").unwrap_err();
    let msg = err.to_string();
    assert!(!msg.is_empty());
}

// ---------------------------------------------------------------------------
// key_combo_to_bytes — control codes
// ---------------------------------------------------------------------------

#[test]
fn ctrl_a_through_z_produce_correct_control_codes() {
    // Ctrl+A = 0x01, Ctrl+B = 0x02, ..., Ctrl+Z = 0x1A
    let letters = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    for (i, letter) in letters.chars().enumerate() {
        let combo = parse_key_combo(&format!("Ctrl+{}", letter)).unwrap();
        let bytes = key_combo_to_bytes(&combo).unwrap();
        assert_eq!(bytes, vec![(i + 1) as u8], "Failed for Ctrl+{}", letter);
    }
}

#[test]
fn bytes_for_enter_is_carriage_return() {
    let combo = parse_key_combo("Enter").unwrap();
    assert_eq!(key_combo_to_bytes(&combo).unwrap(), b"\r");
}

#[test]
fn bytes_for_tab_is_horizontal_tab() {
    let combo = parse_key_combo("Tab").unwrap();
    assert_eq!(key_combo_to_bytes(&combo).unwrap(), b"\t");
}

#[test]
fn bytes_for_backspace_is_del() {
    let combo = parse_key_combo("Backspace").unwrap();
    assert_eq!(key_combo_to_bytes(&combo).unwrap(), b"\x7f");
}

#[test]
fn bytes_for_escape_is_esc() {
    let combo = parse_key_combo("Escape").unwrap();
    assert_eq!(key_combo_to_bytes(&combo).unwrap(), b"\x1b");
}

#[test]
fn bytes_for_space_is_space() {
    let combo = parse_key_combo("Space").unwrap();
    assert_eq!(key_combo_to_bytes(&combo).unwrap(), b" ");
}

#[test]
fn bytes_for_all_arrow_keys() {
    let cases = [
        ("Up", b"\x1b[A" as &[u8]),
        ("Down", b"\x1b[B"),
        ("Right", b"\x1b[C"),
        ("Left", b"\x1b[D"),
    ];
    for (key, expected) in &cases {
        let combo = parse_key_combo(key).unwrap();
        assert_eq!(&key_combo_to_bytes(&combo).unwrap(), expected, "Failed for {}", key);
    }
}

#[test]
fn bytes_for_f1_through_f4_use_ss3_prefix() {
    // F1-F4 use ESC O (SS3) prefix
    let cases = [
        ("F1", b"\x1bOP" as &[u8]),
        ("F2", b"\x1bOQ"),
        ("F3", b"\x1bOR"),
        ("F4", b"\x1bOS"),
    ];
    for (key, expected) in &cases {
        let combo = parse_key_combo(key).unwrap();
        assert_eq!(&key_combo_to_bytes(&combo).unwrap(), expected, "Failed for {}", key);
    }
}

#[test]
fn bytes_for_f5_through_f12_use_csi_prefix() {
    // F5-F12 use ESC [ ... ~ format
    let cases = [
        ("F5", b"\x1b[15~" as &[u8]),
        ("F6", b"\x1b[17~"),
        ("F7", b"\x1b[18~"),
        ("F8", b"\x1b[19~"),
        ("F9", b"\x1b[20~"),
        ("F10", b"\x1b[21~"),
        ("F11", b"\x1b[23~"),
        ("F12", b"\x1b[24~"),
    ];
    for (key, expected) in &cases {
        let combo = parse_key_combo(key).unwrap();
        assert_eq!(&key_combo_to_bytes(&combo).unwrap(), expected, "Failed for {}", key);
    }
}

#[test]
fn bytes_for_navigation_keys() {
    let cases = [
        ("Home", b"\x1b[H" as &[u8]),
        ("End", b"\x1b[F"),
        ("PageUp", b"\x1b[5~"),
        ("PageDown", b"\x1b[6~"),
        ("Insert", b"\x1b[2~"),
        ("Delete", b"\x1b[3~"),
    ];
    for (key, expected) in &cases {
        let combo = parse_key_combo(key).unwrap();
        assert_eq!(&key_combo_to_bytes(&combo).unwrap(), expected, "Failed for {}", key);
    }
}

#[test]
fn bytes_for_alt_key_adds_esc_prefix() {
    let combo = parse_key_combo("Alt+A").unwrap();
    let bytes = key_combo_to_bytes(&combo).unwrap();
    assert_eq!(bytes, vec![0x1b, b'A']);
}

#[test]
fn bytes_for_alt_ctrl_key_adds_esc_before_control_code() {
    let combo = parse_key_combo("Alt+Ctrl+A").unwrap();
    let bytes = key_combo_to_bytes(&combo).unwrap();
    assert_eq!(bytes, vec![0x1b, 0x01]); // ESC + Ctrl+A code
}

#[test]
fn bytes_for_alt_named_key_adds_esc_prefix() {
    let combo = parse_key_combo("Alt+F5").unwrap();
    let bytes = key_combo_to_bytes(&combo).unwrap();
    // Alt+F5 = ESC + F5-sequence
    assert_eq!(bytes[0], 0x1b);
    assert!(bytes.len() > 1);
}

#[test]
fn bytes_for_plain_char() {
    let combo = parse_key_combo("A").unwrap();
    let bytes = key_combo_to_bytes(&combo).unwrap();
    assert_eq!(bytes, b"A");
}

#[test]
fn bytes_for_physical_key_is_error() {
    let combo = parse_key_combo("Ctrl+[KeyZ]").unwrap();
    assert!(key_combo_to_bytes(&combo).is_err());
}

// ---------------------------------------------------------------------------
// parse_key_sequence — multi-key sequences
// ---------------------------------------------------------------------------

#[test]
fn key_sequence_empty_is_error() {
    assert!(parse_key_sequence("").is_err());
    assert!(parse_key_sequence("   ").is_err());
}

#[test]
fn key_sequence_single_key() {
    let seqs = parse_key_sequence("Enter").unwrap();
    assert_eq!(seqs.len(), 1);
    assert_eq!(seqs[0], b"\r");
}

#[test]
fn key_sequence_multiple_keys() {
    let seqs = parse_key_sequence("Up Down Left Right").unwrap();
    assert_eq!(seqs.len(), 4);
    assert_eq!(seqs[0], b"\x1b[A");
    assert_eq!(seqs[1], b"\x1b[B");
    assert_eq!(seqs[2], b"\x1b[D");
    assert_eq!(seqs[3], b"\x1b[C");
}

#[test]
fn key_sequence_konami_code() {
    let seqs = parse_key_sequence("Up Up Down Down Left Right Left Right B A").unwrap();
    assert_eq!(seqs.len(), 10);
    assert_eq!(seqs[0], b"\x1b[A"); // Up
    assert_eq!(seqs[1], b"\x1b[A"); // Up
    assert_eq!(seqs[2], b"\x1b[B"); // Down
    assert_eq!(seqs[3], b"\x1b[B"); // Down
    assert_eq!(seqs[4], b"\x1b[D"); // Left
    assert_eq!(seqs[5], b"\x1b[C"); // Right
    assert_eq!(seqs[6], b"\x1b[D"); // Left
    assert_eq!(seqs[7], b"\x1b[C"); // Right
    assert_eq!(seqs[8], b"B");
    assert_eq!(seqs[9], b"A");
}

#[test]
fn key_sequence_ctrl_keys() {
    let seqs = parse_key_sequence("Ctrl+C Ctrl+Z").unwrap();
    assert_eq!(seqs.len(), 2);
    assert_eq!(seqs[0], vec![0x03]); // ETX
    assert_eq!(seqs[1], vec![0x1a]); // SUB
}

#[test]
fn key_sequence_invalid_key_is_error() {
    assert!(parse_key_sequence("Up BananaKey Down").is_err());
}

#[test]
fn key_sequence_physical_key_is_error() {
    // Physical keys cannot be encoded to bytes
    assert!(parse_key_sequence("Ctrl+[KeyZ]").is_err());
}

#[test]
fn key_sequence_extra_whitespace_trimmed() {
    let seqs = parse_key_sequence("  Enter  Tab  ").unwrap();
    assert_eq!(seqs.len(), 2);
    assert_eq!(seqs[0], b"\r");
    assert_eq!(seqs[1], b"\t");
}

// ---------------------------------------------------------------------------
// KeyCombo display formatting
// ---------------------------------------------------------------------------

#[test]
fn combo_display_ctrl_shift_b() {
    let combo = parse_key_combo("Ctrl+Shift+B").unwrap();
    let s = format!("{}", combo);
    assert!(s.contains("Ctrl"));
    assert!(s.contains("Shift"));
    assert!(s.contains('B'));
}

#[test]
fn combo_display_physical_key() {
    let combo = parse_key_combo("Ctrl+[KeyZ]").unwrap();
    let s = format!("{}", combo);
    assert!(s.contains("Ctrl"));
    assert!(s.contains("[KeyZ]"));
}

#[test]
fn combo_display_cmd_or_ctrl() {
    let combo = parse_key_combo("CmdOrCtrl+A").unwrap();
    let s = format!("{}", combo);
    assert!(s.contains("CmdOrCtrl"));
}

#[test]
fn combo_display_named_key() {
    let combo = parse_key_combo("Escape").unwrap();
    let s = format!("{}", combo);
    assert!(s.contains("Escape"));
}

// ---------------------------------------------------------------------------
// KeyCombo hash/equality (HashMap key correctness)
// ---------------------------------------------------------------------------

#[test]
fn combo_equality_same_key_same_modifiers() {
    let a = parse_key_combo("Ctrl+Shift+A").unwrap();
    let b = parse_key_combo("Ctrl+Shift+A").unwrap();
    assert_eq!(a, b);
}

#[test]
fn combo_inequality_different_modifiers() {
    let a = parse_key_combo("Ctrl+A").unwrap();
    let b = parse_key_combo("Alt+A").unwrap();
    assert_ne!(a, b);
}

#[test]
fn combo_inequality_different_keys() {
    let a = parse_key_combo("Ctrl+A").unwrap();
    let b = parse_key_combo("Ctrl+B").unwrap();
    assert_ne!(a, b);
}

#[test]
fn combo_usable_as_hashmap_key() {
    use std::collections::HashMap;
    let mut map: HashMap<KeyCombo, &str> = HashMap::new();
    let combo = parse_key_combo("Ctrl+Shift+B").unwrap();
    map.insert(combo.clone(), "action");
    assert_eq!(map.get(&combo), Some(&"action"));
    let other = parse_key_combo("Ctrl+Shift+C").unwrap();
    assert_eq!(map.get(&other), None);
}

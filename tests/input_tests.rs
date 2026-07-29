// Input handler tests
//
// Covers handler construction and Option-key state, then the VT byte-sequence
// encoding itself via `KeyInput` — see the note above the encoding tests for why
// these do not go through `winit::event::KeyEvent`.

use par_term::config::OptionKeyMode;
use par_term::input::InputHandler;

#[test]
fn test_input_handler_creation() {
    // Test that we can create an InputHandler
    let _handler = InputHandler::new();
    // If we get here, the handler was created successfully
}

#[test]
fn test_input_handler_default() {
    // Test that we can create an InputHandler using Default
    let _handler = InputHandler::default();
    // If we get here, the handler was created successfully
}

#[test]
fn test_option_key_mode_default() {
    // Test that OptionKeyMode defaults to Esc (most compatible for terminal use)
    let handler = InputHandler::new();
    assert_eq!(handler.left_option_key_mode, OptionKeyMode::Esc);
    assert_eq!(handler.right_option_key_mode, OptionKeyMode::Esc);
}

#[test]
fn test_update_option_key_modes() {
    // Test that we can update the Option key modes
    let mut handler = InputHandler::new();

    // Update to different modes
    handler.update_option_key_modes(OptionKeyMode::Normal, OptionKeyMode::Meta);

    assert_eq!(handler.left_option_key_mode, OptionKeyMode::Normal);
    assert_eq!(handler.right_option_key_mode, OptionKeyMode::Meta);
}

#[test]
fn test_option_key_mode_variants() {
    // Test that all OptionKeyMode variants are distinct
    assert_ne!(OptionKeyMode::Normal, OptionKeyMode::Meta);
    assert_ne!(OptionKeyMode::Normal, OptionKeyMode::Esc);
    assert_ne!(OptionKeyMode::Meta, OptionKeyMode::Esc);

    // Test that same variant equals itself
    assert_eq!(OptionKeyMode::Normal, OptionKeyMode::Normal);
    assert_eq!(OptionKeyMode::Meta, OptionKeyMode::Meta);
    assert_eq!(OptionKeyMode::Esc, OptionKeyMode::Esc);
}

#[test]
fn test_option_key_mode_serde() {
    // Test serialization/deserialization of OptionKeyMode using YAML
    let modes = [
        (OptionKeyMode::Normal, "normal"),
        (OptionKeyMode::Meta, "meta"),
        (OptionKeyMode::Esc, "esc"),
    ];

    for (mode, expected_yaml) in modes {
        // Test serialization
        let yaml = serde_yaml_ng::to_string(&mode).unwrap();
        assert_eq!(yaml.trim(), expected_yaml);

        // Test deserialization
        let deserialized: OptionKeyMode = serde_yaml_ng::from_str(expected_yaml).unwrap();
        assert_eq!(deserialized, mode);
    }
}

// Key encoding tests.
//
// These build a `KeyInput` rather than a `winit::event::KeyEvent`. `KeyEvent`
// has a private platform-specific field and no public constructor, so any test
// that fabricates one leaves that field uninitialized — which is undefined
// behaviour, and which segfaulted the lib test binary on Linux CI until 53705aaf.
// `KeyInput` carries exactly the three fields encoding reads, and is safe to
// construct.

use par_term::input::KeyInput;
use winit::event::{ElementState, Modifiers};
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};

fn key_input(logical_key: Key, physical_key: PhysicalKey) -> KeyInput {
    KeyInput {
        logical_key,
        physical_key,
        state: ElementState::Pressed,
    }
}

#[test]
fn test_tab_sends_horizontal_tab() {
    let mut handler = InputHandler::new();
    // No modifiers — plain Tab
    handler.update_modifiers(Modifiers::default());

    let event = key_input(Key::Named(NamedKey::Tab), PhysicalKey::Code(KeyCode::Tab));

    let result = handler.handle_key_input_with_mode(&event, 0, false);
    assert_eq!(result, Some(vec![b'\t']), "Tab should send HT (0x09)");
}

#[test]
fn test_shift_tab_sends_csi_z() {
    let mut handler = InputHandler::new();
    // Set Shift modifier
    handler.update_modifiers(Modifiers::from(ModifiersState::SHIFT));

    let event = key_input(Key::Named(NamedKey::Tab), PhysicalKey::Code(KeyCode::Tab));

    let result = handler.handle_key_input_with_mode(&event, 0, false);
    assert_eq!(
        result,
        Some(b"\x1b[Z".to_vec()),
        "Shift+Tab should send CSI Z (reverse tab / backtab)"
    );
}

#[test]
fn test_enter_sends_cr() {
    let mut handler = InputHandler::new();
    handler.update_modifiers(Modifiers::default());

    let event = key_input(
        Key::Named(NamedKey::Enter),
        PhysicalKey::Code(KeyCode::Enter),
    );

    let result = handler.handle_key_input_with_mode(&event, 0, false);
    assert_eq!(result, Some(vec![b'\r']), "Enter should send CR (0x0d)");
}

#[test]
fn test_shift_enter_sends_lf() {
    let mut handler = InputHandler::new();
    handler.update_modifiers(Modifiers::from(ModifiersState::SHIFT));

    let event = key_input(
        Key::Named(NamedKey::Enter),
        PhysicalKey::Code(KeyCode::Enter),
    );

    let result = handler.handle_key_input_with_mode(&event, 0, false);
    assert_eq!(
        result,
        Some(vec![b'\n']),
        "Shift+Enter should send LF (0x0a)"
    );
}

#[test]
fn test_escape_sends_escape() {
    let mut handler = InputHandler::new();
    handler.update_modifiers(Modifiers::default());

    let event = key_input(
        Key::Named(NamedKey::Escape),
        PhysicalKey::Code(KeyCode::Escape),
    );

    let result = handler.handle_key_input_with_mode(&event, 0, false);
    assert_eq!(result, Some(vec![0x1b]), "Escape should send ESC (0x1b)");
}

// -----------------------------------------------------------------------------
// modifyOtherKeys mode 2 — Shift-only exemption (matches iTerm2's mapper)
// -----------------------------------------------------------------------------
//
// These tests pin the fix for the bug where Claude Code (outside tmux) was
// receiving `1` instead of `!` when the user pressed Shift+1. iTerm2's reference
// implementation in iTermModifyOtherKeysMapper.m routes any Shift-only printable
// through the OS text-input layer (no modifyOtherKeys encoding), and we match
// that rule — see par-term-input/src/lib.rs::try_modify_other_keys_encoding.

fn char_event(ch: &str, code: KeyCode) -> KeyInput {
    key_input(Key::Character(ch.into()), PhysicalKey::Code(code))
}

#[test]
fn test_shift_digit_mode2_sends_shifted_char() {
    // Shift+1 → '!' (not CSI 27;2;49~)
    // iTerm2 testShiftNumber: Shift+1 → "!"
    let mut handler = InputHandler::new();
    handler.update_modifiers(Modifiers::from(ModifiersState::SHIFT));

    let event = char_event("!", KeyCode::Digit1);
    let result = handler.handle_key_input_with_mode(&event, 2, false);
    assert_eq!(
        result,
        Some(b"!".to_vec()),
        "Shift+1 in mode 2 must pass through the OS-resolved shifted char, not CSI 27;2;49~"
    );
}

#[test]
fn test_shift_symbol_mode2_sends_shifted_char() {
    // Shift+[ → '{'
    // iTerm2 testShiftSymbol: Shift+[ → "{"
    let mut handler = InputHandler::new();
    handler.update_modifiers(Modifiers::from(ModifiersState::SHIFT));

    let event = char_event("{", KeyCode::BracketLeft);
    let result = handler.handle_key_input_with_mode(&event, 2, false);
    assert_eq!(
        result,
        Some(b"{".to_vec()),
        "Shift+[ in mode 2 must pass through '{{', not CSI 27;2;91~"
    );
}

#[test]
fn test_shift_letter_mode2_sends_uppercase() {
    // Shift+a → 'A' — already worked before the broader fix, guard against regression.
    // iTerm2 testShiftLetter: Shift+A → "A"
    let mut handler = InputHandler::new();
    handler.update_modifiers(Modifiers::from(ModifiersState::SHIFT));

    let event = char_event("A", KeyCode::KeyA);
    let result = handler.handle_key_input_with_mode(&event, 2, false);
    assert_eq!(
        result,
        Some(b"A".to_vec()),
        "Shift+a in mode 2 must send 'A'"
    );
}

#[test]
fn test_shift_digit_mode1_sends_shifted_char() {
    // Same rule applies in mode 1.
    let mut handler = InputHandler::new();
    handler.update_modifiers(Modifiers::from(ModifiersState::SHIFT));

    let event = char_event("!", KeyCode::Digit1);
    let result = handler.handle_key_input_with_mode(&event, 1, false);
    assert_eq!(result, Some(b"!".to_vec()));
}

#[test]
fn test_shift_digit_mode0_sends_shifted_char() {
    // And when modifyOtherKeys is disabled entirely.
    let mut handler = InputHandler::new();
    handler.update_modifiers(Modifiers::from(ModifiersState::SHIFT));

    let event = char_event("!", KeyCode::Digit1);
    let result = handler.handle_key_input_with_mode(&event, 0, false);
    assert_eq!(result, Some(b"!".to_vec()));
}

#[test]
fn test_ctrl_digit_mode2_still_encodes() {
    // Ctrl alone is still encoded via modifyOtherKeys in mode 2 — the
    // exemption is specifically for Shift-without-Ctrl.
    let mut handler = InputHandler::new();
    handler.update_modifiers(Modifiers::from(ModifiersState::CONTROL));

    // winit gives the base char when only Ctrl is held.
    let event = char_event("1", KeyCode::Digit1);
    let result = handler.handle_key_input_with_mode(&event, 2, false);
    assert_eq!(
        result,
        Some(b"\x1b[27;5;49~".to_vec()),
        "Ctrl+1 in mode 2 must still emit CSI 27;5;49~"
    );
}

#[test]
fn test_ctrl_shift_digit_mode2_still_encodes() {
    // Ctrl+Shift carries information the character alone cannot express,
    // so we still emit the modifyOtherKeys sequence here.
    let mut handler = InputHandler::new();
    handler.update_modifiers(Modifiers::from(
        ModifiersState::CONTROL | ModifiersState::SHIFT,
    ));

    // With Ctrl+Shift held, the logical key is still '!' from the OS.
    let event = char_event("!", KeyCode::Digit1);
    let result = handler.handle_key_input_with_mode(&event, 2, false);
    assert_eq!(
        result,
        Some(b"\x1b[27;6;49~".to_vec()),
        "Ctrl+Shift+1 in mode 2 must still emit a modifyOtherKeys sequence"
    );
}

#[test]
fn test_ctrl_alt_letter_mode0_sends_esc_prefixed_control_byte() {
    // When modifyOtherKeys is unavailable, Ctrl+Alt+letter must remain distinct
    // from plain Ctrl+letter so inner TUIs can parse legacy Meta+Ctrl chords.
    let mut handler = InputHandler::new();
    handler.update_modifiers(Modifiers::from(
        ModifiersState::CONTROL | ModifiersState::ALT,
    ));

    let event = char_event("p", KeyCode::KeyP);
    let result = handler.handle_key_input_with_mode(&event, 0, false);
    assert_eq!(
        result,
        Some(b"\x1b\x10".to_vec()),
        "Ctrl+Alt+P in mode 0 must send ESC-prefixed Ctrl+P, not collapse to plain Ctrl+P"
    );
}

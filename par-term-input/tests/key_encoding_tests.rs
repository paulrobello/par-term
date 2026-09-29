//! Byte-exact encoding matrix for `InputHandler`.
//!
//! `handle_key_input_with_mode` switches on three axes — key type, modifier set,
//! and terminal mode (modifyOtherKeys level, application cursor). These drive the
//! axes directly and assert the exact bytes written to the PTY.
//!
//! Since ENH-028 the bytes come from the shared encoder in
//! `par-term-emu-core-rust` (`keyboard::encode_key_with`); this crate only maps
//! winit events onto it. The rows below are the ones this crate pinned before the
//! cutover and they pass unchanged through the wired-up encoder, except one
//! deliberate change marked `ENH-028 CHANGE` (Home/End under DECCKM). A second
//! behaviour change, Alt+Space, was never pinned; its new row is marked the same
//! way. The rows are kept as the wrapper check that the winit mapping feeds the
//! core the right codepoint, modifier bits and Alt side.
//!
//! # Two constraints this file works under
//!
//! **Never fabricate a `winit::event::KeyEvent`.** It has a private
//! platform-specific field and no public constructor, so building one leaves
//! that field uninitialized — undefined behaviour that segfaulted the Linux lib
//! test binary before commit 53705aaf. [`KeyInput`] carries exactly the three
//! fields encoding reads and is safe to construct.
//!
//! **Alt side selection goes through `track_alt_physical_key`.** Unless a test
//! presses a side explicitly, no Alt key is tracked and the left mode applies;
//! most tests set both sides to the same mode via [`modes`] so the assertion
//! holds either way. The side-selection tests below press each side.
//!
//! # Non-ASCII coverage
//!
//! Character-key encoding is driven with accented Latin, CJK, emoji, ZWJ
//! sequences, combining marks and RTL text. A multi-byte character reaching the
//! PTY is precisely where a byte/char confusion lands, and it was untested. The
//! canonical corpus lives in `tests/common/unicode_corpus.rs` in the root crate;
//! `par-term-input` publishes to crates.io independently, so it carries its own
//! minimal copy rather than reaching outside its package directory.

use par_term_config::OptionKeyMode;
use par_term_input::{InputHandler, KeyInput, key_encode_options};
use winit::event::{ElementState, Modifiers};
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

fn pressed(logical_key: Key, physical_key: PhysicalKey) -> KeyInput {
    KeyInput {
        logical_key,
        physical_key,
        state: ElementState::Pressed,
    }
}

fn named(key: NamedKey, code: KeyCode) -> KeyInput {
    pressed(Key::Named(key), PhysicalKey::Code(code))
}

fn character(text: &str, code: KeyCode) -> KeyInput {
    pressed(Key::Character(text.into()), PhysicalKey::Code(code))
}

/// A handler with `mods` held and no Option-key tracking.
fn handler_with(mods: ModifiersState) -> InputHandler {
    let mut handler = InputHandler::new();
    handler.update_modifiers(Modifiers::from(mods));
    handler
}

const NONE: ModifiersState = ModifiersState::empty();
const SHIFT: ModifiersState = ModifiersState::SHIFT;
const CTRL: ModifiersState = ModifiersState::CONTROL;
const ALT: ModifiersState = ModifiersState::ALT;
const SUPER: ModifiersState = ModifiersState::SUPER;

/// Encode one key at modifyOtherKeys level 0 with application cursor off.
fn encode(mods: ModifiersState, input: &KeyInput) -> Option<Vec<u8>> {
    handler_with(mods).handle_key_input_with_mode(input, 0, false)
}

fn assert_bytes(mods: ModifiersState, input: &KeyInput, expected: &[u8], what: &str) {
    let actual = encode(mods, input);
    assert_eq!(
        actual.as_deref(),
        Some(expected),
        "{what}: expected {:?}, got {:?}",
        String::from_utf8_lossy(expected),
        actual.as_deref().map(String::from_utf8_lossy)
    );
}

// ---------------------------------------------------------------------------
// Press/release gating
// ---------------------------------------------------------------------------

#[test]
fn released_keys_encode_to_nothing() {
    let release = KeyInput {
        logical_key: Key::Named(NamedKey::ArrowUp),
        physical_key: PhysicalKey::Code(KeyCode::ArrowUp),
        state: ElementState::Released,
    };
    assert_eq!(encode(NONE, &release), None);

    // A release must stay silent in every mode, not just the default one.
    let mut handler = handler_with(CTRL);
    assert_eq!(handler.handle_key_input_with_mode(&release, 2, true), None);
}

// ---------------------------------------------------------------------------
// Named keys, no modifiers
// ---------------------------------------------------------------------------

#[test]
fn bare_named_keys_use_their_documented_sequences() {
    // (key, physical code, expected bytes) — "letter form" arrows/Home/End use
    // CSI <letter>, tilde-form keys use CSI <keycode> ~, F1-F4 use SS3.
    let cases: &[(NamedKey, KeyCode, &[u8])] = &[
        (NamedKey::ArrowUp, KeyCode::ArrowUp, b"\x1b[A"),
        (NamedKey::ArrowDown, KeyCode::ArrowDown, b"\x1b[B"),
        (NamedKey::ArrowRight, KeyCode::ArrowRight, b"\x1b[C"),
        (NamedKey::ArrowLeft, KeyCode::ArrowLeft, b"\x1b[D"),
        (NamedKey::Home, KeyCode::Home, b"\x1b[H"),
        (NamedKey::End, KeyCode::End, b"\x1b[F"),
        (NamedKey::Insert, KeyCode::Insert, b"\x1b[2~"),
        (NamedKey::Delete, KeyCode::Delete, b"\x1b[3~"),
        (NamedKey::PageUp, KeyCode::PageUp, b"\x1b[5~"),
        (NamedKey::PageDown, KeyCode::PageDown, b"\x1b[6~"),
        (NamedKey::F1, KeyCode::F1, b"\x1bOP"),
        (NamedKey::F2, KeyCode::F2, b"\x1bOQ"),
        (NamedKey::F3, KeyCode::F3, b"\x1bOR"),
        (NamedKey::F4, KeyCode::F4, b"\x1bOS"),
        (NamedKey::F5, KeyCode::F5, b"\x1b[15~"),
        (NamedKey::F6, KeyCode::F6, b"\x1b[17~"),
        (NamedKey::F7, KeyCode::F7, b"\x1b[18~"),
        (NamedKey::F8, KeyCode::F8, b"\x1b[19~"),
        (NamedKey::F9, KeyCode::F9, b"\x1b[20~"),
        (NamedKey::F10, KeyCode::F10, b"\x1b[21~"),
        (NamedKey::F11, KeyCode::F11, b"\x1b[23~"),
        (NamedKey::F12, KeyCode::F12, b"\x1b[24~"),
        (NamedKey::Enter, KeyCode::Enter, b"\r"),
        (NamedKey::Tab, KeyCode::Tab, b"\t"),
        (NamedKey::Space, KeyCode::Space, b" "),
        (NamedKey::Backspace, KeyCode::Backspace, b"\x7f"),
        (NamedKey::Escape, KeyCode::Escape, b"\x1b"),
    ];

    for (key, code, expected) in cases {
        assert_bytes(NONE, &named(*key, *code), expected, &format!("{key:?}"));
    }
}

#[test]
fn f5_through_f12_skip_the_keycodes_vt_reserves() {
    // The tilde-form keycodes deliberately skip 16 and 22 — xterm never assigned
    // them. A future edit that renumbers the table sequentially breaks every
    // function key above F5, and nothing else in the suite would notice.
    for (key, code) in [
        (NamedKey::F5, KeyCode::F5),
        (NamedKey::F6, KeyCode::F6),
        (NamedKey::F11, KeyCode::F11),
    ] {
        let bytes = encode(NONE, &named(key, code)).expect("function key encodes");
        let text = String::from_utf8(bytes).expect("ASCII sequence");
        assert_ne!(text, "\x1b[16~", "{key:?} must not use reserved keycode 16");
        assert_ne!(text, "\x1b[22~", "{key:?} must not use reserved keycode 22");
    }
}

// ---------------------------------------------------------------------------
// Named keys × modifier matrix
// ---------------------------------------------------------------------------

/// xterm modifier parameter: bit0 Shift, bit1 Alt, bit2 Ctrl, then +1.
#[test]
fn arrow_keys_encode_every_modifier_combination() {
    let cases: &[(ModifiersState, u8)] = &[
        (SHIFT, 2),
        (ALT, 3),
        (SHIFT | ALT, 4),
        (CTRL, 5),
        (SHIFT | CTRL, 6),
        (ALT | CTRL, 7),
        (SHIFT | ALT | CTRL, 8),
    ];

    for (mods, param) in cases {
        for (key, code, suffix) in [
            (NamedKey::ArrowUp, KeyCode::ArrowUp, 'A'),
            (NamedKey::ArrowDown, KeyCode::ArrowDown, 'B'),
            (NamedKey::ArrowRight, KeyCode::ArrowRight, 'C'),
            (NamedKey::ArrowLeft, KeyCode::ArrowLeft, 'D'),
            (NamedKey::Home, KeyCode::Home, 'H'),
            (NamedKey::End, KeyCode::End, 'F'),
        ] {
            let expected = format!("\x1b[1;{param}{suffix}");
            assert_bytes(
                *mods,
                &named(key, code),
                expected.as_bytes(),
                &format!("{key:?} with {mods:?}"),
            );
        }
    }
}

#[test]
fn super_alone_is_not_an_xterm_modifier() {
    // Super/Cmd carries no bit in the xterm parameter, so Cmd+Up must encode
    // exactly as a bare Up. (Cmd shortcuts are intercepted above this layer.)
    assert_bytes(
        SUPER,
        &named(NamedKey::ArrowUp, KeyCode::ArrowUp),
        b"\x1b[A",
        "Super+Up",
    );
}

#[test]
fn tilde_form_and_f1_to_f4_take_the_same_modifier_parameter() {
    assert_bytes(
        SHIFT,
        &named(NamedKey::Delete, KeyCode::Delete),
        b"\x1b[3;2~",
        "Shift+Delete",
    );
    assert_bytes(
        CTRL,
        &named(NamedKey::PageUp, KeyCode::PageUp),
        b"\x1b[5;5~",
        "Ctrl+PageUp",
    );
    assert_bytes(
        SHIFT | CTRL,
        &named(NamedKey::F12, KeyCode::F12),
        b"\x1b[24;6~",
        "Ctrl+Shift+F12",
    );
    // F1-F4 switch from SS3 to CSI when a modifier is present.
    assert_bytes(
        SHIFT,
        &named(NamedKey::F1, KeyCode::F1),
        b"\x1b[1;2P",
        "Shift+F1",
    );
    assert_bytes(
        ALT | CTRL,
        &named(NamedKey::F4, KeyCode::F4),
        b"\x1b[1;7S",
        "Ctrl+Alt+F4",
    );
}

// ---------------------------------------------------------------------------
// Application cursor mode (DECCKM)
// ---------------------------------------------------------------------------

#[test]
fn application_cursor_switches_bare_arrows_to_ss3() {
    for (key, code, suffix) in [
        (NamedKey::ArrowUp, KeyCode::ArrowUp, 'A'),
        (NamedKey::ArrowDown, KeyCode::ArrowDown, 'B'),
        (NamedKey::ArrowRight, KeyCode::ArrowRight, 'C'),
        (NamedKey::ArrowLeft, KeyCode::ArrowLeft, 'D'),
    ] {
        let input = named(key, code);

        let normal = handler_with(NONE).handle_key_input_with_mode(&input, 0, false);
        assert_eq!(normal.as_deref(), Some(format!("\x1b[{suffix}").as_bytes()));

        let application = handler_with(NONE).handle_key_input_with_mode(&input, 0, true);
        assert_eq!(
            application.as_deref(),
            Some(format!("\x1bO{suffix}").as_bytes()),
            "{key:?} in application cursor mode must use SS3"
        );
    }
}

#[test]
fn application_cursor_switches_bare_home_end_to_ss3_but_not_modified_arrows() {
    // ENH-028 CHANGE: Home/End under DECCKM now send SS3 H/F (formerly CSI).
    // That is the `khome`/`kend` pair the xterm-256color terminfo par-term
    // advertises lists alongside `smkx` (= DECCKM on), and what xterm and
    // iTerm2 (VT100Output.m specialKey → CURSOR_SET_HOME) send.
    for (key, code, expected) in [
        (NamedKey::Home, KeyCode::Home, b"\x1bOH"),
        (NamedKey::End, KeyCode::End, b"\x1bOF"),
    ] {
        let bytes = handler_with(NONE).handle_key_input_with_mode(&named(key, code), 0, true);
        assert_eq!(
            bytes.as_deref(),
            Some(&expected[..]),
            "{key:?} under DECCKM"
        );
    }

    // With any modifier present the sequence switches to CSI form even under
    // DECCKM — SS3 has no modifier encoding.
    let up = named(NamedKey::ArrowUp, KeyCode::ArrowUp);
    let bytes = handler_with(CTRL).handle_key_input_with_mode(&up, 0, true);
    assert_eq!(bytes.as_deref(), Some(&b"\x1b[1;5A"[..]));
}

// ---------------------------------------------------------------------------
// Ctrl + character → control codes
// ---------------------------------------------------------------------------

#[test]
fn ctrl_letter_maps_to_control_codes() {
    let letters = [
        ("a", KeyCode::KeyA, 0x01),
        ("b", KeyCode::KeyB, 0x02),
        ("c", KeyCode::KeyC, 0x03),
        ("i", KeyCode::KeyI, 0x09),
        ("m", KeyCode::KeyM, 0x0d),
        ("z", KeyCode::KeyZ, 0x1a),
    ];
    for (text, code, expected) in letters {
        assert_bytes(
            CTRL,
            &character(text, code),
            &[expected],
            &format!("Ctrl+{text}"),
        );
        // Case of the logical key must not matter — a Shift+Ctrl+letter arrives
        // as an uppercase character.
        assert_bytes(
            CTRL | SHIFT,
            &character(&text.to_uppercase(), code),
            &[expected],
            &format!("Ctrl+Shift+{text}"),
        );
    }
}

#[test]
fn ctrl_punctuation_in_the_0x40_to_0x5f_range_maps_to_control_codes() {
    let cases = [
        ("@", KeyCode::Digit2, 0x00),
        ("[", KeyCode::BracketLeft, 0x1b),
        ("\\", KeyCode::Backslash, 0x1c),
        ("]", KeyCode::BracketRight, 0x1d),
        ("^", KeyCode::Digit6, 0x1e),
        ("_", KeyCode::Minus, 0x1f),
    ];
    for (text, code, expected) in cases {
        assert_bytes(
            CTRL,
            &character(text, code),
            &[expected],
            &format!("Ctrl+{text}"),
        );
    }
}

#[test]
fn ctrl_space_sends_nul() {
    assert_bytes(
        CTRL,
        &named(NamedKey::Space, KeyCode::Space),
        &[0x00],
        "Ctrl+Space",
    );
}

#[test]
fn ctrl_question_mark_does_not_send_del() {
    // DIVERGENCE (pinned, not endorsed): xterm sends DEL (0x7f) for Ctrl+?.
    // `?` is 0x3F, one below the 0x40..=0x5F window in key_encoding.rs:147, and
    // it is not alphabetic, so it falls through to the plain-character path and
    // the literal `?` byte reaches the PTY. Recorded as current behaviour so a
    // future xterm-compatibility fix shows up here as a deliberate change.
    assert_bytes(CTRL, &character("?", KeyCode::Slash), b"?", "Ctrl+?");
}

// ---------------------------------------------------------------------------
// Ctrl + non-ASCII
// ---------------------------------------------------------------------------

#[test]
fn ctrl_non_ascii_char_is_sent_as_utf8_not_truncated() {
    // Regression test. `ch as u8` truncates a non-ASCII scalar to its low byte,
    // so before the `is_ascii()` guard any codepoint whose low byte landed in
    // 0x40..=0x5F was masked with 0x1F and emitted as a control code. Reachable
    // on real layouts at the default modifyOtherKeys level 0, and the damage is
    // silent — a wrong byte reaching the shell rather than a panic:
    //
    //   Polish  Ctrl+ł  U+0142 -> low byte 0x42 -> 0x02  (looked like Ctrl+B)
    //   Latin   Ctrl+ŀ  U+0140 -> low byte 0x40 -> 0x00  (NUL)
    //   Latin   Ctrl+ŕ  U+0155 -> low byte 0x55 -> 0x15  (Ctrl+U, killed the line)
    //   Emoji   Ctrl+🍀 U+1F340 -> low byte 0x40 -> 0x00
    let formerly_truncated = [
        ("ł", KeyCode::KeyL),
        ("ŀ", KeyCode::KeyL),
        ("ŕ", KeyCode::KeyR),
        ("🍀", KeyCode::KeyK),
    ];
    for (text, code) in formerly_truncated {
        assert_bytes(
            CTRL,
            &character(text, code),
            text.as_bytes(),
            &format!("Ctrl+{text}"),
        );
    }

    // Codepoints whose low byte always fell outside the window, unchanged.
    assert_bytes(
        CTRL,
        &character("é", KeyCode::KeyE),
        "é".as_bytes(),
        "Ctrl+é",
    );
    assert_bytes(
        CTRL,
        &character("日", KeyCode::KeyR),
        "日".as_bytes(),
        "Ctrl+日",
    );
}

// ---------------------------------------------------------------------------
// Non-ASCII character keys (ENH-002 corpus applied to the PTY write path)
// ---------------------------------------------------------------------------

/// The corpus, with the byte length each entry must produce. Mirrors
/// `tests/common/unicode_corpus.rs` in the root crate; kept local because
/// `par-term-input` is published as a standalone crate.
const CORPUS: &[(&str, &str, usize)] = &[
    ("accented latin", "é", 2),
    ("accented latin word", "café", 5),
    ("cyrillic", "Привет", 12),
    ("greek", "ΟΔΟΣ", 8),
    ("cjk", "日本語", 9),
    ("hangul", "한글", 6),
    ("emoji", "😀", 4),
    (
        "emoji zwj family",
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
        18,
    ),
    ("flag emoji", "\u{1F1EF}\u{1F1F5}", 8),
    ("skin tone emoji", "\u{1F44D}\u{1F3FD}", 8),
    ("combining mark", "e\u{0301}", 3),
    ("rtl arabic", "مرحبا", 10),
    ("rtl hebrew", "שלום", 8),
    ("zero width joiner", "\u{200D}", 3),
    ("zero width space", "\u{200B}", 3),
    ("curly quotes", "\u{201C}x\u{201D}", 7),
    ("mixed", "a日b😀c", 10),
];

#[test]
fn unmodified_character_keys_write_their_utf8_bytes_verbatim() {
    for (label, text, byte_len) in CORPUS {
        assert_eq!(text.len(), *byte_len, "{label}: corpus byte length drifted");

        let input = character(text, KeyCode::KeyA);
        let bytes = encode(NONE, &input).unwrap_or_else(|| panic!("{label} produced no bytes"));
        assert_eq!(
            bytes,
            text.as_bytes(),
            "{label}: character keys must write UTF-8 verbatim"
        );
        assert_eq!(bytes.len(), *byte_len, "{label}: wrong byte count");
    }
}

#[test]
fn shift_and_super_do_not_disturb_non_ascii_character_bytes() {
    // Neither modifier reaches the character branch, so the OS-resolved
    // character must pass through untouched at every modifyOtherKeys level.
    for (label, text, _) in CORPUS {
        for mods in [SHIFT, SUPER, SHIFT | SUPER] {
            for mode in [0u8, 1, 2] {
                let input = character(text, KeyCode::KeyA);
                let bytes = handler_with(mods).handle_key_input_with_mode(&input, mode, false);
                assert_eq!(
                    bytes.as_deref(),
                    Some(text.as_bytes()),
                    "{label} with {mods:?} at modifyOtherKeys {mode}"
                );
            }
        }
    }
}

#[test]
fn multi_scalar_graphemes_are_written_as_one_unit() {
    // An IME or a ZWJ sequence arrives as a single `Key::Character` holding
    // several scalars. The encoder must not split it: a partial write would put
    // an incomplete grapheme on the wire.
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    assert_eq!(family.chars().count(), 5);

    let bytes = encode(NONE, &character(family, KeyCode::KeyA)).expect("encodes");
    assert_eq!(bytes.len(), 18);
    assert_eq!(String::from_utf8(bytes).expect("valid UTF-8"), family);
}

// ---------------------------------------------------------------------------
// Option/Alt key modes
// ---------------------------------------------------------------------------

/// Set both Option sides to `mode`.
fn modes(handler: &mut InputHandler, mode: OptionKeyMode) {
    handler.key_options = key_encode_options(mode, mode);
}

fn alt_handler(mode: OptionKeyMode) -> InputHandler {
    let mut handler = handler_with(ALT);
    // Both sides set identically — see the module note on Alt side selection.
    modes(&mut handler, mode);
    handler
}

#[test]
fn option_key_modes_transform_an_ascii_base_character() {
    // macOS with Normal mode delivers Option+f as 'ƒ'; the base character comes
    // from the physical key, not the logical one.
    let input = character("ƒ", KeyCode::KeyF);

    let normal = alt_handler(OptionKeyMode::Normal).handle_key_input_with_mode(&input, 0, false);
    assert_eq!(
        normal.as_deref(),
        Some("ƒ".as_bytes()),
        "Normal mode passes the OS-composed character through"
    );

    let meta = alt_handler(OptionKeyMode::Meta).handle_key_input_with_mode(&input, 0, false);
    assert_eq!(
        meta.as_deref(),
        Some(&[0xE6u8][..]),
        "Meta mode sets the high bit on the base character ('f' | 0x80)"
    );

    let esc = alt_handler(OptionKeyMode::Esc).handle_key_input_with_mode(&input, 0, false);
    assert_eq!(
        esc.as_deref(),
        Some(&[0x1bu8, b'f'][..]),
        "Esc mode sends ESC then the base character"
    );
}

#[test]
fn option_key_modes_fall_back_to_esc_prefixing_for_non_ascii() {
    // `KeyCode::IntlBackslash` has no entry in the physical-key table, so the
    // encoder falls back to the first logical character — which is non-ASCII
    // here, taking the branch where Meta cannot set a high bit.
    let input = character("é", KeyCode::IntlBackslash);
    let expected = [&[0x1bu8][..], "é".as_bytes()].concat();

    for mode in [OptionKeyMode::Meta, OptionKeyMode::Esc] {
        let bytes = alt_handler(mode).handle_key_input_with_mode(&input, 0, false);
        assert_eq!(
            bytes.as_deref(),
            Some(&expected[..]),
            "{mode:?} must prepend ESC rather than corrupt a multi-byte character"
        );
        // The character's own bytes must survive intact after the ESC.
        assert_eq!(&bytes.expect("encodes")[1..], "é".as_bytes());
    }

    // Normal mode leaves the multi-byte character completely alone.
    let normal = alt_handler(OptionKeyMode::Normal).handle_key_input_with_mode(&input, 0, false);
    assert_eq!(normal.as_deref(), Some("é".as_bytes()));
}

#[test]
fn ctrl_alt_letter_preserves_the_alt_modifier() {
    let input = character("a", KeyCode::KeyA);
    let mut handler = handler_with(CTRL | ALT);

    modes(&mut handler, OptionKeyMode::Meta);
    assert_eq!(
        handler
            .handle_key_input_with_mode(&input, 0, false)
            .as_deref(),
        Some(&[0x81u8][..]),
        "Meta mode ORs the high bit onto the control byte"
    );

    for mode in [OptionKeyMode::Esc, OptionKeyMode::Normal] {
        modes(&mut handler, mode);
        assert_eq!(
            handler
                .handle_key_input_with_mode(&input, 0, false)
                .as_deref(),
            Some(&[0x1bu8, 0x01][..]),
            "{mode:?} prefixes the control byte with ESC"
        );
    }
}

// ---------------------------------------------------------------------------
// modifyOtherKeys routing
// ---------------------------------------------------------------------------

#[test]
fn modify_other_keys_reports_the_base_codepoint_not_the_shifted_one() {
    // Ctrl+Shift+1: the reported keycode is the *base* character '1' (49), which
    // is what the physical-key table supplies.
    let input = character("!", KeyCode::Digit1);
    let bytes = handler_with(CTRL | SHIFT).handle_key_input_with_mode(&input, 2, false);
    assert_eq!(bytes.as_deref(), Some(&b"\x1b[27;6;49~"[..]));
}

#[test]
fn modify_other_keys_is_skipped_when_the_physical_key_has_no_base_character() {
    // Without a base character the encoder must fall back to normal handling
    // rather than emit a sequence with a wrong or missing keycode.
    let input = character("é", KeyCode::IntlBackslash);
    let bytes = handler_with(CTRL).handle_key_input_with_mode(&input, 2, false);
    assert_eq!(
        bytes.as_deref(),
        Some("é".as_bytes()),
        "no base character means no modifyOtherKeys encoding"
    );
}

#[test]
fn modify_other_keys_level_zero_never_emits_csi_27() {
    for (_, text, _) in CORPUS {
        for mods in [CTRL, ALT, CTRL | ALT, SHIFT | CTRL] {
            let input = character(text, KeyCode::KeyA);
            if let Some(bytes) = handler_with(mods).handle_key_input_with_mode(&input, 0, false) {
                assert!(
                    !bytes.starts_with(b"\x1b[27;"),
                    "level 0 must not use modifyOtherKeys for {text:?} with {mods:?}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Fuzz-style invariants over the corpus (deterministic, no proptest dependency)
// ---------------------------------------------------------------------------

/// Deterministic 64-bit LCG. A seeded generator keeps failures reproducible and
/// avoids adding `proptest` to a published crate's dependency graph.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next() >> 33) as usize % items.len()]
    }
}

#[test]
fn encoding_never_panics_and_always_yields_valid_utf8_or_control_bytes() {
    let codes = [
        KeyCode::KeyA,
        KeyCode::KeyL,
        KeyCode::Digit1,
        KeyCode::BracketLeft,
        KeyCode::IntlBackslash,
        KeyCode::Slash,
    ];
    let modifier_sets = [
        NONE,
        SHIFT,
        CTRL,
        ALT,
        SUPER,
        SHIFT | CTRL,
        CTRL | ALT,
        SHIFT | ALT | CTRL,
    ];
    let option_modes = [
        OptionKeyMode::Normal,
        OptionKeyMode::Meta,
        OptionKeyMode::Esc,
    ];

    let mut rng = Lcg(0x5EED_1234_ABCD_0001);
    for _ in 0..4_000 {
        // Build a string by concatenating one to three corpus atoms.
        let atom_count = 1 + (rng.next() >> 33) as usize % 3;
        let mut text = String::new();
        for _ in 0..atom_count {
            text.push_str(rng.pick(CORPUS).1);
        }

        let mods = *rng.pick(&modifier_sets);
        let code = *rng.pick(&codes);
        let mode = (rng.next() >> 33) as u8 % 3;
        let application_cursor = rng.next() & 1 == 0;
        let option_mode = *rng.pick(&option_modes);

        let mut handler = handler_with(mods);
        modes(&mut handler, option_mode);
        let input = character(&text, code);

        // The property: encoding must return, never panic, and never produce a
        // truncated multi-byte character. Bytes are either a pure-ASCII control
        // sequence or valid UTF-8 (optionally after an ESC or high-bit prefix).
        if let Some(bytes) = handler.handle_key_input_with_mode(&input, mode, application_cursor) {
            assert!(
                !bytes.is_empty(),
                "empty encoding for {text:?} / {mods:?} / mode {mode}"
            );
            let payload = bytes.strip_prefix(&[0x1b]).unwrap_or(&bytes);
            if payload.iter().any(|b| *b >= 0x80) {
                assert!(
                    std::str::from_utf8(payload).is_ok() || payload.len() == 1,
                    "non-ASCII payload must stay valid UTF-8 (or be a single \
                     high-bit Meta byte): {payload:?} for {text:?} / {mods:?}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ENH-028: rows from the core conformance suite that this crate did not pin,
// driven through the winit mapping (par-term-emu-core-rust src/keyboard.rs).
// ---------------------------------------------------------------------------

fn press_alt(handler: &mut InputHandler, code: KeyCode) {
    handler.track_alt_physical_key(PhysicalKey::Code(code), ElementState::Pressed);
}

#[test]
fn right_alt_selects_the_right_option_mode_and_both_held_selects_left() {
    let input = character("å", KeyCode::KeyA);
    let handler = || {
        let mut h = handler_with(ALT);
        h.key_options = key_encode_options(OptionKeyMode::Normal, OptionKeyMode::Meta);
        h
    };

    let mut left = handler();
    press_alt(&mut left, KeyCode::AltLeft);
    assert_eq!(
        left.handle_key_input_with_mode(&input, 0, false).as_deref(),
        Some("å".as_bytes()),
        "left Alt → left (Normal) mode passes the composed glyph through"
    );

    let mut right = handler();
    press_alt(&mut right, KeyCode::AltRight);
    assert_eq!(
        right
            .handle_key_input_with_mode(&input, 0, false)
            .as_deref(),
        Some(&[0xE1u8][..]),
        "right Alt → right (Meta) mode sets the high bit on the base 'a'"
    );

    let mut both = handler();
    press_alt(&mut both, KeyCode::AltLeft);
    press_alt(&mut both, KeyCode::AltRight);
    assert_eq!(
        both.handle_key_input_with_mode(&input, 0, false).as_deref(),
        Some("å".as_bytes()),
        "both Alts held → left wins"
    );

    // Releasing the left key leaves the right one in charge.
    both.track_alt_physical_key(PhysicalKey::Code(KeyCode::AltLeft), ElementState::Released);
    assert_eq!(
        both.handle_key_input_with_mode(&input, 0, false).as_deref(),
        Some(&[0xE1u8][..])
    );
}

#[test]
fn shift_enter_sends_lf() {
    assert_bytes(
        SHIFT,
        &named(NamedKey::Enter, KeyCode::Enter),
        b"\n",
        "Shift+Enter",
    );
}

#[test]
fn ctrl_super_up_counts_ctrl_only() {
    assert_bytes(
        CTRL | SUPER,
        &named(NamedKey::ArrowUp, KeyCode::ArrowUp),
        b"\x1b[1;5A",
        "Ctrl+Super+Up",
    );
}

#[test]
fn modify_other_keys_mode_one_and_two_share_the_rule_set() {
    let cases: &[(ModifiersState, KeyInput, u8, &[u8], &str)] = &[
        (
            CTRL,
            character("c", KeyCode::KeyC),
            1,
            b"\x1b[27;5;99~",
            "mode 1 Ctrl+c",
        ),
        (
            CTRL,
            character("a", KeyCode::KeyA),
            2,
            b"\x1b[27;5;97~",
            "mode 2 Ctrl+a",
        ),
        (
            CTRL,
            named(NamedKey::Space, KeyCode::Space),
            1,
            b"\x1b[27;5;32~",
            "mode 1 Ctrl+Space",
        ),
        // Alt routes to the 27-form too, reporting the base 'f', not 'ƒ'; the
        // Option mode does not apply under modifyOtherKeys.
        (
            ALT,
            character("ƒ", KeyCode::KeyF),
            2,
            b"\x1b[27;3;102~",
            "mode 2 Alt+f",
        ),
        // Functional keys never take the 27-form.
        (
            CTRL,
            named(NamedKey::Enter, KeyCode::Enter),
            2,
            b"\r",
            "mode 2 Ctrl+Enter",
        ),
    ];
    for (mods, input, mode, expected, what) in cases {
        let bytes = handler_with(*mods).handle_key_input_with_mode(input, *mode, false);
        assert_eq!(bytes.as_deref(), Some(*expected), "{what}");
    }
}

#[test]
fn alt_space_applies_the_option_mode() {
    // ENH-028 CHANGE: Alt+Space formerly sent a plain space because Space fell
    // into the encoder's catch-all "no modifier encoding" arm. It now runs the
    // Option-key transform like every other Alt+key, as iTerm2 does
    // (dataForOptionModifiedKeypress has no Space exception). Users who type
    // with Option-composed characters already run Normal mode on that side,
    // and Normal still sends a plain space.
    let space = named(NamedKey::Space, KeyCode::Space);
    let cases: &[(OptionKeyMode, &[u8])] = &[
        (OptionKeyMode::Normal, b" "),
        (OptionKeyMode::Meta, &[0xA0]),
        (OptionKeyMode::Esc, b"\x1b "),
    ];
    for (mode, expected) in cases {
        let bytes = alt_handler(*mode).handle_key_input_with_mode(&space, 0, false);
        assert_eq!(bytes.as_deref(), Some(*expected), "Alt+Space in {mode:?}");
    }

    // Ctrl+Alt+Space keeps Alt the same way Ctrl+Alt+letter does.
    let mut handler = handler_with(CTRL | ALT);
    modes(&mut handler, OptionKeyMode::Esc);
    assert_eq!(
        handler
            .handle_key_input_with_mode(&space, 0, false)
            .as_deref(),
        Some(&[0x1bu8, 0x00][..])
    );
    modes(&mut handler, OptionKeyMode::Meta);
    assert_eq!(
        handler
            .handle_key_input_with_mode(&space, 0, false)
            .as_deref(),
        Some(&[0x80u8][..])
    );
}

#[test]
fn keys_without_a_terminal_encoding_produce_nothing() {
    assert_eq!(
        encode(NONE, &named(NamedKey::CapsLock, KeyCode::CapsLock)),
        None
    );
    assert_eq!(encode(NONE, &named(NamedKey::F13, KeyCode::F13)), None);
    let dead = pressed(Key::Dead(Some('`')), PhysicalKey::Code(KeyCode::Backquote));
    assert_eq!(encode(NONE, &dead), None);
}

#[test]
fn alt_esc_prefix_keeps_a_multi_scalar_string_whole_when_there_is_no_base_key() {
    // No physical base character and a non-ASCII first scalar: Meta and Esc both
    // fall back to an ESC prefix, and the rest of the string must follow it.
    let input = character("é\u{301}", KeyCode::IntlBackslash);
    let expected = [&[0x1bu8][..], "é\u{301}".as_bytes()].concat();
    for mode in [OptionKeyMode::Meta, OptionKeyMode::Esc] {
        let bytes = alt_handler(mode).handle_key_input_with_mode(&input, 0, false);
        assert_eq!(bytes.as_deref(), Some(&expected[..]), "{mode:?}");
    }
}

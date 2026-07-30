//! Byte-exact encoding matrix for `InputHandler`.
//!
//! `handle_key_input_with_mode` switches on three axes — key type, modifier set,
//! and terminal mode (modifyOtherKeys level, application cursor) — and until now
//! the crate had no tests of its own. These drive the axes directly and assert
//! the exact bytes written to the PTY.
//!
//! # Two constraints this file works under
//!
//! **Never fabricate a `winit::event::KeyEvent`.** It has a private
//! platform-specific field and no public constructor, so building one leaves
//! that field uninitialized — undefined behaviour that segfaulted the Linux lib
//! test binary before commit 53705aaf. [`KeyInput`] carries exactly the three
//! fields encoding reads and is safe to construct.
//!
//! **Left/right Alt selection is unreachable.** `InputHandler::track_alt_key`
//! takes a `&KeyEvent`, so no test can set `left_alt_pressed` /
//! `right_alt_pressed`. Every Alt case below therefore exercises the
//! "neither tracked" fallback, which resolves to the *left* mode. The tests set
//! both modes to the same value via `update_option_key_modes(m, m)` so the
//! assertion holds either way.
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
use par_term_input::{InputHandler, KeyInput};
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
fn application_cursor_does_not_apply_to_home_end_or_modified_arrows() {
    // Home/End are in the same "letter form" table but are not cursor keys, so
    // DECCKM must leave them on CSI.
    let home = named(NamedKey::Home, KeyCode::Home);
    let bytes = handler_with(NONE).handle_key_input_with_mode(&home, 0, true);
    assert_eq!(bytes.as_deref(), Some(&b"\x1b[H"[..]));

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
// Ctrl + non-ASCII — PRODUCTION BUG, pinned
// ---------------------------------------------------------------------------

#[test]
fn ctrl_non_ascii_char_truncates_to_a_control_byte() {
    // BUG (report-only, do not treat these assertions as desired behaviour).
    //
    // `par-term-input/src/key_encoding.rs:146` does `let byte = ch as u8;`,
    // which *truncates* a non-ASCII scalar to its low byte instead of rejecting
    // it. Any codepoint whose low byte lands in 0x40..=0x5F is then masked with
    // 0x1F and emitted as a control code. This is the same column/byte/char
    // conflation class as the six defects fixed today, on a path where the
    // damage is a wrong byte sent to the shell rather than a panic.
    //
    // Reachable on real layouts at the default modifyOtherKeys level 0:
    //   Polish  Ctrl+ł  U+0142 -> low byte 0x42 -> 0x02  (indistinguishable from Ctrl+B)
    //   Latin   Ctrl+ŀ  U+0140 -> low byte 0x40 -> 0x00  (NUL)
    //   Latin   Ctrl+ŕ  U+0155 -> low byte 0x55 -> 0x15  (Ctrl+U, kills the line in readline)
    //   Emoji   Ctrl+🍀 U+1F340 -> low byte 0x40 -> 0x00
    //
    // Correct behaviour would be to require `ch.is_ascii()` before the range
    // test, letting non-ASCII fall through to the plain-character path.
    let truncating = [
        ("ł", KeyCode::KeyL, 0x02u8),
        ("ŀ", KeyCode::KeyL, 0x00),
        ("ŕ", KeyCode::KeyR, 0x15),
        ("🍀", KeyCode::KeyK, 0x00),
    ];
    for (text, code, wrong_byte) in truncating {
        assert_bytes(
            CTRL,
            &character(text, code),
            &[wrong_byte],
            &format!("Ctrl+{text} (pinned bug)"),
        );
    }

    // Codepoints whose low byte falls outside the window are unaffected and
    // reach the PTY as UTF-8, which is what all of them should do.
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

fn alt_handler(mode: OptionKeyMode) -> InputHandler {
    let mut handler = handler_with(ALT);
    // Both sides set identically — see the module note on `track_alt_key`.
    handler.update_option_key_modes(mode, mode);
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

    handler.update_option_key_modes(OptionKeyMode::Meta, OptionKeyMode::Meta);
    assert_eq!(
        handler
            .handle_key_input_with_mode(&input, 0, false)
            .as_deref(),
        Some(&[0x81u8][..]),
        "Meta mode ORs the high bit onto the control byte"
    );

    for mode in [OptionKeyMode::Esc, OptionKeyMode::Normal] {
        handler.update_option_key_modes(mode, mode);
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
        handler.update_option_key_modes(option_mode, option_mode);
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

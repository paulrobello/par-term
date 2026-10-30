//! Platform-specific keybinding resolution.
//!
//! Contains:
//! - `cmd_or_ctrl` modifier expansion (Cmd on macOS, Ctrl elsewhere)
//! - Physical key → QWERTY character mapping for language-agnostic bindings
//! - Named key alias table (string → `NamedKey`)
//! - Physical key code alias table (string → `KeyCode`)

use winit::keyboard::{KeyCode, NamedKey};

/// Resolve the `CmdOrCtrl` modifier for the current platform.
///
/// Returns `(expected_ctrl, expected_super)` given a `cmd_or_ctrl` flag and the
/// raw `ctrl`/`super_key` values from the parsed combo.
///
/// - macOS: `CmdOrCtrl` maps to Super (Cmd key).
/// - All other platforms: `CmdOrCtrl` maps to Ctrl.
#[inline]
pub fn resolve_cmd_or_ctrl(cmd_or_ctrl: bool, ctrl: bool, super_key: bool) -> (bool, bool) {
    if cmd_or_ctrl {
        #[cfg(target_os = "macos")]
        {
            (ctrl, true) // CmdOrCtrl -> Super on macOS
        }
        #[cfg(not(target_os = "macos"))]
        {
            (true, super_key) // CmdOrCtrl -> Ctrl on other platforms
        }
    } else {
        (ctrl, super_key)
    }
}

/// Check if a physical key code corresponds to a character on a QWERTY layout.
///
/// This maps physical key positions (scan codes) to the characters they produce
/// on a US QWERTY keyboard, enabling language-agnostic keybindings.
pub fn physical_key_matches_char(code: KeyCode, ch: char) -> bool {
    let expected_char = match code {
        KeyCode::KeyA => 'A',
        KeyCode::KeyB => 'B',
        KeyCode::KeyC => 'C',
        KeyCode::KeyD => 'D',
        KeyCode::KeyE => 'E',
        KeyCode::KeyF => 'F',
        KeyCode::KeyG => 'G',
        KeyCode::KeyH => 'H',
        KeyCode::KeyI => 'I',
        KeyCode::KeyJ => 'J',
        KeyCode::KeyK => 'K',
        KeyCode::KeyL => 'L',
        KeyCode::KeyM => 'M',
        KeyCode::KeyN => 'N',
        KeyCode::KeyO => 'O',
        KeyCode::KeyP => 'P',
        KeyCode::KeyQ => 'Q',
        KeyCode::KeyR => 'R',
        KeyCode::KeyS => 'S',
        KeyCode::KeyT => 'T',
        KeyCode::KeyU => 'U',
        KeyCode::KeyV => 'V',
        KeyCode::KeyW => 'W',
        KeyCode::KeyX => 'X',
        KeyCode::KeyY => 'Y',
        KeyCode::KeyZ => 'Z',
        KeyCode::Digit0 => '0',
        KeyCode::Digit1 => '1',
        KeyCode::Digit2 => '2',
        KeyCode::Digit3 => '3',
        KeyCode::Digit4 => '4',
        KeyCode::Digit5 => '5',
        KeyCode::Digit6 => '6',
        KeyCode::Digit7 => '7',
        KeyCode::Digit8 => '8',
        KeyCode::Digit9 => '9',
        KeyCode::Minus => '-',
        KeyCode::Equal => '=',
        KeyCode::BracketLeft => '[',
        KeyCode::BracketRight => ']',
        KeyCode::Backslash => '\\',
        KeyCode::Semicolon => ';',
        KeyCode::Quote => '\'',
        KeyCode::Backquote => '`',
        KeyCode::Comma => ',',
        KeyCode::Period => '.',
        KeyCode::Slash => '/',
        _ => return false,
    };
    expected_char.eq_ignore_ascii_case(&ch)
}

/// The unshifted character on the same physical key as a US-layout shifted
/// symbol (`:` shares a key with `;`), or `None` when `ch` is not one.
pub fn us_unshifted_char(ch: char) -> Option<char> {
    Some(match ch {
        ':' => ';',
        '"' => '\'',
        '<' => ',',
        '>' => '.',
        '?' => '/',
        '{' => '[',
        '}' => ']',
        '|' => '\\',
        '_' => '-',
        '+' => '=',
        '~' => '`',
        '!' => '1',
        '@' => '2',
        '#' => '3',
        '$' => '4',
        '%' => '5',
        '^' => '6',
        '&' => '7',
        '*' => '8',
        '(' => '9',
        ')' => '0',
        _ => return None,
    })
}

/// Parse a named key string into a [`NamedKey`].
///
/// Accepts human-readable aliases such as `"Enter"`, `"Return"`, `"Esc"`,
/// `"PgUp"`, arrow keys, and function keys F1–F12.  Matching is
/// case-insensitive.  Returns `None` for unrecognised strings.
pub fn parse_named_key(s: &str) -> Option<NamedKey> {
    match s.to_lowercase().as_str() {
        // Function keys
        "f1" => Some(NamedKey::F1),
        "f2" => Some(NamedKey::F2),
        "f3" => Some(NamedKey::F3),
        "f4" => Some(NamedKey::F4),
        "f5" => Some(NamedKey::F5),
        "f6" => Some(NamedKey::F6),
        "f7" => Some(NamedKey::F7),
        "f8" => Some(NamedKey::F8),
        "f9" => Some(NamedKey::F9),
        "f10" => Some(NamedKey::F10),
        "f11" => Some(NamedKey::F11),
        "f12" => Some(NamedKey::F12),

        // Common named keys
        "enter" | "return" => Some(NamedKey::Enter),
        "escape" | "esc" => Some(NamedKey::Escape),
        "space" => Some(NamedKey::Space),
        "tab" => Some(NamedKey::Tab),
        "backspace" => Some(NamedKey::Backspace),
        "delete" | "del" => Some(NamedKey::Delete),
        "insert" | "ins" => Some(NamedKey::Insert),
        "home" => Some(NamedKey::Home),
        "end" => Some(NamedKey::End),
        "pageup" | "pgup" => Some(NamedKey::PageUp),
        "pagedown" | "pgdn" => Some(NamedKey::PageDown),

        // Arrow keys
        "up" | "arrowup" => Some(NamedKey::ArrowUp),
        "down" | "arrowdown" => Some(NamedKey::ArrowDown),
        "left" | "arrowleft" => Some(NamedKey::ArrowLeft),
        "right" | "arrowright" => Some(NamedKey::ArrowRight),

        _ => None,
    }
}

/// Physical key code spellings accepted by [`parse_physical_key_code`],
/// keyed by their lowercase form: letters, number row, punctuation by
/// position, F1–F12, navigation, and special keys.
const PHYSICAL_KEY_CODES: &[(&str, KeyCode)] = &[
    ("keya", KeyCode::KeyA),
    ("keyb", KeyCode::KeyB),
    ("keyc", KeyCode::KeyC),
    ("keyd", KeyCode::KeyD),
    ("keye", KeyCode::KeyE),
    ("keyf", KeyCode::KeyF),
    ("keyg", KeyCode::KeyG),
    ("keyh", KeyCode::KeyH),
    ("keyi", KeyCode::KeyI),
    ("keyj", KeyCode::KeyJ),
    ("keyk", KeyCode::KeyK),
    ("keyl", KeyCode::KeyL),
    ("keym", KeyCode::KeyM),
    ("keyn", KeyCode::KeyN),
    ("keyo", KeyCode::KeyO),
    ("keyp", KeyCode::KeyP),
    ("keyq", KeyCode::KeyQ),
    ("keyr", KeyCode::KeyR),
    ("keys", KeyCode::KeyS),
    ("keyt", KeyCode::KeyT),
    ("keyu", KeyCode::KeyU),
    ("keyv", KeyCode::KeyV),
    ("keyw", KeyCode::KeyW),
    ("keyx", KeyCode::KeyX),
    ("keyy", KeyCode::KeyY),
    ("keyz", KeyCode::KeyZ),
    ("digit0", KeyCode::Digit0),
    ("digit1", KeyCode::Digit1),
    ("digit2", KeyCode::Digit2),
    ("digit3", KeyCode::Digit3),
    ("digit4", KeyCode::Digit4),
    ("digit5", KeyCode::Digit5),
    ("digit6", KeyCode::Digit6),
    ("digit7", KeyCode::Digit7),
    ("digit8", KeyCode::Digit8),
    ("digit9", KeyCode::Digit9),
    ("minus", KeyCode::Minus),
    ("equal", KeyCode::Equal),
    ("bracketleft", KeyCode::BracketLeft),
    ("bracketright", KeyCode::BracketRight),
    ("backslash", KeyCode::Backslash),
    ("semicolon", KeyCode::Semicolon),
    ("quote", KeyCode::Quote),
    ("backquote", KeyCode::Backquote),
    ("comma", KeyCode::Comma),
    ("period", KeyCode::Period),
    ("slash", KeyCode::Slash),
    ("f1", KeyCode::F1),
    ("f2", KeyCode::F2),
    ("f3", KeyCode::F3),
    ("f4", KeyCode::F4),
    ("f5", KeyCode::F5),
    ("f6", KeyCode::F6),
    ("f7", KeyCode::F7),
    ("f8", KeyCode::F8),
    ("f9", KeyCode::F9),
    ("f10", KeyCode::F10),
    ("f11", KeyCode::F11),
    ("f12", KeyCode::F12),
    ("arrowup", KeyCode::ArrowUp),
    ("arrowdown", KeyCode::ArrowDown),
    ("arrowleft", KeyCode::ArrowLeft),
    ("arrowright", KeyCode::ArrowRight),
    ("home", KeyCode::Home),
    ("end", KeyCode::End),
    ("pageup", KeyCode::PageUp),
    ("pagedown", KeyCode::PageDown),
    ("insert", KeyCode::Insert),
    ("delete", KeyCode::Delete),
    ("enter", KeyCode::Enter),
    ("escape", KeyCode::Escape),
    ("space", KeyCode::Space),
    ("tab", KeyCode::Tab),
    ("backspace", KeyCode::Backspace),
];

/// Parse a physical key code string into a [`KeyCode`].
///
/// Supports common key code names like `"KeyA"`, `"KeyZ"`, `"Digit0"`, etc.
/// Matching is case-insensitive.  Returns `None` for unrecognised strings.
pub fn parse_physical_key_code(s: &str) -> Option<KeyCode> {
    let lower = s.to_lowercase();
    PHYSICAL_KEY_CODES
        .iter()
        .find(|(name, _)| *name == lower)
        .map(|&(_, code)| code)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Independent oracle for [`parse_physical_key_code`], one entry per
    /// accepted spelling, grouped by key-code family.
    const LETTERS: &[(&str, KeyCode)] = &[
        ("KeyA", KeyCode::KeyA),
        ("KeyB", KeyCode::KeyB),
        ("KeyC", KeyCode::KeyC),
        ("KeyD", KeyCode::KeyD),
        ("KeyE", KeyCode::KeyE),
        ("KeyF", KeyCode::KeyF),
        ("KeyG", KeyCode::KeyG),
        ("KeyH", KeyCode::KeyH),
        ("KeyI", KeyCode::KeyI),
        ("KeyJ", KeyCode::KeyJ),
        ("KeyK", KeyCode::KeyK),
        ("KeyL", KeyCode::KeyL),
        ("KeyM", KeyCode::KeyM),
        ("KeyN", KeyCode::KeyN),
        ("KeyO", KeyCode::KeyO),
        ("KeyP", KeyCode::KeyP),
        ("KeyQ", KeyCode::KeyQ),
        ("KeyR", KeyCode::KeyR),
        ("KeyS", KeyCode::KeyS),
        ("KeyT", KeyCode::KeyT),
        ("KeyU", KeyCode::KeyU),
        ("KeyV", KeyCode::KeyV),
        ("KeyW", KeyCode::KeyW),
        ("KeyX", KeyCode::KeyX),
        ("KeyY", KeyCode::KeyY),
        ("KeyZ", KeyCode::KeyZ),
    ];
    const DIGITS: &[(&str, KeyCode)] = &[
        ("Digit0", KeyCode::Digit0),
        ("Digit1", KeyCode::Digit1),
        ("Digit2", KeyCode::Digit2),
        ("Digit3", KeyCode::Digit3),
        ("Digit4", KeyCode::Digit4),
        ("Digit5", KeyCode::Digit5),
        ("Digit6", KeyCode::Digit6),
        ("Digit7", KeyCode::Digit7),
        ("Digit8", KeyCode::Digit8),
        ("Digit9", KeyCode::Digit9),
    ];
    const PUNCTUATION: &[(&str, KeyCode)] = &[
        ("Minus", KeyCode::Minus),
        ("Equal", KeyCode::Equal),
        ("BracketLeft", KeyCode::BracketLeft),
        ("BracketRight", KeyCode::BracketRight),
        ("Backslash", KeyCode::Backslash),
        ("Semicolon", KeyCode::Semicolon),
        ("Quote", KeyCode::Quote),
        ("Backquote", KeyCode::Backquote),
        ("Comma", KeyCode::Comma),
        ("Period", KeyCode::Period),
        ("Slash", KeyCode::Slash),
    ];
    const FUNCTION: &[(&str, KeyCode)] = &[
        ("F1", KeyCode::F1),
        ("F2", KeyCode::F2),
        ("F3", KeyCode::F3),
        ("F4", KeyCode::F4),
        ("F5", KeyCode::F5),
        ("F6", KeyCode::F6),
        ("F7", KeyCode::F7),
        ("F8", KeyCode::F8),
        ("F9", KeyCode::F9),
        ("F10", KeyCode::F10),
        ("F11", KeyCode::F11),
        ("F12", KeyCode::F12),
    ];
    const NAVIGATION: &[(&str, KeyCode)] = &[
        ("ArrowUp", KeyCode::ArrowUp),
        ("ArrowDown", KeyCode::ArrowDown),
        ("ArrowLeft", KeyCode::ArrowLeft),
        ("ArrowRight", KeyCode::ArrowRight),
        ("Home", KeyCode::Home),
        ("End", KeyCode::End),
        ("PageUp", KeyCode::PageUp),
        ("PageDown", KeyCode::PageDown),
        ("Insert", KeyCode::Insert),
        ("Delete", KeyCode::Delete),
    ];
    const SPECIAL: &[(&str, KeyCode)] = &[
        ("Enter", KeyCode::Enter),
        ("Escape", KeyCode::Escape),
        ("Space", KeyCode::Space),
        ("Tab", KeyCode::Tab),
        ("Backspace", KeyCode::Backspace),
    ];
    const FAMILIES: &[(&str, &[(&str, KeyCode)])] = &[
        ("letters", LETTERS),
        ("digits", DIGITS),
        ("punctuation", PUNCTUATION),
        ("function", FUNCTION),
        ("navigation", NAVIGATION),
        ("special", SPECIAL),
    ];

    fn assert_family(family: &str, entries: &[(&str, KeyCode)]) {
        for &(name, code) in entries {
            for spelling in [name.to_string(), name.to_lowercase(), name.to_uppercase()] {
                assert_eq!(
                    parse_physical_key_code(&spelling),
                    Some(code),
                    "{family}: {spelling:?} must parse to {code:?}"
                );
            }
        }
    }

    #[test]
    fn physical_letters_parse() {
        assert_family("letters", LETTERS);
    }

    #[test]
    fn physical_digits_parse() {
        assert_family("digits", DIGITS);
    }

    #[test]
    fn physical_punctuation_parses() {
        assert_family("punctuation", PUNCTUATION);
    }

    #[test]
    fn physical_function_keys_parse() {
        assert_family("function", FUNCTION);
    }

    #[test]
    fn physical_navigation_keys_parse() {
        assert_family("navigation", NAVIGATION);
    }

    #[test]
    fn physical_special_keys_parse() {
        assert_family("special", SPECIAL);
    }

    #[test]
    fn physical_oracle_has_no_duplicate_spellings() {
        let mut seen = std::collections::HashSet::new();
        for (_, entries) in FAMILIES {
            for (name, _) in *entries {
                assert!(seen.insert(name.to_lowercase()), "duplicate {name}");
            }
        }
        assert_eq!(seen.len(), 74);
    }

    /// Aliases the named-key parser accepts but the physical parser does
    /// not, plus out-of-range and empty inputs.
    #[test]
    fn physical_rejects_unknown_spellings() {
        for input in [
            "",
            "a",
            "Key",
            "KeyAA",
            "Digit10",
            "F0",
            "F13",
            "F24",
            "pgup",
            "pgdn",
            "up",
            "down",
            "left",
            "right",
            "esc",
            "return",
            "del",
            "ins",
            "Numpad0",
            "NumpadEnter",
            "ShiftLeft",
            "ControlLeft",
            "AltLeft",
            "SuperLeft",
            "CapsLock",
            "IntlBackslash",
            " KeyA",
            "KeyA ",
            "Key A",
        ] {
            assert_eq!(
                parse_physical_key_code(input),
                None,
                "{input:?} must not parse as a physical key code"
            );
        }
    }

    /// Unicode lowercasing folds KELVIN SIGN (U+212A) to ASCII `k`; the
    /// parser lowercases with `str::to_lowercase`, so this spelling parses.
    #[test]
    fn physical_lowercasing_is_unicode_aware() {
        assert_eq!(parse_physical_key_code("\u{212A}eyA"), Some(KeyCode::KeyA));
    }
}

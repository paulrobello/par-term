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

/// Parse a physical key code string into a [`KeyCode`].
///
/// Supports common key code names like `"KeyA"`, `"KeyZ"`, `"Digit0"`, etc.
/// Matching is case-insensitive.  Returns `None` for unrecognised strings.
pub fn parse_physical_key_code(s: &str) -> Option<KeyCode> {
    match s.to_lowercase().as_str() {
        // Letter keys
        "keya" => Some(KeyCode::KeyA),
        "keyb" => Some(KeyCode::KeyB),
        "keyc" => Some(KeyCode::KeyC),
        "keyd" => Some(KeyCode::KeyD),
        "keye" => Some(KeyCode::KeyE),
        "keyf" => Some(KeyCode::KeyF),
        "keyg" => Some(KeyCode::KeyG),
        "keyh" => Some(KeyCode::KeyH),
        "keyi" => Some(KeyCode::KeyI),
        "keyj" => Some(KeyCode::KeyJ),
        "keyk" => Some(KeyCode::KeyK),
        "keyl" => Some(KeyCode::KeyL),
        "keym" => Some(KeyCode::KeyM),
        "keyn" => Some(KeyCode::KeyN),
        "keyo" => Some(KeyCode::KeyO),
        "keyp" => Some(KeyCode::KeyP),
        "keyq" => Some(KeyCode::KeyQ),
        "keyr" => Some(KeyCode::KeyR),
        "keys" => Some(KeyCode::KeyS),
        "keyt" => Some(KeyCode::KeyT),
        "keyu" => Some(KeyCode::KeyU),
        "keyv" => Some(KeyCode::KeyV),
        "keyw" => Some(KeyCode::KeyW),
        "keyx" => Some(KeyCode::KeyX),
        "keyy" => Some(KeyCode::KeyY),
        "keyz" => Some(KeyCode::KeyZ),

        // Number row
        "digit0" => Some(KeyCode::Digit0),
        "digit1" => Some(KeyCode::Digit1),
        "digit2" => Some(KeyCode::Digit2),
        "digit3" => Some(KeyCode::Digit3),
        "digit4" => Some(KeyCode::Digit4),
        "digit5" => Some(KeyCode::Digit5),
        "digit6" => Some(KeyCode::Digit6),
        "digit7" => Some(KeyCode::Digit7),
        "digit8" => Some(KeyCode::Digit8),
        "digit9" => Some(KeyCode::Digit9),

        // Punctuation/symbols by position
        "minus" => Some(KeyCode::Minus),
        "equal" => Some(KeyCode::Equal),
        "bracketleft" => Some(KeyCode::BracketLeft),
        "bracketright" => Some(KeyCode::BracketRight),
        "backslash" => Some(KeyCode::Backslash),
        "semicolon" => Some(KeyCode::Semicolon),
        "quote" => Some(KeyCode::Quote),
        "backquote" => Some(KeyCode::Backquote),
        "comma" => Some(KeyCode::Comma),
        "period" => Some(KeyCode::Period),
        "slash" => Some(KeyCode::Slash),

        // Function keys
        "f1" => Some(KeyCode::F1),
        "f2" => Some(KeyCode::F2),
        "f3" => Some(KeyCode::F3),
        "f4" => Some(KeyCode::F4),
        "f5" => Some(KeyCode::F5),
        "f6" => Some(KeyCode::F6),
        "f7" => Some(KeyCode::F7),
        "f8" => Some(KeyCode::F8),
        "f9" => Some(KeyCode::F9),
        "f10" => Some(KeyCode::F10),
        "f11" => Some(KeyCode::F11),
        "f12" => Some(KeyCode::F12),

        // Navigation keys
        "arrowup" => Some(KeyCode::ArrowUp),
        "arrowdown" => Some(KeyCode::ArrowDown),
        "arrowleft" => Some(KeyCode::ArrowLeft),
        "arrowright" => Some(KeyCode::ArrowRight),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "pageup" => Some(KeyCode::PageUp),
        "pagedown" => Some(KeyCode::PageDown),
        "insert" => Some(KeyCode::Insert),
        "delete" => Some(KeyCode::Delete),

        // Special keys
        "enter" => Some(KeyCode::Enter),
        "escape" => Some(KeyCode::Escape),
        "space" => Some(KeyCode::Space),
        "tab" => Some(KeyCode::Tab),
        "backspace" => Some(KeyCode::Backspace),

        _ => None,
    }
}

//! Key combination parser.
//!
//! Parses human-readable key strings like "Ctrl+Shift+B" into KeyCombo structs.
//! Also supports physical key codes for language-agnostic bindings (e.g., "Ctrl+[KeyZ]").

use crate::platform;
use std::fmt;
use winit::keyboard::{KeyCode, NamedKey};

/// Error type for key parsing failures.
#[derive(Debug, Clone)]
pub struct ParseError(String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ParseError {}

/// Set of active modifiers for a key combination.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
    /// If true, this represents CmdOrCtrl (Cmd on macOS, Ctrl elsewhere)
    pub cmd_or_ctrl: bool,
}

/// A parsed key combination (modifiers + key).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub modifiers: Modifiers,
    pub key: ParsedKey,
}

impl fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();

        if self.modifiers.cmd_or_ctrl {
            parts.push("CmdOrCtrl".to_string());
        }
        if self.modifiers.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.modifiers.alt {
            parts.push("Alt".to_string());
        }
        if self.modifiers.shift {
            parts.push("Shift".to_string());
        }
        if self.modifiers.super_key {
            parts.push("Super".to_string());
        }

        match &self.key {
            ParsedKey::Character(c) => parts.push(c.to_string()),
            ParsedKey::Named(n) => parts.push(format!("{:?}", n)),
            ParsedKey::Physical(k) => parts.push(format!("[{:?}]", k)),
        }

        write!(f, "{}", parts.join("+"))
    }
}

/// The actual key (either a character or a named key).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ParsedKey {
    /// A single character key (e.g., 'a', 'B', '1')
    Character(char),
    /// A named key (e.g., F1, Enter, Escape)
    Named(NamedKey),
    /// A physical key code (e.g., KeyZ, KeyA) for language-agnostic bindings
    /// This matches by key position rather than character produced.
    Physical(KeyCode),
}

/// Parse a key combination string into a KeyCombo.
///
/// Supported format: "Modifier+Modifier+Key"
///
/// Modifiers:
/// - `Ctrl`, `Control` - Control key
/// - `Alt`, `Option` - Alt/Option key
/// - `Shift` - Shift key
/// - `Super`, `Cmd`, `Command`, `Meta`, `Win` - Super/Cmd key
/// - `CmdOrCtrl` - Cmd on macOS, Ctrl on other platforms
///
/// Keys:
/// - Single characters: `A`, `B`, `1`, etc.
/// - Named keys: `F1`-`F12`, `Enter`, `Escape`, `Space`, `Tab`, etc.
pub fn parse_key_combo(s: &str) -> Result<KeyCombo, ParseError> {
    let parts: Vec<&str> = s.split('+').map(str::trim).collect();

    if parts.is_empty() {
        return Err(ParseError("Empty key combination".to_string()));
    }

    let mut modifiers = Modifiers::default();
    let mut key_part = None;

    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;
        let part_lower = part.to_lowercase();

        // Check if this is a modifier
        let is_modifier = match part_lower.as_str() {
            "ctrl" | "control" => {
                modifiers.ctrl = true;
                true
            }
            "alt" | "option" => {
                modifiers.alt = true;
                true
            }
            "shift" => {
                modifiers.shift = true;
                true
            }
            "super" | "cmd" | "command" | "meta" | "win" => {
                modifiers.super_key = true;
                true
            }
            "cmdorctrl" => {
                modifiers.cmd_or_ctrl = true;
                true
            }
            _ => false,
        };

        if !is_modifier {
            if key_part.is_some() {
                return Err(ParseError(format!(
                    "Multiple keys specified: already have key, found '{}'",
                    part
                )));
            }
            key_part = Some(*part);
        } else if is_last {
            // Last part is a modifier with no key - invalid
            return Err(ParseError(
                "Key combination ends with modifier, no key specified".to_string(),
            ));
        }
    }

    let key_str = key_part.ok_or_else(|| ParseError("No key specified".to_string()))?;
    let key = parse_key(key_str)?;

    Ok(KeyCombo { modifiers, key })
}

/// Parse a key string into a ParsedKey.
fn parse_key(s: &str) -> Result<ParsedKey, ParseError> {
    // Check for physical key syntax: [KeyCode] (e.g., [KeyZ], [KeyA])
    if s.starts_with('[') && s.ends_with(']') {
        let code_str = &s[1..s.len() - 1];
        if let Some(code) = platform::parse_physical_key_code(code_str) {
            return Ok(ParsedKey::Physical(code));
        }
        return Err(ParseError(format!(
            "Unknown physical key code: '{}'",
            code_str
        )));
    }

    // Try named keys first (case-insensitive)
    if let Some(named) = platform::parse_named_key(s) {
        return Ok(ParsedKey::Named(named));
    }

    // Single character
    let chars: Vec<char> = s.chars().collect();
    if chars.len() == 1 {
        return Ok(ParsedKey::Character(chars[0].to_ascii_uppercase()));
    }

    Err(ParseError(format!("Unknown key: '{}'", s)))
}

/// Convert a parsed `KeyCombo` into terminal byte sequence(s).
///
/// Maps key combinations to their terminal escape sequences:
/// - Ctrl+Character: control code (char - 'a' + 1)
/// - Named keys (Enter, Tab, arrows, F-keys): standard escape sequences
/// - Plain characters: UTF-8 bytes
/// - Alt+key: ESC prefix + key bytes
pub fn key_combo_to_bytes(combo: &KeyCombo) -> Result<Vec<u8>, String> {
    let has_ctrl = combo.modifiers.ctrl || combo.modifiers.cmd_or_ctrl;
    let has_alt = combo.modifiers.alt;

    match &combo.key {
        ParsedKey::Character(c) => {
            if has_ctrl {
                // Control codes: Ctrl+A = 0x01, Ctrl+C = 0x03, etc.
                let upper = c.to_ascii_uppercase();
                if upper.is_ascii_uppercase() {
                    let code = upper as u8 - b'A' + 1;
                    let bytes = vec![code];
                    if has_alt {
                        // Alt+Ctrl+Key = ESC + control code
                        let mut result = vec![0x1b];
                        result.extend_from_slice(&bytes);
                        Ok(result)
                    } else {
                        Ok(bytes)
                    }
                } else {
                    Err(format!("Cannot compute Ctrl code for '{}'", c))
                }
            } else if has_alt {
                // Alt+Key = ESC + key
                let mut bytes = vec![0x1b];
                let mut buf = [0u8; 4];
                let encoded = c.encode_utf8(&mut buf);
                bytes.extend_from_slice(encoded.as_bytes());
                Ok(bytes)
            } else {
                // Plain character
                let mut buf = [0u8; 4];
                let encoded = c.encode_utf8(&mut buf);
                Ok(encoded.as_bytes().to_vec())
            }
        }
        ParsedKey::Named(named) => {
            let seq: &[u8] = match named {
                NamedKey::Enter => b"\r",
                NamedKey::Tab => b"\t",
                NamedKey::Space => b" ",
                NamedKey::Backspace => b"\x7f",
                NamedKey::Escape => b"\x1b",
                NamedKey::Insert => b"\x1b[2~",
                NamedKey::Delete => b"\x1b[3~",
                NamedKey::ArrowUp => b"\x1b[A",
                NamedKey::ArrowDown => b"\x1b[B",
                NamedKey::ArrowRight => b"\x1b[C",
                NamedKey::ArrowLeft => b"\x1b[D",
                NamedKey::Home => b"\x1b[H",
                NamedKey::End => b"\x1b[F",
                NamedKey::PageUp => b"\x1b[5~",
                NamedKey::PageDown => b"\x1b[6~",
                NamedKey::F1 => b"\x1bOP",
                NamedKey::F2 => b"\x1bOQ",
                NamedKey::F3 => b"\x1bOR",
                NamedKey::F4 => b"\x1bOS",
                NamedKey::F5 => b"\x1b[15~",
                NamedKey::F6 => b"\x1b[17~",
                NamedKey::F7 => b"\x1b[18~",
                NamedKey::F8 => b"\x1b[19~",
                NamedKey::F9 => b"\x1b[20~",
                NamedKey::F10 => b"\x1b[21~",
                NamedKey::F11 => b"\x1b[23~",
                NamedKey::F12 => b"\x1b[24~",
                _ => return Err(format!("Unsupported named key: {:?}", named)),
            };
            let bytes = seq.to_vec();
            if has_alt {
                let mut result = vec![0x1b];
                result.extend_from_slice(&bytes);
                Ok(result)
            } else {
                Ok(bytes)
            }
        }
        ParsedKey::Physical(_code) => {
            Err("Physical key codes cannot be converted to bytes without a keyboard layout".into())
        }
    }
}

/// Parse a key sequence string into a list of byte sequences.
///
/// The input string contains whitespace-separated key combos.
/// Each key combo is parsed with `parse_key_combo()` and converted to bytes.
///
/// Example: "Up Up Down Down" → four arrow key escape sequences
/// Example: "Ctrl+C" → single \x03 byte
pub fn parse_key_sequence(keys: &str) -> Result<Vec<Vec<u8>>, String> {
    let trimmed = keys.trim();
    if trimmed.is_empty() {
        return Err("Empty key sequence".to_string());
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    let mut result = Vec::with_capacity(parts.len());

    for part in parts {
        let combo = parse_key_combo(part).map_err(|e| format!("'{}': {}", part, e))?;
        let bytes = key_combo_to_bytes(&combo)?;
        result.push(bytes);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_key() {
        let combo = parse_key_combo("A").unwrap();
        assert!(!combo.modifiers.ctrl);
        assert!(!combo.modifiers.shift);
        assert_eq!(combo.key, ParsedKey::Character('A'));
    }

    #[test]
    fn test_ctrl_key() {
        let combo = parse_key_combo("Ctrl+A").unwrap();
        assert!(combo.modifiers.ctrl);
        assert!(!combo.modifiers.shift);
        assert_eq!(combo.key, ParsedKey::Character('A'));
    }

    #[test]
    fn test_ctrl_shift_key() {
        let combo = parse_key_combo("Ctrl+Shift+B").unwrap();
        assert!(combo.modifiers.ctrl);
        assert!(combo.modifiers.shift);
        assert_eq!(combo.key, ParsedKey::Character('B'));
    }

    #[test]
    fn test_cmd_or_ctrl() {
        let combo = parse_key_combo("CmdOrCtrl+Shift+B").unwrap();
        assert!(combo.modifiers.cmd_or_ctrl);
        assert!(combo.modifiers.shift);
        assert!(!combo.modifiers.ctrl);
        assert_eq!(combo.key, ParsedKey::Character('B'));
    }

    #[test]
    fn test_function_key() {
        let combo = parse_key_combo("F5").unwrap();
        assert!(!combo.modifiers.ctrl);
        assert_eq!(combo.key, ParsedKey::Named(NamedKey::F5));
    }

    #[test]
    fn test_ctrl_function_key() {
        let combo = parse_key_combo("Ctrl+F12").unwrap();
        assert!(combo.modifiers.ctrl);
        assert_eq!(combo.key, ParsedKey::Named(NamedKey::F12));
    }

    #[test]
    fn test_case_insensitive() {
        let combo = parse_key_combo("ctrl+shift+a").unwrap();
        assert!(combo.modifiers.ctrl);
        assert!(combo.modifiers.shift);
        assert_eq!(combo.key, ParsedKey::Character('A'));
    }

    #[test]
    fn test_modifier_aliases() {
        // Control alias
        let combo = parse_key_combo("Control+A").unwrap();
        assert!(combo.modifiers.ctrl);

        // Option alias
        let combo = parse_key_combo("Option+A").unwrap();
        assert!(combo.modifiers.alt);

        // Command aliases
        let combo = parse_key_combo("Cmd+A").unwrap();
        assert!(combo.modifiers.super_key);

        let combo = parse_key_combo("Command+A").unwrap();
        assert!(combo.modifiers.super_key);

        let combo = parse_key_combo("Meta+A").unwrap();
        assert!(combo.modifiers.super_key);
    }

    #[test]
    fn test_named_key_aliases() {
        let combo = parse_key_combo("Enter").unwrap();
        assert_eq!(combo.key, ParsedKey::Named(NamedKey::Enter));

        let combo = parse_key_combo("Return").unwrap();
        assert_eq!(combo.key, ParsedKey::Named(NamedKey::Enter));

        let combo = parse_key_combo("Esc").unwrap();
        assert_eq!(combo.key, ParsedKey::Named(NamedKey::Escape));

        let combo = parse_key_combo("PgUp").unwrap();
        assert_eq!(combo.key, ParsedKey::Named(NamedKey::PageUp));
    }

    #[test]
    fn test_invalid_empty() {
        assert!(parse_key_combo("").is_err());
    }

    #[test]
    fn test_invalid_modifier_only() {
        assert!(parse_key_combo("Ctrl").is_err());
        assert!(parse_key_combo("Ctrl+Shift").is_err());
    }

    #[test]
    fn test_invalid_unknown_key() {
        assert!(parse_key_combo("Ctrl+UnknownKey").is_err());
    }

    #[test]
    fn test_display() {
        let combo = parse_key_combo("Ctrl+Shift+B").unwrap();
        let display = format!("{}", combo);
        assert!(display.contains("Ctrl"));
        assert!(display.contains("Shift"));
        assert!(display.contains("B"));
    }

    #[test]
    fn test_physical_key() {
        let combo = parse_key_combo("Ctrl+[KeyZ]").unwrap();
        assert!(combo.modifiers.ctrl);
        assert_eq!(combo.key, ParsedKey::Physical(KeyCode::KeyZ));
    }

    #[test]
    fn test_physical_key_case_insensitive() {
        let combo = parse_key_combo("Ctrl+[keya]").unwrap();
        assert!(combo.modifiers.ctrl);
        assert_eq!(combo.key, ParsedKey::Physical(KeyCode::KeyA));
    }

    #[test]
    fn test_physical_key_display() {
        let combo = parse_key_combo("Ctrl+[KeyZ]").unwrap();
        let display = format!("{}", combo);
        assert!(display.contains("Ctrl"));
        assert!(display.contains("[KeyZ]"));
    }

    #[test]
    fn test_invalid_physical_key() {
        assert!(parse_key_combo("Ctrl+[Unknown]").is_err());
    }

    // Tests for key_combo_to_bytes and parse_key_sequence

    #[test]
    fn test_key_combo_to_bytes_enter() {
        let combo = parse_key_combo("Enter").unwrap();
        let bytes = key_combo_to_bytes(&combo).unwrap();
        assert_eq!(bytes, b"\r");
    }

    #[test]
    fn test_key_combo_to_bytes_tab() {
        let combo = parse_key_combo("Tab").unwrap();
        let bytes = key_combo_to_bytes(&combo).unwrap();
        assert_eq!(bytes, b"\t");
    }

    #[test]
    fn test_key_combo_to_bytes_ctrl_c() {
        let combo = parse_key_combo("Ctrl+C").unwrap();
        let bytes = key_combo_to_bytes(&combo).unwrap();
        assert_eq!(bytes, vec![0x03]); // Ctrl+C = ETX
    }

    #[test]
    fn test_key_combo_to_bytes_ctrl_a() {
        let combo = parse_key_combo("Ctrl+A").unwrap();
        let bytes = key_combo_to_bytes(&combo).unwrap();
        assert_eq!(bytes, vec![0x01]); // Ctrl+A = SOH
    }

    #[test]
    fn test_key_combo_to_bytes_ctrl_z() {
        let combo = parse_key_combo("Ctrl+Z").unwrap();
        let bytes = key_combo_to_bytes(&combo).unwrap();
        assert_eq!(bytes, vec![0x1a]); // Ctrl+Z = SUB
    }

    #[test]
    fn test_key_combo_to_bytes_arrow_up() {
        let combo = parse_key_combo("Up").unwrap();
        let bytes = key_combo_to_bytes(&combo).unwrap();
        assert_eq!(bytes, b"\x1b[A");
    }

    #[test]
    fn test_key_combo_to_bytes_arrow_down() {
        let combo = parse_key_combo("Down").unwrap();
        let bytes = key_combo_to_bytes(&combo).unwrap();
        assert_eq!(bytes, b"\x1b[B");
    }

    #[test]
    fn test_key_combo_to_bytes_f5() {
        let combo = parse_key_combo("F5").unwrap();
        let bytes = key_combo_to_bytes(&combo).unwrap();
        assert_eq!(bytes, b"\x1b[15~");
    }

    #[test]
    fn test_key_combo_to_bytes_escape() {
        let combo = parse_key_combo("Escape").unwrap();
        let bytes = key_combo_to_bytes(&combo).unwrap();
        assert_eq!(bytes, b"\x1b");
    }

    #[test]
    fn test_key_combo_to_bytes_plain_char() {
        let combo = parse_key_combo("A").unwrap();
        let bytes = key_combo_to_bytes(&combo).unwrap();
        assert_eq!(bytes, b"A");
    }

    #[test]
    fn test_key_combo_to_bytes_alt_key() {
        let combo = parse_key_combo("Alt+A").unwrap();
        let bytes = key_combo_to_bytes(&combo).unwrap();
        assert_eq!(bytes, vec![0x1b, b'A']);
    }

    #[test]
    fn test_parse_key_sequence_single() {
        let seqs = parse_key_sequence("Enter").unwrap();
        assert_eq!(seqs.len(), 1);
        assert_eq!(seqs[0], b"\r");
    }

    #[test]
    fn test_parse_key_sequence_ctrl_c() {
        let seqs = parse_key_sequence("Ctrl+C").unwrap();
        assert_eq!(seqs.len(), 1);
        assert_eq!(seqs[0], vec![0x03]);
    }

    #[test]
    fn test_parse_key_sequence_multi_keys() {
        let seqs = parse_key_sequence("Up Up Down Down").unwrap();
        assert_eq!(seqs.len(), 4);
        assert_eq!(seqs[0], b"\x1b[A");
        assert_eq!(seqs[1], b"\x1b[A");
        assert_eq!(seqs[2], b"\x1b[B");
        assert_eq!(seqs[3], b"\x1b[B");
    }

    #[test]
    fn test_parse_key_sequence_empty() {
        assert!(parse_key_sequence("").is_err());
        assert!(parse_key_sequence("   ").is_err());
    }

    #[test]
    fn test_parse_key_sequence_invalid_key() {
        assert!(parse_key_sequence("InvalidKey").is_err());
    }

    #[test]
    fn test_key_combo_to_bytes_physical_key_error() {
        let combo = parse_key_combo("Ctrl+[KeyZ]").unwrap();
        assert!(key_combo_to_bytes(&combo).is_err());
    }
}

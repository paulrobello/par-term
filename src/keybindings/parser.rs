//! Key combination parser.
//!
//! Parses human-readable key strings like "Ctrl+Shift+B" into KeyCombo structs.

use std::fmt;
use winit::keyboard::NamedKey;

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
    // Try named keys first (case-insensitive)
    if let Some(named) = parse_named_key(s) {
        return Ok(ParsedKey::Named(named));
    }

    // Single character
    let chars: Vec<char> = s.chars().collect();
    if chars.len() == 1 {
        return Ok(ParsedKey::Character(chars[0].to_ascii_uppercase()));
    }

    Err(ParseError(format!("Unknown key: '{}'", s)))
}

/// Parse a named key string into a NamedKey.
fn parse_named_key(s: &str) -> Option<NamedKey> {
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
}

//! Key event matching.
//!
//! Matches winit KeyEvents against parsed KeyCombos.

use super::parser::{KeyCombo, Modifiers, ParsedKey};
use winit::event::{KeyEvent, Modifiers as WinitModifiers};
use winit::keyboard::{Key, NamedKey};

/// Matcher for comparing winit key events against keybindings.
#[derive(Debug)]
pub struct KeybindingMatcher {
    /// Active modifiers from the event
    modifiers: Modifiers,
    /// The key from the event
    key: Option<MatchKey>,
}

/// Normalized key for matching purposes.
#[derive(Debug)]
enum MatchKey {
    Character(char),
    Named(NamedKey),
}

impl KeybindingMatcher {
    /// Create a matcher from a winit key event.
    pub fn from_event(event: &KeyEvent, modifiers: &WinitModifiers) -> Self {
        let mods = Modifiers {
            ctrl: modifiers.state().control_key(),
            alt: modifiers.state().alt_key(),
            shift: modifiers.state().shift_key(),
            super_key: modifiers.state().super_key(),
            cmd_or_ctrl: false, // Resolved during matching
        };

        let key = match &event.logical_key {
            Key::Character(c) => {
                // Get the first character, uppercased for case-insensitive matching
                c.chars()
                    .next()
                    .map(|ch| MatchKey::Character(ch.to_ascii_uppercase()))
            }
            Key::Named(named) => Some(MatchKey::Named(*named)),
            _ => None,
        };

        Self {
            modifiers: mods,
            key,
        }
    }

    /// Check if this event matches the given key combo.
    pub fn matches(&self, combo: &KeyCombo) -> bool {
        // Check key first (quick rejection)
        let key_matches = match (&self.key, &combo.key) {
            (Some(MatchKey::Character(event_char)), ParsedKey::Character(combo_char)) => {
                // Case-insensitive character comparison
                event_char.eq_ignore_ascii_case(combo_char)
            }
            (Some(MatchKey::Named(event_named)), ParsedKey::Named(combo_named)) => {
                event_named == combo_named
            }
            _ => false,
        };

        if !key_matches {
            return false;
        }

        // Check modifiers
        self.modifiers_match(&combo.modifiers)
    }

    /// Check if modifiers match, handling CmdOrCtrl specially.
    fn modifiers_match(&self, combo_mods: &Modifiers) -> bool {
        // Handle CmdOrCtrl: on macOS it means Super, elsewhere it means Ctrl
        let (expected_ctrl, expected_super) = if combo_mods.cmd_or_ctrl {
            #[cfg(target_os = "macos")]
            {
                (combo_mods.ctrl, true) // CmdOrCtrl -> Super on macOS
            }
            #[cfg(not(target_os = "macos"))]
            {
                (true, combo_mods.super_key) // CmdOrCtrl -> Ctrl on other platforms
            }
        } else {
            (combo_mods.ctrl, combo_mods.super_key)
        };

        // Check each modifier
        self.modifiers.ctrl == expected_ctrl
            && self.modifiers.alt == combo_mods.alt
            && self.modifiers.shift == combo_mods.shift
            && self.modifiers.super_key == expected_super
    }
}

// Note: Integration tests for KeybindingMatcher require constructing winit KeyEvent
// which has private fields. The matcher is tested indirectly through the registry tests
// and the actual runtime behavior.
//
// The matching logic (modifiers_match) is tested through the Modifiers struct which
// we can construct directly.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybindings::parser::parse_key_combo;

    /// Test that Modifiers comparison works correctly for CmdOrCtrl
    #[test]
    fn test_cmd_or_ctrl_modifiers() {
        // Create a matcher with specific modifiers manually
        let matcher_ctrl_shift = KeybindingMatcher {
            modifiers: Modifiers {
                ctrl: true,
                alt: false,
                shift: true,
                super_key: false,
                cmd_or_ctrl: false,
            },
            key: Some(MatchKey::Character('B')),
        };

        let matcher_super_shift = KeybindingMatcher {
            modifiers: Modifiers {
                ctrl: false,
                alt: false,
                shift: true,
                super_key: true,
                cmd_or_ctrl: false,
            },
            key: Some(MatchKey::Character('B')),
        };

        let combo = parse_key_combo("CmdOrCtrl+Shift+B").unwrap();

        // On macOS, CmdOrCtrl means Super
        #[cfg(target_os = "macos")]
        {
            assert!(matcher_super_shift.matches(&combo));
            assert!(!matcher_ctrl_shift.matches(&combo));
        }

        // On non-macOS, CmdOrCtrl means Ctrl
        #[cfg(not(target_os = "macos"))]
        {
            assert!(matcher_ctrl_shift.matches(&combo));
            assert!(!matcher_super_shift.matches(&combo));
        }
    }

    /// Test character key matching (case insensitive)
    #[test]
    fn test_character_matching() {
        let combo = parse_key_combo("Ctrl+A").unwrap();

        // Lowercase should match
        let matcher_lower = KeybindingMatcher {
            modifiers: Modifiers {
                ctrl: true,
                alt: false,
                shift: false,
                super_key: false,
                cmd_or_ctrl: false,
            },
            key: Some(MatchKey::Character('a')),
        };
        assert!(matcher_lower.matches(&combo));

        // Uppercase should match
        let matcher_upper = KeybindingMatcher {
            modifiers: Modifiers {
                ctrl: true,
                alt: false,
                shift: false,
                super_key: false,
                cmd_or_ctrl: false,
            },
            key: Some(MatchKey::Character('A')),
        };
        assert!(matcher_upper.matches(&combo));

        // Different key should not match
        let matcher_wrong = KeybindingMatcher {
            modifiers: Modifiers {
                ctrl: true,
                alt: false,
                shift: false,
                super_key: false,
                cmd_or_ctrl: false,
            },
            key: Some(MatchKey::Character('B')),
        };
        assert!(!matcher_wrong.matches(&combo));
    }

    /// Test named key matching
    #[test]
    fn test_named_key_matching() {
        let combo = parse_key_combo("F5").unwrap();

        let matcher = KeybindingMatcher {
            modifiers: Modifiers::default(),
            key: Some(MatchKey::Named(NamedKey::F5)),
        };
        assert!(matcher.matches(&combo));

        let matcher_wrong = KeybindingMatcher {
            modifiers: Modifiers::default(),
            key: Some(MatchKey::Named(NamedKey::F6)),
        };
        assert!(!matcher_wrong.matches(&combo));
    }

    /// Test modifier mismatch
    #[test]
    fn test_modifier_mismatch() {
        let combo = parse_key_combo("Ctrl+Shift+B").unwrap();

        // Missing Shift
        let matcher = KeybindingMatcher {
            modifiers: Modifiers {
                ctrl: true,
                alt: false,
                shift: false,
                super_key: false,
                cmd_or_ctrl: false,
            },
            key: Some(MatchKey::Character('B')),
        };
        assert!(!matcher.matches(&combo));

        // Extra Alt
        let matcher = KeybindingMatcher {
            modifiers: Modifiers {
                ctrl: true,
                alt: true,
                shift: true,
                super_key: false,
                cmd_or_ctrl: false,
            },
            key: Some(MatchKey::Character('B')),
        };
        assert!(!matcher.matches(&combo));
    }
}

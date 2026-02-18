//! Key event matching.
//!
//! Matches winit KeyEvents against parsed KeyCombos.
//! Supports both logical key matching (character-based) and physical key matching
//! (scan code-based) for language-agnostic bindings.

use super::parser::{KeyCombo, Modifiers, ParsedKey};
use winit::event::{KeyEvent, Modifiers as WinitModifiers};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};

/// Matcher for comparing winit key events against keybindings.
#[derive(Debug)]
pub struct KeybindingMatcher {
    /// Active modifiers from the event
    modifiers: Modifiers,
    /// The logical key from the event
    key: Option<MatchKey>,
    /// The physical key code from the event (for language-agnostic matching)
    physical_key: Option<KeyCode>,
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

        // Extract physical key code
        let physical_key = match event.physical_key {
            PhysicalKey::Code(code) => Some(code),
            PhysicalKey::Unidentified(_) => None,
        };

        Self {
            modifiers: mods,
            key,
            physical_key,
        }
    }

    /// Create a matcher from a winit key event with remapped modifiers.
    ///
    /// This applies modifier remapping before matching, allowing users to customize
    /// which physical keys act as which modifiers.
    pub fn from_event_with_remapping(
        event: &KeyEvent,
        modifiers: &WinitModifiers,
        remapping: &par_term_config::ModifierRemapping,
    ) -> Self {
        use par_term_config::ModifierTarget;

        // Start with the raw modifier state
        let mut ctrl = modifiers.state().control_key();
        let mut alt = modifiers.state().alt_key();
        let mut shift = modifiers.state().shift_key();
        let mut super_key = modifiers.state().super_key();

        // Apply remapping based on which physical modifier keys are pressed
        // We need to check which specific modifier keys are pressed and remap them

        // Helper to apply a remap: if target is set, contribute to that modifier
        let apply_remap = |target: ModifierTarget,
                           ctrl: &mut bool,
                           alt: &mut bool,
                           shift: &mut bool,
                           super_key: &mut bool,
                           is_pressed: bool| {
            if !is_pressed {
                return;
            }
            match target {
                ModifierTarget::None => {} // Keep original behavior
                ModifierTarget::Ctrl => *ctrl = true,
                ModifierTarget::Alt => *alt = true,
                ModifierTarget::Shift => *shift = true,
                ModifierTarget::Super => *super_key = true,
            }
        };

        // Check if any remapping is active
        let has_remapping = remapping.left_ctrl != ModifierTarget::None
            || remapping.right_ctrl != ModifierTarget::None
            || remapping.left_alt != ModifierTarget::None
            || remapping.right_alt != ModifierTarget::None
            || remapping.left_super != ModifierTarget::None
            || remapping.right_super != ModifierTarget::None;

        if has_remapping {
            // Get the physical key to determine which specific modifier was pressed
            if let PhysicalKey::Code(code) = event.physical_key {
                // Reset modifiers if we're remapping - we'll rebuild from physical keys
                let orig_ctrl = ctrl;
                let orig_alt = alt;
                let _orig_shift = shift; // Shift is not remappable, but kept for consistency
                let orig_super = super_key;

                // Clear modifiers that are being remapped
                if remapping.left_ctrl != ModifierTarget::None
                    || remapping.right_ctrl != ModifierTarget::None
                {
                    ctrl = false;
                }
                if remapping.left_alt != ModifierTarget::None
                    || remapping.right_alt != ModifierTarget::None
                {
                    alt = false;
                }
                if remapping.left_super != ModifierTarget::None
                    || remapping.right_super != ModifierTarget::None
                {
                    super_key = false;
                }

                // Re-apply based on remapping
                // Note: We use the original modifier state to detect which modifiers are held
                // The physical key code tells us which specific key this event is for

                // For Ctrl keys
                if orig_ctrl {
                    if remapping.left_ctrl != ModifierTarget::None {
                        apply_remap(
                            remapping.left_ctrl,
                            &mut ctrl,
                            &mut alt,
                            &mut shift,
                            &mut super_key,
                            true,
                        );
                    } else if remapping.right_ctrl != ModifierTarget::None {
                        apply_remap(
                            remapping.right_ctrl,
                            &mut ctrl,
                            &mut alt,
                            &mut shift,
                            &mut super_key,
                            true,
                        );
                    } else {
                        ctrl = true; // No remap, keep original
                    }
                }

                // For Alt keys
                if orig_alt {
                    if remapping.left_alt != ModifierTarget::None {
                        apply_remap(
                            remapping.left_alt,
                            &mut ctrl,
                            &mut alt,
                            &mut shift,
                            &mut super_key,
                            true,
                        );
                    } else if remapping.right_alt != ModifierTarget::None {
                        apply_remap(
                            remapping.right_alt,
                            &mut ctrl,
                            &mut alt,
                            &mut shift,
                            &mut super_key,
                            true,
                        );
                    } else {
                        alt = true; // No remap, keep original
                    }
                }

                // For Super keys
                if orig_super {
                    if remapping.left_super != ModifierTarget::None {
                        apply_remap(
                            remapping.left_super,
                            &mut ctrl,
                            &mut alt,
                            &mut shift,
                            &mut super_key,
                            true,
                        );
                    } else if remapping.right_super != ModifierTarget::None {
                        apply_remap(
                            remapping.right_super,
                            &mut ctrl,
                            &mut alt,
                            &mut shift,
                            &mut super_key,
                            true,
                        );
                    } else {
                        super_key = true; // No remap, keep original
                    }
                }

                // Handle specific physical key remaps (for when this key IS a modifier being pressed)
                match code {
                    KeyCode::ControlLeft if remapping.left_ctrl != ModifierTarget::None => {
                        // This key itself is being remapped
                    }
                    KeyCode::ControlRight if remapping.right_ctrl != ModifierTarget::None => {}
                    KeyCode::AltLeft if remapping.left_alt != ModifierTarget::None => {}
                    KeyCode::AltRight if remapping.right_alt != ModifierTarget::None => {}
                    KeyCode::SuperLeft if remapping.left_super != ModifierTarget::None => {}
                    KeyCode::SuperRight if remapping.right_super != ModifierTarget::None => {}
                    _ => {}
                }
            }
        }

        let mods = Modifiers {
            ctrl,
            alt,
            shift,
            super_key,
            cmd_or_ctrl: false,
        };

        let key = match &event.logical_key {
            Key::Character(c) => c
                .chars()
                .next()
                .map(|ch| MatchKey::Character(ch.to_ascii_uppercase())),
            Key::Named(named) => Some(MatchKey::Named(*named)),
            _ => None,
        };

        let physical_key = match event.physical_key {
            PhysicalKey::Code(code) => Some(code),
            PhysicalKey::Unidentified(_) => None,
        };

        Self {
            modifiers: mods,
            key,
            physical_key,
        }
    }

    /// Check if this event matches the given key combo.
    pub fn matches(&self, combo: &KeyCombo) -> bool {
        self.matches_with_physical_preference(combo, false)
    }

    /// Check if this event matches the given key combo, with option to prefer physical keys.
    ///
    /// When `use_physical_keys` is true, physical key matches are attempted first for
    /// character-based keybindings, making them work consistently across keyboard layouts.
    pub fn matches_with_physical_preference(
        &self,
        combo: &KeyCombo,
        use_physical_keys: bool,
    ) -> bool {
        // Check key first (quick rejection)
        let key_matches = match (&combo.key, use_physical_keys) {
            // Physical key binding - always match by physical key
            (ParsedKey::Physical(combo_code), _) => self.physical_key.as_ref() == Some(combo_code),
            // Character binding with physical key preference enabled
            (ParsedKey::Character(combo_char), true) => {
                // Try to match by physical key position first
                if let Some(physical) = self.physical_key {
                    physical_key_matches_char(physical, *combo_char)
                } else if let Some(MatchKey::Character(event_char)) = &self.key {
                    // Fall back to logical match if no physical key
                    event_char.eq_ignore_ascii_case(combo_char)
                } else {
                    false
                }
            }
            // Character binding with logical matching (default)
            (ParsedKey::Character(combo_char), false) => {
                if let Some(MatchKey::Character(event_char)) = &self.key {
                    event_char.eq_ignore_ascii_case(combo_char)
                } else {
                    false
                }
            }
            // Named key binding
            (ParsedKey::Named(combo_named), _) => {
                if let Some(MatchKey::Named(event_named)) = &self.key {
                    event_named == combo_named
                } else {
                    false
                }
            }
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

/// Check if a physical key code corresponds to a character on a QWERTY layout.
///
/// This maps physical key positions (scan codes) to the characters they produce
/// on a US QWERTY keyboard, enabling language-agnostic keybindings.
fn physical_key_matches_char(code: KeyCode, ch: char) -> bool {
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

// Note: Integration tests for KeybindingMatcher require constructing winit KeyEvent
// which has private fields. The matcher is tested indirectly through the registry tests
// and the actual runtime behavior.
//
// The matching logic (modifiers_match) is tested through the Modifiers struct which
// we can construct directly.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_key_combo;

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
            physical_key: Some(KeyCode::KeyB),
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
            physical_key: Some(KeyCode::KeyB),
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
            physical_key: Some(KeyCode::KeyA),
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
            physical_key: Some(KeyCode::KeyA),
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
            physical_key: Some(KeyCode::KeyB),
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
            physical_key: Some(KeyCode::F5),
        };
        assert!(matcher.matches(&combo));

        let matcher_wrong = KeybindingMatcher {
            modifiers: Modifiers::default(),
            key: Some(MatchKey::Named(NamedKey::F6)),
            physical_key: Some(KeyCode::F6),
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
            physical_key: Some(KeyCode::KeyB),
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
            physical_key: Some(KeyCode::KeyB),
        };
        assert!(!matcher.matches(&combo));
    }

    /// Test physical key matching
    #[test]
    fn test_physical_key_matching() {
        // Physical key binding should match by scan code
        let combo = parse_key_combo("Ctrl+[KeyZ]").unwrap();

        // Should match when physical key is KeyZ
        let matcher = KeybindingMatcher {
            modifiers: Modifiers {
                ctrl: true,
                alt: false,
                shift: false,
                super_key: false,
                cmd_or_ctrl: false,
            },
            key: Some(MatchKey::Character('W')), // Different logical key (e.g., AZERTY)
            physical_key: Some(KeyCode::KeyZ),
        };
        assert!(matcher.matches(&combo));

        // Should not match when physical key is different
        let matcher_wrong = KeybindingMatcher {
            modifiers: Modifiers {
                ctrl: true,
                alt: false,
                shift: false,
                super_key: false,
                cmd_or_ctrl: false,
            },
            key: Some(MatchKey::Character('Z')),
            physical_key: Some(KeyCode::KeyW),
        };
        assert!(!matcher_wrong.matches(&combo));
    }

    /// Test physical key preference mode
    #[test]
    fn test_physical_key_preference() {
        // Character binding with physical preference enabled
        let combo = parse_key_combo("Ctrl+Z").unwrap();

        // On AZERTY, physical KeyZ produces 'W', but with physical preference
        // we match by position (QWERTY Z position)
        let matcher_azerty = KeybindingMatcher {
            modifiers: Modifiers {
                ctrl: true,
                alt: false,
                shift: false,
                super_key: false,
                cmd_or_ctrl: false,
            },
            key: Some(MatchKey::Character('W')), // AZERTY produces 'W'
            physical_key: Some(KeyCode::KeyZ),   // But physical position is KeyZ
        };

        // Without physical preference, should NOT match (W != Z)
        assert!(!matcher_azerty.matches_with_physical_preference(&combo, false));

        // With physical preference, SHOULD match (KeyZ position)
        assert!(matcher_azerty.matches_with_physical_preference(&combo, true));
    }
}

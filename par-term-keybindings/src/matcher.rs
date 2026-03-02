//! Key event matching.
//!
//! Matches winit KeyEvents against parsed KeyCombos.
//! Supports both logical key matching (character-based) and physical key matching
//! (scan code-based) for language-agnostic bindings.

use super::parser::{KeyCombo, Modifiers, ParsedKey};
use crate::platform;
use winit::event::{KeyEvent, Modifiers as WinitModifiers};
use winit::keyboard::{Key, KeyCode, ModifiersKeyState, NamedKey, PhysicalKey};

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
            // Get the physical key to determine which specific modifier was pressed.
            // We only proceed if we have a known physical key code.
            if let PhysicalKey::Code(_code) = event.physical_key {
                // Use winit's per-side modifier state to determine which specific modifier
                // keys are currently held. ModifiersKeyState::Pressed means that side is
                // held; ModifiersKeyState::Unknown means the platform cannot distinguish
                // sides (in that case we treat the modifier as active on both sides so
                // that at least one remapping fires).
                let left_ctrl_held = modifiers.lcontrol_state() == ModifiersKeyState::Pressed
                    || (ctrl
                        && modifiers.lcontrol_state() == ModifiersKeyState::Unknown
                        && modifiers.rcontrol_state() == ModifiersKeyState::Unknown);
                let right_ctrl_held = modifiers.rcontrol_state() == ModifiersKeyState::Pressed
                    || (ctrl
                        && modifiers.lcontrol_state() == ModifiersKeyState::Unknown
                        && modifiers.rcontrol_state() == ModifiersKeyState::Unknown);

                let left_alt_held = modifiers.lalt_state() == ModifiersKeyState::Pressed
                    || (alt
                        && modifiers.lalt_state() == ModifiersKeyState::Unknown
                        && modifiers.ralt_state() == ModifiersKeyState::Unknown);
                let right_alt_held = modifiers.ralt_state() == ModifiersKeyState::Pressed
                    || (alt
                        && modifiers.lalt_state() == ModifiersKeyState::Unknown
                        && modifiers.ralt_state() == ModifiersKeyState::Unknown);

                let left_super_held = modifiers.lsuper_state() == ModifiersKeyState::Pressed
                    || (super_key
                        && modifiers.lsuper_state() == ModifiersKeyState::Unknown
                        && modifiers.rsuper_state() == ModifiersKeyState::Unknown);
                let right_super_held = modifiers.rsuper_state() == ModifiersKeyState::Pressed
                    || (super_key
                        && modifiers.lsuper_state() == ModifiersKeyState::Unknown
                        && modifiers.rsuper_state() == ModifiersKeyState::Unknown);

                // Clear modifiers that are being remapped — we will re-apply them
                // side-by-side from the individual left/right states below.
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

                // Re-apply each side independently so that when both left and right of
                // the same modifier type are held, both remappings take effect (rather
                // than the left mapping unconditionally winning).

                // Left Ctrl
                if left_ctrl_held {
                    if remapping.left_ctrl != ModifierTarget::None {
                        apply_remap(
                            remapping.left_ctrl,
                            &mut ctrl,
                            &mut alt,
                            &mut shift,
                            &mut super_key,
                            true,
                        );
                    } else {
                        ctrl = true; // No remap configured for left ctrl — keep it
                    }
                }

                // Right Ctrl
                if right_ctrl_held {
                    if remapping.right_ctrl != ModifierTarget::None {
                        apply_remap(
                            remapping.right_ctrl,
                            &mut ctrl,
                            &mut alt,
                            &mut shift,
                            &mut super_key,
                            true,
                        );
                    } else {
                        ctrl = true; // No remap configured for right ctrl — keep it
                    }
                }

                // Left Alt
                if left_alt_held {
                    if remapping.left_alt != ModifierTarget::None {
                        apply_remap(
                            remapping.left_alt,
                            &mut ctrl,
                            &mut alt,
                            &mut shift,
                            &mut super_key,
                            true,
                        );
                    } else {
                        alt = true; // No remap configured for left alt — keep it
                    }
                }

                // Right Alt
                if right_alt_held {
                    if remapping.right_alt != ModifierTarget::None {
                        apply_remap(
                            remapping.right_alt,
                            &mut ctrl,
                            &mut alt,
                            &mut shift,
                            &mut super_key,
                            true,
                        );
                    } else {
                        alt = true; // No remap configured for right alt — keep it
                    }
                }

                // Left Super
                if left_super_held {
                    if remapping.left_super != ModifierTarget::None {
                        apply_remap(
                            remapping.left_super,
                            &mut ctrl,
                            &mut alt,
                            &mut shift,
                            &mut super_key,
                            true,
                        );
                    } else {
                        super_key = true; // No remap configured for left super — keep it
                    }
                }

                // Right Super
                if right_super_held {
                    if remapping.right_super != ModifierTarget::None {
                        apply_remap(
                            remapping.right_super,
                            &mut ctrl,
                            &mut alt,
                            &mut shift,
                            &mut super_key,
                            true,
                        );
                    } else {
                        super_key = true; // No remap configured for right super — keep it
                    }
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
                    platform::physical_key_matches_char(physical, *combo_char)
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
        let (expected_ctrl, expected_super) = platform::resolve_cmd_or_ctrl(
            combo_mods.cmd_or_ctrl,
            combo_mods.ctrl,
            combo_mods.super_key,
        );

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

    /// Test that modifier remapping applied to left-only uses left remapping,
    /// and right-only uses right remapping (using manually constructed matchers
    /// that represent the post-remapping state).
    ///
    /// Note: from_event_with_remapping() uses winit's lcontrol_state() /
    /// rcontrol_state() (etc.) to distinguish sides. When both return Unknown
    /// (platforms that cannot distinguish sides), it treats the modifier as
    /// active on both sides, which matches existing behaviour. We verify the
    /// matcher output here via manually-constructed KeybindingMatcher instances
    /// because winit::KeyEvent has private fields that prevent construction in
    /// tests.
    #[test]
    fn test_modifier_remapping_left_right_distinction() {
        // Simulate: RightCtrl remapped to Alt, LeftCtrl kept as Ctrl.
        // When only RightCtrl is held the result should be Alt=true, Ctrl=false.
        let matcher_right_ctrl_as_alt = KeybindingMatcher {
            modifiers: Modifiers {
                ctrl: false,
                alt: true, // right ctrl remapped to alt
                shift: false,
                super_key: false,
                cmd_or_ctrl: false,
            },
            key: Some(MatchKey::Character('A')),
            physical_key: Some(KeyCode::KeyA),
        };
        let combo_alt_a = parse_key_combo("Alt+A").unwrap();
        let combo_ctrl_a = parse_key_combo("Ctrl+A").unwrap();
        assert!(matcher_right_ctrl_as_alt.matches(&combo_alt_a));
        assert!(!matcher_right_ctrl_as_alt.matches(&combo_ctrl_a));

        // Simulate: LeftCtrl remapped to Super, RightCtrl kept as Ctrl.
        // When only LeftCtrl is held the result should be super_key=true, ctrl=false.
        let matcher_left_ctrl_as_super = KeybindingMatcher {
            modifiers: Modifiers {
                ctrl: false,
                alt: false,
                shift: false,
                super_key: true, // left ctrl remapped to super
                cmd_or_ctrl: false,
            },
            key: Some(MatchKey::Character('A')),
            physical_key: Some(KeyCode::KeyA),
        };
        let combo_super_a = parse_key_combo("Super+A").unwrap();
        assert!(matcher_left_ctrl_as_super.matches(&combo_super_a));
        assert!(!matcher_left_ctrl_as_super.matches(&combo_ctrl_a));

        // Simulate: both LeftCtrl (→ Super) and RightCtrl (→ Alt) held simultaneously.
        // Both remappings apply: super_key=true AND alt=true.
        let matcher_both_ctrl_remapped = KeybindingMatcher {
            modifiers: Modifiers {
                ctrl: false,
                alt: true,
                shift: false,
                super_key: true,
                cmd_or_ctrl: false,
            },
            key: Some(MatchKey::Character('A')),
            physical_key: Some(KeyCode::KeyA),
        };
        let combo_super_alt_a = parse_key_combo("Super+Alt+A").unwrap();
        assert!(matcher_both_ctrl_remapped.matches(&combo_super_alt_a));
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

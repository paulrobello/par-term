//! Keybinding system for par-term.
//!
//! This module provides runtime-configurable keybindings that allow users
//! to define custom keyboard shortcuts in their config.yaml.
//!
//! Features:
//! - Configurable key combinations (Ctrl+Shift+B, CmdOrCtrl+V, etc.)
//! - Modifier remapping (swap Ctrl and Super, etc.)
//! - Physical key support for language-agnostic bindings

#![warn(missing_docs)]

mod matcher;
pub mod parser;
pub mod platform;

pub use matcher::KeybindingMatcher;
pub use parser::KeyCombo;
// `ParseError` is consumed by `src/keybindings/mod.rs` in the root crate.
// The suppression is intentional: nothing inside par-term-keybindings itself
// imports this re-export, so the lint fires even though downstream consumers use it.
#[allow(unused_imports)]
pub use parser::ParseError;
pub use parser::{key_combo_to_bytes, parse_key_sequence};

use par_term_config::{KeyBinding, ModifierRemapping};
use std::collections::HashMap;

/// Sentinel action id that claims a chord for the terminal itself
/// (UX.md K2/K27 "unbind / pass to terminal").
///
/// A config row `{ key: "Alt+1", action: "pass_to_terminal" }` suppresses
/// every hardcoded interception of that chord — key layers, utility and tab
/// shortcuts, paste/copy — so the key is delivered to the shell. The row also
/// claims the chord for the §3.3 merge rule, so a default bound to the same
/// chord is not re-added on load (B20).
pub const PASS_TO_TERMINAL: &str = "pass_to_terminal";

fn is_removed_action(action: &str) -> bool {
    matches!(action, "toggle_prettifier")
}

/// Registry of keybindings mapping key combinations to action names.
#[derive(Debug, Default)]
pub struct KeybindingRegistry {
    /// Map of parsed key combos to action names
    bindings: HashMap<KeyCombo, String>,
    /// Config-order position of each registered combo, so the chord an
    /// action advertises is its first binding (the primary), not whichever
    /// alias sorts first.
    order: HashMap<KeyCombo, usize>,
}

impl KeybindingRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a registry from config keybindings.
    ///
    /// Invalid keybinding strings are logged and skipped.
    pub fn from_config(keybindings: &[KeyBinding]) -> Self {
        let mut registry = Self::new();

        log::info!(
            "Building keybinding registry from {} config keybindings",
            keybindings.len()
        );
        for binding in keybindings {
            if is_removed_action(&binding.action) {
                log::info!(
                    "Ignoring removed keybinding action '{}': {}",
                    binding.action,
                    binding.key
                );
                continue;
            }

            match parser::parse_key_combo(&binding.key) {
                Ok(combo) => {
                    // UX.md §3.3: the registry rejects duplicate chords at
                    // load. Chords are compared in platform-normalized form
                    // so two spellings of the same combination cannot
                    // silently coexist (B23); the first binding in config
                    // order wins and later claimants are skipped with a
                    // warning.
                    let combo = combo.platform_normalized();
                    if let Some(existing) = registry.bindings.get(&combo) {
                        if existing != &binding.action {
                            log::warn!(
                                "Duplicate chord '{}' for action '{}' (parsed as: {}): already \
                                 bound to '{}', keeping the first binding",
                                binding.key,
                                binding.action,
                                combo,
                                existing
                            );
                        }
                        continue;
                    }

                    log::info!(
                        "Registered keybinding: {} -> {} (parsed as: {:?})",
                        binding.key,
                        binding.action,
                        combo
                    );
                    let position = registry.bindings.len();
                    registry.order.insert(combo.clone(), position);
                    registry.bindings.insert(combo, binding.action.clone());
                }
                Err(e) => {
                    log::warn!(
                        "Invalid keybinding '{}' for action '{}': {}",
                        binding.key,
                        binding.action,
                        e
                    );
                }
            }
        }

        log::info!(
            "Keybinding registry initialized with {} bindings",
            registry.bindings.len()
        );
        registry
    }

    /// Look up an action for a key event.
    ///
    /// Returns the action name if a matching keybinding is found.
    pub fn lookup(
        &self,
        event: &winit::event::KeyEvent,
        modifiers: &winit::event::Modifiers,
    ) -> Option<&str> {
        self.lookup_with_options(event, modifiers, &ModifierRemapping::default(), false)
    }

    /// Look up an action for a key event with advanced options.
    ///
    /// # Arguments
    /// * `event` - The key event from winit
    /// * `modifiers` - Current modifier state
    /// * `remapping` - Modifier key remapping configuration
    /// * `use_physical_keys` - If true, match by physical key position (scan code) for
    ///   language-agnostic bindings. This makes keybindings consistent across keyboard layouts.
    ///
    /// Returns the action name if a matching keybinding is found.
    pub fn lookup_with_options(
        &self,
        event: &winit::event::KeyEvent,
        modifiers: &winit::event::Modifiers,
        remapping: &ModifierRemapping,
        use_physical_keys: bool,
    ) -> Option<&str> {
        let matcher = KeybindingMatcher::from_event_with_remapping(event, modifiers, remapping);

        for (combo, action) in &self.bindings {
            if matcher.matches_with_physical_preference(combo, use_physical_keys) {
                return Some(action.as_str());
            }
        }

        None
    }

    /// Look up an action for a synthetic key described by public fields.
    ///
    /// The injection seam for in-app UI testing: winit's `KeyEvent` has
    /// private fields and cannot be constructed outside winit, so chord
    /// injection enters the same matcher + registry loop through the key
    /// fields instead. With fields matching a real event, the result is
    /// identical to [`Self::lookup_with_options`].
    ///
    /// # Arguments
    /// * `logical_key` - The logical key (e.g. `Key::Character("p".into())`)
    /// * `physical_key` - The physical key code
    /// * `modifiers` - Current modifier state
    /// * `remapping` - Modifier key remapping configuration
    /// * `use_physical_keys` - If true, match by physical key position
    pub fn lookup_with_key_fields(
        &self,
        logical_key: &winit::keyboard::Key,
        physical_key: winit::keyboard::PhysicalKey,
        modifiers: &winit::event::Modifiers,
        remapping: &ModifierRemapping,
        use_physical_keys: bool,
    ) -> Option<&str> {
        let matcher = KeybindingMatcher::from_key_fields_with_remapping(
            logical_key,
            physical_key,
            modifiers,
            remapping,
        );

        for (combo, action) in &self.bindings {
            if matcher.matches_with_physical_preference(combo, use_physical_keys) {
                return Some(action.as_str());
            }
        }

        None
    }

    /// Check if the registry has any bindings.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Look up the action bound to a chord string, if any.
    ///
    /// Chord spellings that resolve to the same platform combination (for
    /// example `Cmd+D` and `CmdOrCtrl+D` on macOS) share one entry, matching
    /// the equivalence [`KeybindingRegistry::from_config`] enforces.
    pub fn find_by_chord(&self, key: &str) -> Option<&str> {
        parser::parse_key_combo(key)
            .ok()
            .map(parser::KeyCombo::platform_normalized)
            .and_then(|combo| self.bindings.get(&combo).map(String::as_str))
    }

    /// Look up the live chord bound to an action, if any (UX.md B22).
    ///
    /// The registry maps chord → action, so this is a reverse lookup over the
    /// whole map. An action bound to several chords (`next_tab` has both the
    /// bracket chord and `Ctrl+Tab`; `reopen_closed_tab` keeps `Cmd+Z` as an
    /// alias) yields the one registered first in config order — the primary,
    /// the same chord the menu displays — so the choice is deterministic
    /// despite the HashMap's random iteration order.
    pub fn chord_for_action(&self, action: &str) -> Option<parser::KeyCombo> {
        self.bindings
            .iter()
            .filter(|(_, bound)| bound.as_str() == action)
            .min_by_key(|(combo, _)| self.order.get(*combo).copied().unwrap_or(usize::MAX))
            .map(|(combo, _)| combo.clone())
    }

    /// Get the number of registered bindings.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_registry() {
        let registry = KeybindingRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_from_config() {
        let bindings = vec![
            KeyBinding {
                key: "Ctrl+Shift+B".to_string(),
                action: "toggle_background_shader".to_string(),
            },
            KeyBinding {
                key: "Ctrl+Shift+U".to_string(),
                action: "toggle_cursor_shader".to_string(),
            },
        ];

        let registry = KeybindingRegistry::from_config(&bindings);
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_removed_prettifier_action_skipped() {
        let bindings = vec![
            KeyBinding {
                key: "Ctrl+Shift+P".to_string(),
                action: "toggle_prettifier".to_string(),
            },
            KeyBinding {
                key: "Ctrl+Shift+B".to_string(),
                action: "toggle_background_shader".to_string(),
            },
        ];

        let registry = KeybindingRegistry::from_config(&bindings);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_invalid_keybinding_skipped() {
        let bindings = vec![
            KeyBinding {
                key: "InvalidKey".to_string(),
                action: "some_action".to_string(),
            },
            KeyBinding {
                key: "Ctrl+A".to_string(),
                action: "valid_action".to_string(),
            },
        ];

        let registry = KeybindingRegistry::from_config(&bindings);
        // Only valid bindings should be registered
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_duplicate_chord_first_binding_wins() {
        // Two spellings of the same chord must collapse to one entry, with
        // the first binding in config order winning (B23: HashMap iteration
        // order used to decide).
        let bindings = vec![
            KeyBinding {
                key: "Ctrl+D".to_string(),
                action: "first_action".to_string(),
            },
            KeyBinding {
                key: "Control+D".to_string(),
                action: "second_action".to_string(),
            },
        ];

        let registry = KeybindingRegistry::from_config(&bindings);
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.find_by_chord("CTRL+d"), Some("first_action"));
    }

    #[test]
    fn test_pass_to_terminal_row_registers_and_resolves() {
        // UX K2/K27: a `pass_to_terminal` row registers like any binding and
        // resolves on lookup, so dispatch can skip hardcoded layers for the
        // chord (Alt+1 case).
        let bindings = vec![KeyBinding {
            key: "Alt+1".to_string(),
            action: PASS_TO_TERMINAL.to_string(),
        }];

        let registry = KeybindingRegistry::from_config(&bindings);
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.find_by_chord("Alt+1"), Some(PASS_TO_TERMINAL));
    }

    #[test]
    fn test_duplicate_chord_same_action_deduplicated() {
        let bindings = vec![
            KeyBinding {
                key: "Ctrl+D".to_string(),
                action: "same_action".to_string(),
            },
            KeyBinding {
                key: "Control+D".to_string(),
                action: "same_action".to_string(),
            },
        ];

        let registry = KeybindingRegistry::from_config(&bindings);
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.find_by_chord("Ctrl+D"), Some("same_action"));
    }

    #[test]
    fn test_cmd_or_ctrl_spellings_share_one_entry_on_macos() {
        let bindings = vec![
            KeyBinding {
                key: "Cmd+D".to_string(),
                action: "user_binding".to_string(),
            },
            KeyBinding {
                key: "CmdOrCtrl+D".to_string(),
                action: "default_binding".to_string(),
            },
        ];

        let registry = KeybindingRegistry::from_config(&bindings);
        if cfg!(target_os = "macos") {
            // CmdOrCtrl resolves to Cmd on macOS: same chord, first wins.
            assert_eq!(registry.len(), 1);
            assert_eq!(registry.find_by_chord("Command+D"), Some("user_binding"));
        } else {
            // Distinct modifiers: both entries coexist.
            assert_eq!(registry.len(), 2);
            assert_eq!(registry.find_by_chord("Cmd+D"), Some("user_binding"));
            assert_eq!(registry.find_by_chord("Ctrl+D"), Some("default_binding"));
        }
    }

    #[test]
    fn test_chord_for_action_returns_the_live_binding() {
        // B22: the reverse lookup answers "what chord fires this action
        // right now", so a rebind (toggle_fullscreen moved to F9) is visible
        // to chord-advertising surfaces.
        let bindings = vec![
            KeyBinding {
                key: "F9".to_string(),
                action: "toggle_fullscreen".to_string(),
            },
            KeyBinding {
                key: "Ctrl+D".to_string(),
                action: "split_down".to_string(),
            },
        ];

        let registry = KeybindingRegistry::from_config(&bindings);
        assert_eq!(
            registry
                .chord_for_action("toggle_fullscreen")
                .map(|combo| combo.to_string()),
            Some("F9".to_string())
        );
        assert_eq!(
            registry
                .chord_for_action("split_down")
                .map(|combo| combo.to_string()),
            Some("Ctrl+D".to_string())
        );
        assert!(registry.chord_for_action("new_tab").is_none());
    }

    #[test]
    fn test_chord_for_action_is_deterministic_across_dual_chords() {
        // next_tab ships with two chords; the smallest normalized spelling
        // wins every call despite the HashMap's random iteration order.
        let bindings = vec![
            KeyBinding {
                key: "CmdOrCtrl+Shift+]".to_string(),
                action: "next_tab".to_string(),
            },
            KeyBinding {
                key: "Ctrl+Tab".to_string(),
                action: "next_tab".to_string(),
            },
        ];

        let registry = KeybindingRegistry::from_config(&bindings);
        let expected = registry.chord_for_action("next_tab").expect("bound");
        for _ in 0..32 {
            assert_eq!(
                registry.chord_for_action("next_tab"),
                Some(expected.clone())
            );
        }
    }
}

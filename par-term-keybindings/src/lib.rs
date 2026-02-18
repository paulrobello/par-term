//! Keybinding system for par-term.
//!
//! This module provides runtime-configurable keybindings that allow users
//! to define custom keyboard shortcuts in their config.yaml.
//!
//! Features:
//! - Configurable key combinations (Ctrl+Shift+B, CmdOrCtrl+V, etc.)
//! - Modifier remapping (swap Ctrl and Super, etc.)
//! - Physical key support for language-agnostic bindings

mod matcher;
pub mod parser;

pub use matcher::KeybindingMatcher;
pub use parser::KeyCombo;
// ParseError exported for consumers who might want to handle parsing errors
#[allow(unused_imports)]
pub use parser::ParseError;
pub use parser::{key_combo_to_bytes, parse_key_sequence};

use par_term_config::{KeyBinding, ModifierRemapping};
use std::collections::HashMap;

/// Registry of keybindings mapping key combinations to action names.
#[derive(Debug, Default)]
pub struct KeybindingRegistry {
    /// Map of parsed key combos to action names
    bindings: HashMap<KeyCombo, String>,
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
            match parser::parse_key_combo(&binding.key) {
                Ok(combo) => {
                    log::info!(
                        "Registered keybinding: {} -> {} (parsed as: {:?})",
                        binding.key,
                        binding.action,
                        combo
                    );
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

    /// Check if the registry has any bindings.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Get the number of registered bindings.
    #[allow(dead_code)]
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
}

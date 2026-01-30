//! Keybinding system for par-term.
//!
//! This module provides runtime-configurable keybindings that allow users
//! to define custom keyboard shortcuts in their config.yaml.

mod matcher;
mod parser;

pub use matcher::KeybindingMatcher;
pub use parser::KeyCombo;
// ParseError exported for consumers who might want to handle parsing errors
#[allow(unused_imports)]
pub use parser::ParseError;

use crate::config::KeyBinding;
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

        for binding in keybindings {
            match parser::parse_key_combo(&binding.key) {
                Ok(combo) => {
                    log::debug!(
                        "Registered keybinding: {} -> {}",
                        binding.key,
                        binding.action
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
        let matcher = KeybindingMatcher::from_event(event, modifiers);

        for (combo, action) in &self.bindings {
            if matcher.matches(combo) {
                log::debug!("Keybinding matched: {} -> {}", combo, action);
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

//! Profile keyboard shortcuts: per-profile hotkeys and shortcut string building.

use crate::app::window_state::WindowState;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

impl WindowState {
    /// Handle profile keyboard shortcuts (per-profile hotkeys defined in profiles.yaml)
    pub(crate) fn handle_profile_shortcuts(&mut self, event: &KeyEvent) -> bool {
        if event.state != ElementState::Pressed {
            return false;
        }

        // Build shortcut string from current key event
        let shortcut = self.build_shortcut_string(event);
        if shortcut.is_empty() {
            return false;
        }

        // Look up profile by shortcut
        if let Some(profile) = self.overlay_ui.profile_manager.find_by_shortcut(&shortcut) {
            let profile_id = profile.id;
            let profile_name = profile.name.clone();

            // Open the profile (creates a new tab)
            self.open_profile(profile_id);
            log::info!(
                "Opened profile '{}' via shortcut '{}'",
                profile_name,
                shortcut
            );

            if let Some(window) = &self.window {
                window.request_redraw();
            }

            return true;
        }

        false
    }

    /// Build a shortcut string from a key event (e.g., "Cmd+1", "Ctrl+Shift+2")
    pub(crate) fn build_shortcut_string(&self, event: &KeyEvent) -> String {
        let modifiers = self.input_handler.modifiers.state();
        let mut parts = Vec::new();

        // Add modifier keys (in canonical order)
        #[cfg(target_os = "macos")]
        {
            if modifiers.super_key() {
                parts.push("Cmd");
            }
            if modifiers.control_key() {
                parts.push("Ctrl");
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            if modifiers.control_key() {
                parts.push("Ctrl");
            }
        }

        if modifiers.alt_key() {
            parts.push("Alt");
        }
        if modifiers.shift_key() {
            parts.push("Shift");
        }

        // Add the key itself
        let key_name = match &event.logical_key {
            Key::Character(c) => {
                let s = c.to_string();
                if s.len() == 1 {
                    Some(s.to_uppercase())
                } else {
                    None
                }
            }
            Key::Named(named) => {
                // Convert named keys to string representation
                match named {
                    NamedKey::F1 => Some("F1".to_string()),
                    NamedKey::F2 => Some("F2".to_string()),
                    NamedKey::F3 => Some("F3".to_string()),
                    NamedKey::F4 => Some("F4".to_string()),
                    NamedKey::F5 => Some("F5".to_string()),
                    NamedKey::F6 => Some("F6".to_string()),
                    NamedKey::F7 => Some("F7".to_string()),
                    NamedKey::F8 => Some("F8".to_string()),
                    NamedKey::F9 => Some("F9".to_string()),
                    NamedKey::F10 => Some("F10".to_string()),
                    NamedKey::F11 => Some("F11".to_string()),
                    NamedKey::F12 => Some("F12".to_string()),
                    _ => None,
                }
            }
            _ => None,
        };

        if let Some(key) = key_name {
            parts.push(key.leak()); // Safe for short-lived strings in this context
            parts.join("+")
        } else {
            String::new()
        }
    }
}

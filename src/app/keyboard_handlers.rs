//! Keyboard handler operations for WindowState.
//!
//! This module contains methods for handling keyboard shortcuts
//! like fullscreen toggle, settings toggle, help toggle, etc.

use super::window_state::WindowState;
use winit::event::KeyEvent;

impl WindowState {
    pub(crate) fn handle_fullscreen_toggle(&mut self, event: &KeyEvent) -> bool {
        use winit::event::ElementState;
        use winit::keyboard::{Key, NamedKey};

        if event.state != ElementState::Pressed {
            return false;
        }

        // F11: Toggle fullscreen
        if matches!(event.logical_key, Key::Named(NamedKey::F11))
            && let Some(window) = &self.window
        {
            self.is_fullscreen = !self.is_fullscreen;

            if self.is_fullscreen {
                window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                log::info!("Entering fullscreen mode");
            } else {
                window.set_fullscreen(None);
                log::info!("Exiting fullscreen mode");
            }

            return true;
        }

        false
    }

    pub(crate) fn handle_settings_toggle(&mut self, event: &KeyEvent) -> bool {
        use winit::event::ElementState;
        use winit::keyboard::{Key, NamedKey};

        if event.state != ElementState::Pressed {
            return false;
        }

        // F12: Toggle settings UI
        if matches!(event.logical_key, Key::Named(NamedKey::F12)) {
            self.settings_ui.toggle();
            log::info!(
                "Settings UI toggled: {}",
                if self.settings_ui.visible {
                    "visible"
                } else {
                    "hidden"
                }
            );

            // Request redraw to show/hide settings
            if let Some(window) = &self.window {
                window.request_redraw();
            }

            return true;
        }

        false
    }

    /// Handle F1 key to toggle help panel
    pub(crate) fn handle_help_toggle(&mut self, event: &KeyEvent) -> bool {
        use winit::event::ElementState;
        use winit::keyboard::{Key, NamedKey};

        if event.state != ElementState::Pressed {
            return false;
        }

        // F1: Toggle help UI
        if matches!(event.logical_key, Key::Named(NamedKey::F1)) {
            self.help_ui.toggle();
            log::info!(
                "Help UI toggled: {}",
                if self.help_ui.visible {
                    "visible"
                } else {
                    "hidden"
                }
            );

            // Request redraw to show/hide help
            if let Some(window) = &self.window {
                window.request_redraw();
            }

            return true;
        }

        // Escape: Close help UI if visible
        if matches!(event.logical_key, Key::Named(NamedKey::Escape)) && self.help_ui.visible {
            self.help_ui.visible = false;
            log::info!("Help UI closed via Escape");

            if let Some(window) = &self.window {
                window.request_redraw();
            }

            return true;
        }

        false
    }

    /// Handle F11 key to toggle shader editor
    pub(crate) fn handle_shader_editor_toggle(&mut self, event: &KeyEvent) -> bool {
        use winit::event::ElementState;
        use winit::keyboard::{Key, NamedKey};

        if event.state != ElementState::Pressed {
            return false;
        }

        // F11: Toggle shader editor
        if matches!(event.logical_key, Key::Named(NamedKey::F11)) {
            if self.settings_ui.is_shader_editor_visible() {
                // Close shader editor - handled by the UI itself
                log::info!("Shader editor close requested via F11");
            } else {
                // Open shader editor
                if self.settings_ui.open_shader_editor() {
                    log::info!("Shader editor opened via F11");
                } else {
                    log::warn!("Cannot open shader editor: no shader path configured in settings");
                }
            }

            // Request redraw to show/hide shader editor
            if let Some(window) = &self.window {
                window.request_redraw();
            }

            return true;
        }

        false
    }

    /// Handle F3 key to toggle FPS overlay
    pub(crate) fn handle_fps_overlay_toggle(&mut self, event: &KeyEvent) -> bool {
        use winit::event::ElementState;
        use winit::keyboard::{Key, NamedKey};

        if event.state != ElementState::Pressed {
            return false;
        }

        // F3: Toggle FPS overlay
        if matches!(event.logical_key, Key::Named(NamedKey::F3)) {
            self.debug.show_fps_overlay = !self.debug.show_fps_overlay;
            log::info!(
                "FPS overlay toggled: {}",
                if self.debug.show_fps_overlay {
                    "visible"
                } else {
                    "hidden"
                }
            );

            // Request redraw to show/hide FPS overlay
            if let Some(window) = &self.window {
                window.request_redraw();
            }

            return true;
        }

        false
    }
}

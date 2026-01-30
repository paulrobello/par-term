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

        // F12 or Cmd+, (macOS): Open settings window
        let is_f12 = matches!(event.logical_key, Key::Named(NamedKey::F12));
        let is_cmd_comma = matches!(event.logical_key, Key::Character(ref c) if c == ",")
            && self.input_handler.modifiers.state().super_key();

        if is_f12 || is_cmd_comma {
            // Signal to window manager to open settings window
            self.open_settings_window_requested = true;
            log::info!("Settings window requested");

            // Request redraw to trigger event processing
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

    /// Handle F11 key - shader editor toggle is now handled via standalone settings window
    /// This function is kept for backwards compatibility but no longer does anything
    pub(crate) fn handle_shader_editor_toggle(&mut self, _event: &KeyEvent) -> bool {
        // Shader editor is now accessed through the standalone settings window
        // F11 may be used for fullscreen via keybindings
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

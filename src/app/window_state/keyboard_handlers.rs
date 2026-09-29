//! Keyboard handler operations for WindowState.
//!
//! The chord shortcuts that used to live here (fullscreen, settings, shader
//! editor, FPS overlay, profile drawer) resolve through the registry as
//! default keybindings (`defaults::layer_chords`, UX K2). What remains is the
//! help handler's state-conditional Escape branches.

use super::WindowState;
use winit::event::KeyEvent;

impl WindowState {
    /// Handle Escape while the help panel, shader install UI, or integrations
    /// dialog is visible. Opening the help panel resolves through the
    /// registry's `toggle_help` default (UX K2).
    pub(crate) fn handle_help_toggle(&mut self, event: &KeyEvent) -> bool {
        use winit::event::ElementState;
        use winit::keyboard::{Key, NamedKey};

        if event.state != ElementState::Pressed {
            return false;
        }

        // Escape: Close help UI if visible
        if matches!(event.logical_key, Key::Named(NamedKey::Escape))
            && self.overlay_ui.help_ui.visible
        {
            self.overlay_ui.help_ui.visible = false;
            log::info!("Help UI closed via Escape");

            self.request_redraw();

            return true;
        }

        // Escape: Close shader install UI if visible (only when not installing)
        if matches!(event.logical_key, Key::Named(NamedKey::Escape))
            && self.overlay_ui.shader_install_ui.visible
            && !self.overlay_ui.shader_install_ui.installing
        {
            self.overlay_ui.shader_install_ui.visible = false;
            log::info!("Shader install UI closed via Escape");

            self.request_redraw();

            return true;
        }

        // Escape: Close integrations welcome dialog if visible (only when not installing)
        if matches!(event.logical_key, Key::Named(NamedKey::Escape))
            && self.overlay_ui.integrations_ui.visible
            && !self.overlay_ui.integrations_ui.installing
        {
            self.overlay_ui.integrations_ui.visible = false;
            log::info!("Integrations dialog closed via Escape");

            self.request_redraw();

            return true;
        }

        false
    }
}

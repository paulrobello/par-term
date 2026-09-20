//! Command palette key handling.
//!
//! The palette has no chord of its own — it is opened by the
//! `toggle_command_palette` action — so unlike `search.rs` this layer never
//! opens anything. It exists only to own Escape while the palette is on
//! screen, so dismissing it cannot also send an escape byte to the PTY.

use crate::app::window_state::WindowState;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

impl WindowState {
    pub(crate) fn handle_command_palette_keys(&mut self, event: &KeyEvent) -> bool {
        if !self.overlay_ui.command_palette.visible {
            return false;
        }

        if event.state == ElementState::Pressed
            && let Key::Named(NamedKey::Escape) = &event.logical_key
        {
            self.overlay_ui.command_palette.close();
            self.focus_state.needs_redraw = true;
            return true;
        }

        // Every other key belongs to the palette's text field and list
        // navigation, which egui handles. Returning false lets the event
        // propagate to the UI, exactly as the search layer does.
        false
    }
}

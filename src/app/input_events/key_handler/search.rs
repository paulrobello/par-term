//! Search UI key handling while the search bar is visible.
//!
//! Opening (Cmd+F / Ctrl+Shift+F) resolves through the registry's
//! `toggle_search` default (UX K2); this layer only owns the state machine
//! while the UI is open.

use crate::app::window_state::WindowState;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

impl WindowState {
    pub(crate) fn handle_search_keys(&mut self, event: &KeyEvent) -> bool {
        // Handle keys when search UI is visible
        if self.overlay_ui.search_ui.visible {
            if event.state == ElementState::Pressed
                && let Key::Named(NamedKey::Escape) = &event.logical_key
            {
                self.overlay_ui.search_ui.close();
                self.focus_state.needs_redraw = true;
                return true;
            }
            // While search is visible, let egui handle most keys
            // Return false to let the event propagate to the UI
            return false;
        }

        false
    }
}

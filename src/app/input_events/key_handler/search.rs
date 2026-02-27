//! Search UI key handling (Cmd/Ctrl+F).

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
                self.needs_redraw = true;
                return true;
            }
            // While search is visible, let egui handle most keys
            // Return false to let the event propagate to the UI
            return false;
        }

        // macOS: Cmd+F / Windows/Linux: Ctrl+Shift+F
        // (Ctrl+F is "forward character" in readline, must not be intercepted on non-macOS)
        if event.state == ElementState::Pressed {
            let shift = self.input_handler.modifiers.state().shift_key();

            #[cfg(target_os = "macos")]
            let is_search = {
                let cmd = self.input_handler.modifiers.state().super_key();
                cmd && !shift
                    && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("f"))
            };
            #[cfg(not(target_os = "macos"))]
            let is_search = {
                let ctrl = self.input_handler.modifiers.state().control_key();
                ctrl && shift
                    && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("f"))
            };

            if is_search {
                self.overlay_ui.search_ui.open();
                // Initialize from config
                self.overlay_ui
                    .search_ui
                    .init_from_config(self.config.search_case_sensitive, self.config.search_regex);
                self.needs_redraw = true;
                log::debug!("Search UI opened");
                return true;
            }
        }

        false
    }
}

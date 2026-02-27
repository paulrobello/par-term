//! Scroll navigation key handling (PageUp/PageDown, Home/End, mark navigation).

use crate::app::window_state::WindowState;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

impl WindowState {
    pub(crate) fn handle_scroll_keys(&mut self, event: &KeyEvent) -> bool {
        if event.state != ElementState::Pressed {
            return false;
        }

        let modifiers = self.input_handler.modifiers.state();
        let shift = modifiers.shift_key();
        let super_key = modifiers.super_key();

        let handled = match &event.logical_key {
            Key::Named(NamedKey::ArrowUp) if super_key => {
                self.scroll_to_previous_mark();
                true
            }
            Key::Named(NamedKey::ArrowDown) if super_key => {
                self.scroll_to_next_mark();
                true
            }
            Key::Named(NamedKey::PageUp) => {
                // Scroll up one page
                self.scroll_up_page();
                true
            }
            Key::Named(NamedKey::PageDown) => {
                // Scroll down one page
                self.scroll_down_page();
                true
            }
            Key::Named(NamedKey::Home) if shift => {
                // Shift+Home: Scroll to top
                self.scroll_to_top();
                true
            }
            Key::Named(NamedKey::End) if shift => {
                // Shift+End: Scroll to bottom
                self.scroll_to_bottom();
                true
            }
            _ => false,
        };

        if handled && let Some(window) = &self.window {
            window.request_redraw();
        }

        handled
    }
}

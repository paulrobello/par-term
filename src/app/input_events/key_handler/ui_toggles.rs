//! AI inspector (Assistant panel) toggle key handling.

use crate::app::window_state::WindowState;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::Key;

impl WindowState {
    pub(crate) fn handle_ai_inspector_toggle(&mut self, event: &KeyEvent) -> bool {
        if event.state != ElementState::Pressed {
            return false;
        }

        if !self.config.ai_inspector_enabled {
            return false;
        }

        let shift = self.input_handler.modifiers.state().shift_key();

        // Assistant panel toggle: Cmd+I (macOS) / Ctrl+Shift+I (other)
        #[cfg(target_os = "macos")]
        let is_inspector = {
            let cmd = self.input_handler.modifiers.state().super_key();
            cmd && !shift
                && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("i"))
        };
        #[cfg(not(target_os = "macos"))]
        let is_inspector = {
            let ctrl = self.input_handler.modifiers.state().control_key();
            ctrl && shift
                && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("i"))
        };

        if is_inspector {
            let just_opened = self.overlay_ui.ai_inspector.toggle();
            self.sync_ai_inspector_width();
            if just_opened {
                self.try_auto_connect_agent();
            }
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            log::debug!(
                "Assistant panel toggled: {}",
                self.overlay_ui.ai_inspector.open
            );
            return true;
        }

        false
    }
}

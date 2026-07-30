//! AI inspector (Assistant panel) toggle key handling.

use super::claims;
use crate::app::window_state::WindowState;
use winit::event::{ElementState, KeyEvent};

impl WindowState {
    pub(crate) fn handle_ai_inspector_toggle(&mut self, event: &KeyEvent) -> bool {
        if event.state != ElementState::Pressed {
            return false;
        }

        if !self.config.load().ai_inspector.ai_inspector_enabled {
            return false;
        }

        let mods = self.input_handler.modifiers.state();

        // Assistant panel toggle: Cmd+I (macOS) / Ctrl+Shift+I (other).
        // Driven by the layer's declared claim so the declaration cannot drift
        // from what actually dispatches.
        let is_inspector = claims::AI_INSPECTOR[0].matches_event(&mods, &event.logical_key);

        if is_inspector {
            let just_opened = self.overlay_ui.ai_inspector.toggle();
            self.sync_ai_inspector_width();
            if just_opened {
                if self
                    .config
                    .load()
                    .ai_inspector
                    .ai_inspector_input_history_mode
                    == par_term_config::AssistantInputHistoryMode::Persist
                {
                    self.overlay_ui.ai_inspector.merge_persisted_input_history();
                }
                self.try_auto_connect_agent();
            }
            self.request_redraw();
            log::debug!(
                "Assistant panel toggled: {}",
                self.overlay_ui.ai_inspector.open
            );
            return true;
        }

        false
    }
}

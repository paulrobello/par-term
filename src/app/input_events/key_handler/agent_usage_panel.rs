//! Agent-usage panel key handling.
//!
//! The panel has no chord of its own — it opens from the status-bar widget
//! click or the `toggle_agent_usage_panel` action — so this layer never opens
//! anything. While the panel is visible it owns Escape (the egui-side close
//! in `panel.rs::show` handles the focused case; this is the backstop, and
//! `close()` is idempotent so both paths coexist) and `h`/`l` (agent
//! switching — claimed here rather than consumed in egui so the keys cannot
//! reach the PTY even when egui holds no focus). `r` stays egui-side: the
//! panel has no text field, so unfocused keys still propagate to egui, where
//! `show()` consumes it as the refresh trigger.

use crate::app::window_state::WindowState;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

impl WindowState {
    pub(crate) fn handle_agent_usage_panel_keys(&mut self, event: &KeyEvent) -> bool {
        if !self.overlay_ui.agent_usage_panel.visible {
            return false;
        }

        if event.state == ElementState::Pressed {
            match &event.logical_key {
                Key::Named(NamedKey::Escape) => {
                    self.overlay_ui.agent_usage_panel.close();
                    self.focus_state.needs_redraw = true;
                    return true;
                }
                Key::Character(ch) if ch.as_str() == "l" => {
                    let count = self.status_bar_ui.usage_snapshot().records.len();
                    self.overlay_ui.agent_usage_panel.cycle_agent(true, count);
                    self.focus_state.needs_redraw = true;
                    return true;
                }
                Key::Character(ch) if ch.as_str() == "h" => {
                    let count = self.status_bar_ui.usage_snapshot().records.len();
                    self.overlay_ui.agent_usage_panel.cycle_agent(false, count);
                    self.focus_state.needs_redraw = true;
                    return true;
                }
                _ => {}
            }
        }

        // Everything else propagates: the panel is display-only apart from
        // the keys above, and egui-side handling (`r`, click) runs on the
        // egui input path regardless of widget focus.
        false
    }
}

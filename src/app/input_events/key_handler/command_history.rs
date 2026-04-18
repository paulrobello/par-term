//! Command history key handling (UI navigation) and toggle.
//!
//! The toggle shortcut is driven entirely by the configured keybinding
//! (`toggle_command_history` action, default `CmdOrCtrl+R`) via the registry's
//! strict modifier matcher. Only in-UI navigation (Escape/Arrows/Enter) is
//! handled here.

use crate::app::window_state::WindowState;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

impl WindowState {
    pub(crate) fn handle_command_history_keys(&mut self, event: &KeyEvent) -> bool {
        // Handle keys when command history UI is visible
        if self.overlay_ui.command_history_ui.visible {
            if event.state == ElementState::Pressed {
                match &event.logical_key {
                    Key::Named(NamedKey::Escape) => {
                        self.overlay_ui.command_history_ui.close();
                        self.focus_state.needs_redraw = true;
                        return true;
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        self.overlay_ui.command_history_ui.select_previous();
                        self.focus_state.needs_redraw = true;
                        return true;
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        self.overlay_ui
                            .command_history_ui
                            .select_next(self.overlay_ui.command_history.len());
                        self.focus_state.needs_redraw = true;
                        return true;
                    }
                    Key::Named(NamedKey::Enter) => {
                        // Insert the selected command into the terminal
                        if let Some(command) = self.overlay_ui.command_history_ui.selected_command()
                        {
                            self.overlay_ui.command_history_ui.close();
                            self.paste_text(&command);
                        }
                        self.focus_state.needs_redraw = true;
                        return true;
                    }
                    _ => {}
                }
            }
            // While command history is visible, consume all key events
            return true;
        }

        false
    }

    pub(crate) fn toggle_command_history(&mut self) {
        // Refresh entries from persistent history before showing
        self.overlay_ui
            .command_history_ui
            .update_entries(self.overlay_ui.command_history.entries());
        self.overlay_ui.command_history_ui.toggle();
        self.focus_state.needs_redraw = true;
        log::debug!(
            "Command history UI toggled: {}",
            self.overlay_ui.command_history_ui.visible
        );
    }
}

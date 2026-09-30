//! The command history toggle.
//!
//! The toggle shortcut is driven entirely by the configured keybinding
//! (`toggle_command_history` action, default `CmdOrCtrl+R`) via the registry's
//! strict modifier matcher. While the panel is open, its keys (Escape,
//! arrows, Enter) are read by `CommandHistoryUI::show()` on the egui side,
//! behind the overlay stack (UX.md OV2).

use crate::app::window_state::WindowState;

impl WindowState {
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

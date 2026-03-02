//! Tab bar action handlers.
//!
//! Contains [`WindowState::handle_tab_bar_action_after_render`], dispatching
//! all [`TabBarAction`] variants produced during egui rendering.

use crate::app::window_state::WindowState;
use crate::tab_bar_ui::TabBarAction;

impl WindowState {
    /// Handle tab bar actions collected during egui rendering (called after renderer borrow released).
    pub(crate) fn handle_tab_bar_action_after_render(
        &mut self,
        action: crate::tab_bar_ui::TabBarAction,
    ) {
        // Handle tab bar actions collected during egui rendering
        // (done here to avoid borrow conflicts with renderer)
        match action {
            TabBarAction::SwitchTo(id) => {
                self.tab_manager.switch_to(id);
                // Clear renderer cells and invalidate cache to ensure clean switch
                self.clear_and_invalidate();
            }
            TabBarAction::Close(id) => {
                // Switch to the tab first so close_current_tab() operates on it.
                // This routes through the full close path: running-jobs confirmation,
                // session undo capture, and preserve-shell logic.
                self.tab_manager.switch_to(id);
                let was_last = self.close_current_tab();
                if was_last {
                    self.is_shutting_down = true;
                }
                self.request_redraw();
            }
            TabBarAction::NewTab => {
                self.new_tab();
                self.request_redraw();
            }
            TabBarAction::SetColor(id, color) => {
                if let Some(tab) = self.tab_manager.get_tab_mut(id) {
                    tab.set_custom_color(color);
                    log::info!(
                        "Set custom color for tab {}: RGB({}, {}, {})",
                        id,
                        color[0],
                        color[1],
                        color[2]
                    );
                }
                self.request_redraw();
            }
            TabBarAction::ClearColor(id) => {
                if let Some(tab) = self.tab_manager.get_tab_mut(id) {
                    tab.clear_custom_color();
                    log::info!("Cleared custom color for tab {}", id);
                }
                self.request_redraw();
            }
            TabBarAction::Reorder(id, target_index) => {
                if self.tab_manager.move_tab_to_index(id, target_index) {
                    self.focus_state.needs_redraw = true;
                    self.request_redraw();
                }
            }
            TabBarAction::NewTabWithProfile(profile_id) => {
                self.open_profile(profile_id);
                self.request_redraw();
            }
            TabBarAction::RenameTab(id, name) => {
                if let Some(tab) = self.tab_manager.get_tab_mut(id) {
                    if name.is_empty() {
                        // Blank name: revert to auto title mode
                        tab.user_named = false;
                        tab.has_default_title = true;
                        // Trigger immediate title update
                        tab.update_title(self.config.tab_title_mode);
                    } else {
                        tab.title = name;
                        tab.user_named = true;
                        tab.has_default_title = false;
                    }
                }
                self.request_redraw();
            }
            TabBarAction::Duplicate(id) => {
                self.duplicate_tab_by_id(id);
                self.request_redraw();
            }
            TabBarAction::ToggleAssistantPanel => {
                let just_opened = self.overlay_ui.ai_inspector.toggle();
                self.sync_ai_inspector_width();
                if just_opened {
                    self.try_auto_connect_agent();
                }
                self.request_redraw();
            }
            TabBarAction::SetTabIcon(tab_id, icon) => {
                if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
                    tab.custom_icon = icon;
                }
                self.request_redraw();
            }
            TabBarAction::None => {}
        }
    }
}

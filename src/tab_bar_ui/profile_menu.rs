//! Profile selection menu for creating new tabs from profiles.
//!
//! Contains the [`TabBarUI`] method for rendering the floating "New Tab" profile
//! picker popup that appears when the chevron (▾) button is clicked.

use crate::config::Config;
use crate::ui_constants::{
    TAB_NEW_PROFILE_MENU_OFFSET_X, TAB_NEW_PROFILE_MENU_OFFSET_Y, TAB_NEW_PROFILE_MENU_WIDTH,
};

use super::TabBarAction;
use super::TabBarUI;

impl TabBarUI {
    /// Render the new-tab profile selection popup.
    pub(super) fn render_new_tab_profile_menu(
        &mut self,
        ctx: &egui::Context,
        profiles: &crate::profile::ProfileManager,
        config: &Config,
    ) -> TabBarAction {
        let mut action = TabBarAction::None;

        if !self.show_new_tab_profile_menu {
            return action;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_new_tab_profile_menu = false;
            return action;
        }

        let mut open = true;
        egui::Window::new("New Tab")
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .fixed_size(egui::vec2(TAB_NEW_PROFILE_MENU_WIDTH, 0.0))
            .anchor(
                egui::Align2::RIGHT_TOP,
                egui::vec2(TAB_NEW_PROFILE_MENU_OFFSET_X, TAB_NEW_PROFILE_MENU_OFFSET_Y),
            )
            .open(&mut open)
            .show(ctx, |ui| {
                // "Default" entry — always first
                if ui
                    .selectable_label(false, "  Default")
                    .on_hover_text("Open a new tab with default settings")
                    .clicked()
                {
                    action = TabBarAction::NewTab;
                    self.show_new_tab_profile_menu = false;
                }
                ui.separator();

                // Profile entries in display order
                for profile in profiles.profiles_ordered() {
                    let icon = profile.icon.as_deref().unwrap_or("  ");
                    let label = format!("{} {}", icon, profile.name);
                    if ui.selectable_label(false, &label).clicked() {
                        action = TabBarAction::NewTabWithProfile(profile.id);
                        self.show_new_tab_profile_menu = false;
                    }
                }

                // Assistant panel toggle (only when enabled in config)
                if config.ai_inspector.ai_inspector_enabled {
                    ui.separator();
                    if ui
                        .selectable_label(false, "  Assistant Panel")
                        .on_hover_text("Toggle the AI assistant panel")
                        .clicked()
                    {
                        action = TabBarAction::ToggleAssistantPanel;
                        self.show_new_tab_profile_menu = false;
                    }
                }
            });

        if !open {
            self.show_new_tab_profile_menu = false;
        }

        action
    }
}

//! Settings section layout: sidebar navigation, tab content dispatch, keybinding check.
//!
//! Contains: show_settings_sections(), show_tab_content(), check_keybinding_conflict().

use crate::sidebar::SettingsTab;

use super::SettingsUI;

impl SettingsUI {
    /// Show all settings sections using the sidebar + tab layout.
    pub(super) fn show_settings_sections(
        &mut self,
        ui: &mut egui::Ui,
        changes_this_frame: &mut bool,
    ) {
        crate::quick_settings::show(ui, self, changes_this_frame);
        ui.separator();

        let available_width = ui.available_width();
        // Reserve space for the footer (separator + button row)
        let footer_height = 45.0;
        let available_height = (ui.available_height() - footer_height).max(100.0);
        let sidebar_width = 150.0;
        let content_width = (available_width - sidebar_width - 15.0).max(300.0);

        let layout = egui::Layout::left_to_right(egui::Align::Min);
        ui.allocate_ui_with_layout(
            egui::vec2(available_width, available_height),
            layout,
            |ui| {
                // Sidebar with its own scroll area
                ui.allocate_ui_with_layout(
                    egui::vec2(sidebar_width, available_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("settings_sidebar")
                            .max_height(available_height)
                            .show(ui, |ui| {
                                crate::sidebar::show(
                                    ui,
                                    &mut self.selected_tab,
                                    &self.search_query,
                                );
                            });
                    },
                );

                ui.separator();

                // Content area with its own scroll area
                ui.allocate_ui_with_layout(
                    egui::vec2(content_width, available_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("settings_tab_content")
                            .max_height(available_height)
                            .show(ui, |ui| {
                                ui.set_min_width(content_width - 20.0);
                                self.show_tab_content(ui, changes_this_frame);
                            });
                    },
                );
            },
        );
    }

    /// Show the content for the currently selected tab.
    pub(super) fn show_tab_content(&mut self, ui: &mut egui::Ui, changes_this_frame: &mut bool) {
        let mut collapsed = std::mem::take(&mut self.collapsed_sections);

        match self.selected_tab {
            SettingsTab::Appearance => {
                crate::appearance_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Window => {
                crate::window_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Input => {
                crate::input_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Terminal => {
                crate::terminal_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Effects => {
                crate::effects_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Badge => {
                crate::badge_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::ProgressBar => {
                crate::progress_bar_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::StatusBar => {
                crate::status_bar_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Profiles => {
                crate::profiles_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Ssh => {
                self.show_ssh_tab(ui, changes_this_frame);
            }
            SettingsTab::Notifications => {
                crate::notifications_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Integrations => {
                self.show_integrations_tab(ui, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Automation => {
                crate::automation_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Scripts => {
                crate::scripts_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Snippets => {
                crate::snippets_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Actions => {
                crate::actions_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::ContentPrettifier => {
                crate::prettifier_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Arrangements => {
                crate::arrangements_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::AiInspector => {
                crate::ai_inspector_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Advanced => {
                crate::advanced_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
        }

        self.collapsed_sections = collapsed;
    }

    /// Check if a keybinding conflicts with existing keybindings.
    pub fn check_keybinding_conflict(&self, key: &str, exclude_id: Option<&str>) -> Option<String> {
        for binding in &self.config.keybindings {
            if binding.key == key {
                return Some(format!("Already bound to: {}", binding.action));
            }
        }

        for snippet in &self.config.snippets {
            if let Some(snippet_key) = &snippet.keybinding
                && snippet_key == key
            {
                if exclude_id == Some(&snippet.id) {
                    continue;
                }
                return Some(format!("Already bound to snippet: {}", snippet.title));
            }
        }

        None
    }
}

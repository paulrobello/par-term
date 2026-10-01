//! Profile edit/create view for `ProfileModalUI`.
//!
//! The header, sub-tab bar, validation message, and Done/Cancel are drawn
//! here and stay visible on every sub-tab; the fields of each sub-tab are
//! in `edit_tabs.rs`.

use super::{ModalMode, ProfileModalUI};
use crate::nerd_font::NERD_FONT_PRESETS;
use crate::shell_detection;
use par_term_config::layout_constants::{
    PROFILE_ICON_PICKER_MAX_HEIGHT, PROFILE_ICON_PICKER_MIN_WIDTH,
};
use std::collections::HashSet;

impl ProfileModalUI {
    /// Render the edit/create view
    pub(crate) fn render_edit_view(&mut self, ui: &mut egui::Ui, collapsed: &mut HashSet<String>) {
        // Check if the profile being edited is a dynamic profile
        let is_dynamic_profile = self
            .editing_id
            .and_then(|id| self.working_profiles.iter().find(|p| p.id == id))
            .is_some_and(|p| p.source.is_dynamic());

        let title = match &self.mode {
            ModalMode::Create => "Create Profile",
            ModalMode::Edit(_) => {
                if is_dynamic_profile {
                    "View Profile"
                } else {
                    "Edit Profile"
                }
            }
            _ => "Profile",
        };

        ui.heading(title);

        // Show read-only notice for dynamic profiles
        if is_dynamic_profile {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("ℹ").color(egui::Color32::from_rgb(100, 180, 255)));
                ui.colored_label(
                    egui::Color32::from_rgb(100, 180, 255),
                    "This profile is managed by a remote source and cannot be edited locally.",
                );
            });
        }

        ui.add_space(4.0);
        self.render_edit_tab_bar(ui);
        ui.separator();

        ui.scope(|ui| {
            // Disable all form fields for dynamic (read-only) profiles
            if is_dynamic_profile {
                ui.disable();
            }
            self.render_edit_tab(ui, collapsed);
        });

        // Validation error
        if let Some(error) = &self.validation_error {
            ui.add_space(8.0);
            ui.colored_label(egui::Color32::RED, error);
        }

        // Help text
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(
                "Note: Inherited settings from parent profiles are used when this profile's field is empty.",
            )
            .small()
            .color(egui::Color32::GRAY),
        );

        // Footer buttons
        ui.add_space(8.0);
        ui.separator();
        ui.horizontal(|ui| {
            if is_dynamic_profile {
                // Dynamic profiles are read-only; only show Back button
                if ui.button("Back").clicked() {
                    self.cancel_edit();
                }
            } else {
                if ui
                    .button("Done")
                    .on_hover_text(
                        "Keep these edits and return to the list. Save at the bottom of \
                         the window writes them; Revert drops them.",
                    )
                    .clicked()
                {
                    self.save_form();
                }
                if ui
                    .button("Cancel")
                    .on_hover_text("Drop the edits to this profile")
                    .clicked()
                {
                    self.cancel_edit();
                }
            }
        });
    }

    /// The icon field and its Nerd Font picker popup.
    pub(super) fn render_icon_picker(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.temp_icon);
            let picker_label = if self.temp_icon.is_empty() {
                "\u{ea7b}" // Nerd Font file icon
            } else {
                &self.temp_icon
            };
            let picker_btn = ui.button(picker_label);
            egui::Popup::from_toggle_button_response(&picker_btn)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.set_min_width(PROFILE_ICON_PICKER_MIN_WIDTH);
                    egui::ScrollArea::vertical()
                        .max_height(PROFILE_ICON_PICKER_MAX_HEIGHT)
                        .show(ui, |ui| {
                            for (category, icons) in NERD_FONT_PRESETS {
                                ui.label(egui::RichText::new(*category).small().strong());
                                ui.horizontal_wrapped(|ui| {
                                    for (icon, label) in *icons {
                                        let btn = ui.add_sized(
                                            [28.0, 28.0],
                                            egui::Button::new(
                                                egui::RichText::new(*icon).size(16.0),
                                            )
                                            .frame(false),
                                        );
                                        if btn.on_hover_text(*label).clicked() {
                                            self.temp_icon = icon.to_string();
                                            egui::Popup::close_all(ui.ctx());
                                        }
                                    }
                                });
                                ui.add_space(2.0);
                            }
                            ui.add_space(4.0);
                            if ui.button("Clear icon").clicked() {
                                self.temp_icon.clear();
                                egui::Popup::close_all(ui.ctx());
                            }
                        });
                });
        });
    }

    /// The shell picker: inherit the global shell, or one of the detected
    /// shells.
    pub(super) fn render_shell_selector(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let shells = shell_detection::detected_shells();
            let selected_label = self
                .temp_shell
                .as_ref()
                .map(|path| {
                    shells
                        .iter()
                        .find(|s| s.path == *path)
                        .map(|s| s.name.clone())
                        .unwrap_or_else(|| path.clone())
                })
                .unwrap_or_else(|| "Default (inherit global)".to_string());

            egui::ComboBox::from_id_salt("shell_selector")
                .selected_text(&selected_label)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(self.temp_shell.is_none(), "Default (inherit global)")
                        .clicked()
                    {
                        self.temp_shell = None;
                    }
                    ui.separator();
                    for shell in shells {
                        let is_selected =
                            self.temp_shell.as_ref().is_some_and(|s| s == &shell.path);
                        if ui
                            .selectable_label(
                                is_selected,
                                format!("{} ({})", shell.name, shell.path),
                            )
                            .clicked()
                        {
                            self.temp_shell = Some(shell.path.clone());
                        }
                    }
                });
        });
    }
}

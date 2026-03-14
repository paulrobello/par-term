//! Profile edit/create view for `ProfileModalUI`.
//!
//! Covers: `render_edit_view` (calls into `badge_section` and `parent_selector`).

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

        ui.separator();

        // Form in a scrollable area to handle many fields
        egui::ScrollArea::vertical()
            .max_height(ui.available_height() - 60.0)
            .show(ui, |ui| {
                // Disable all form fields for dynamic (read-only) profiles
                if is_dynamic_profile {
                    ui.disable();
                }

                egui::Grid::new("profile_form")
                    .num_columns(2)
                    .spacing([10.0, 8.0])
                    .show(ui, |ui| {
                        // === Basic Settings ===
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut self.temp_name);
                        ui.end_row();

                        ui.label("Icon:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_icon);
                            let picker_label = if self.temp_icon.is_empty() {
                                "\u{ea7b}" // Nerd Font file icon
                            } else {
                                &self.temp_icon
                            };
                            let picker_btn = ui.button(picker_label);
                            egui::Popup::from_toggle_button_response(&picker_btn)
                                .close_behavior(
                                    egui::PopupCloseBehavior::CloseOnClickOutside,
                                )
                                .show(|ui| {
                                    ui.set_min_width(PROFILE_ICON_PICKER_MIN_WIDTH);
                                    egui::ScrollArea::vertical()
                                        .max_height(PROFILE_ICON_PICKER_MAX_HEIGHT)
                                        .show(ui, |ui| {
                                            for (category, icons) in NERD_FONT_PRESETS {
                                                ui.label(
                                                    egui::RichText::new(*category)
                                                        .small()
                                                        .strong(),
                                                );
                                                ui.horizontal_wrapped(|ui| {
                                                    for (icon, label) in *icons {
                                                        let btn = ui.add_sized(
                                                            [28.0, 28.0],
                                                            egui::Button::new(
                                                                egui::RichText::new(*icon)
                                                                    .size(16.0),
                                                            )
                                                            .frame(false),
                                                        );
                                                        if btn
                                                            .on_hover_text(*label)
                                                            .clicked()
                                                        {
                                                            self.temp_icon =
                                                                icon.to_string();
                                                            egui::Popup::close_all(
                                                                ui.ctx(),
                                                            );
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
                        ui.end_row();

                        ui.label("Working Directory:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_working_dir);
                            if ui.small_button("Browse...").clicked()
                                && let Some(path) = rfd::FileDialog::new().pick_folder()
                            {
                                self.temp_working_dir = path.display().to_string();
                            }
                        });
                        ui.end_row();

                        // Shell selection dropdown
                        ui.label("Shell:");
                        ui.horizontal(|ui| {
                            let shells = shell_detection::detected_shells();
                            let selected_label = self
                                .temp_shell
                                .as_ref()
                                .map(|path| {
                                    // Find display name for selected shell
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
                                    // Default option (inherit global)
                                    if ui
                                        .selectable_label(
                                            self.temp_shell.is_none(),
                                            "Default (inherit global)",
                                        )
                                        .clicked()
                                    {
                                        self.temp_shell = None;
                                    }
                                    ui.separator();
                                    // Detected shells
                                    for shell in shells {
                                        let is_selected = self
                                            .temp_shell
                                            .as_ref()
                                            .is_some_and(|s| s == &shell.path);
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
                        ui.end_row();

                        // Login shell toggle
                        ui.label("Login Shell:");
                        ui.horizontal(|ui| {
                            let mut use_custom = self.temp_login_shell.is_some();
                            if ui.checkbox(&mut use_custom, "").changed() {
                                if use_custom {
                                    self.temp_login_shell = Some(true);
                                } else {
                                    self.temp_login_shell = None;
                                }
                            }
                            if let Some(ref mut login) = self.temp_login_shell {
                                ui.checkbox(login, "Use login shell (-l)");
                            } else {
                                ui.label(
                                    egui::RichText::new("(inherit global)")
                                        .small()
                                        .color(egui::Color32::GRAY),
                                );
                            }
                        });
                        ui.end_row();

                        ui.label("Command:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_command);
                            ui.label(
                                egui::RichText::new("(overrides shell)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();

                        ui.label("Arguments:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_args);
                            ui.label(
                                egui::RichText::new("(space-separated)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();

                        ui.label("Tab Name:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_tab_name);
                            ui.label(
                                egui::RichText::new("(optional)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();
                    });

                // === Enhanced Features Section (issue #78) ===
                ui.add_space(12.0);
                ui.separator();
                ui.label(
                    egui::RichText::new("Enhanced Features")
                        .strong()
                        .color(egui::Color32::LIGHT_BLUE),
                );
                ui.add_space(4.0);

                egui::Grid::new("profile_form_enhanced")
                    .num_columns(2)
                    .spacing([10.0, 8.0])
                    .show(ui, |ui| {
                        // Tags
                        ui.label("Tags:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_tags);
                            ui.label(
                                egui::RichText::new("(comma-separated)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();

                        // Parent profile (inheritance)
                        ui.label("Inherit From:");
                        self.render_parent_selector(ui);
                        ui.end_row();

                        // Keyboard shortcut
                        ui.label("Keyboard Shortcut:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_keyboard_shortcut);
                            ui.label(
                                egui::RichText::new({
                                    #[cfg(target_os = "macos")]
                                    { "(e.g. Cmd+1)" }
                                    #[cfg(not(target_os = "macos"))]
                                    { "(e.g. Ctrl+Shift+1)" }
                                })
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();

                        // Hostname patterns for auto-switching
                        ui.label("Auto-Switch Hosts:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_hostname_patterns);
                            ui.label(
                                egui::RichText::new("(*.example.com)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();

                        // Tmux session patterns for auto-switching
                        ui.label("Auto-Switch Tmux:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_tmux_session_patterns);
                            ui.label(
                                egui::RichText::new("(work-*, *-dev)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();

                        // Directory patterns for auto-switching
                        ui.label("Auto-Switch Dirs:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_directory_patterns);
                            ui.label(
                                egui::RichText::new("(~/projects/work-*)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();

                        // Badge text
                        ui.label("Badge Text:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_badge_text);
                            ui.label(
                                egui::RichText::new("(overrides global)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();
                    });

                // Badge Appearance section (collapsible)
                ui.add_space(8.0);
                self.render_badge_section(ui, collapsed);

                // Tmux auto-connect section
                ui.add_space(8.0);
                self.render_tmux_section(ui, collapsed);

                // SSH Connection section
                ui.add_space(8.0);
                self.render_ssh_section(ui, collapsed);

                // Validation error
                if let Some(error) = &self.validation_error {
                    ui.add_space(8.0);
                    ui.colored_label(egui::Color32::RED, error);
                }

                // Help text
                ui.add_space(16.0);
                ui.label(
                    egui::RichText::new(
                        "Note: Inherited settings from parent profiles are used when this profile's field is empty.",
                    )
                    .small()
                    .color(egui::Color32::GRAY),
                );
            });

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
                if ui.button("Save Profile").clicked() {
                    self.save_form();
                }
                if ui.button("Cancel").clicked() {
                    self.cancel_edit();
                }
            }
        });
    }
}

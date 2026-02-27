//! Profile edit/create view rendering for the profile modal.

use crate::settings_ui::nerd_font::NERD_FONT_PRESETS;
use crate::shell_detection;
use crate::ui_constants::{
    PROFILE_ICON_PICKER_MAX_HEIGHT, PROFILE_ICON_PICKER_MIN_WIDTH, PROFILE_SSH_PORT_FIELD_WIDTH,
};

use super::state::{ModalMode, ProfileModalUI};

impl ProfileModalUI {
    /// Render the edit/create view
    pub(crate) fn render_edit_view(&mut self, ui: &mut egui::Ui) {
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
                egui::CollapsingHeader::new(
                    egui::RichText::new("Badge Appearance")
                        .strong()
                        .color(egui::Color32::LIGHT_BLUE),
                )
                .default_open(self.badge_section_expanded)
                .show(ui, |ui| {
                    self.badge_section_expanded = true;
                    egui::Grid::new("profile_form_badge_appearance")
                        .num_columns(2)
                        .spacing([10.0, 8.0])
                        .show(ui, |ui| {
                            // Badge color
                            ui.label("Color:");
                            ui.horizontal(|ui| {
                                let mut use_custom = self.temp_badge_color.is_some();
                                if ui.checkbox(&mut use_custom, "").changed() {
                                    if use_custom {
                                        self.temp_badge_color = Some([255, 0, 0]); // Default red
                                    } else {
                                        self.temp_badge_color = None;
                                    }
                                }
                                if let Some(ref mut color) = self.temp_badge_color {
                                    let mut egui_color =
                                        egui::Color32::from_rgb(color[0], color[1], color[2]);
                                    if egui::color_picker::color_edit_button_srgba(
                                        ui,
                                        &mut egui_color,
                                        egui::color_picker::Alpha::Opaque,
                                    )
                                    .changed()
                                    {
                                        *color = [egui_color.r(), egui_color.g(), egui_color.b()];
                                    }
                                } else {
                                    ui.label(
                                        egui::RichText::new("(use global)")
                                            .small()
                                            .color(egui::Color32::GRAY),
                                    );
                                }
                            });
                            ui.end_row();

                            // Badge alpha/opacity
                            ui.label("Opacity:");
                            ui.horizontal(|ui| {
                                let mut use_custom = self.temp_badge_color_alpha.is_some();
                                if ui.checkbox(&mut use_custom, "").changed() {
                                    if use_custom {
                                        self.temp_badge_color_alpha = Some(0.5);
                                    } else {
                                        self.temp_badge_color_alpha = None;
                                    }
                                }
                                if let Some(ref mut alpha) = self.temp_badge_color_alpha {
                                    ui.add(egui::Slider::new(alpha, 0.0..=1.0).step_by(0.05));
                                } else {
                                    ui.label(
                                        egui::RichText::new("(use global)")
                                            .small()
                                            .color(egui::Color32::GRAY),
                                    );
                                }
                            });
                            ui.end_row();

                            // Badge font
                            ui.label("Font:");
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut self.temp_badge_font);
                                ui.label(
                                    egui::RichText::new("(blank = global)")
                                        .small()
                                        .color(egui::Color32::GRAY),
                                );
                            });
                            ui.end_row();

                            // Badge font bold
                            ui.label("Bold:");
                            ui.horizontal(|ui| {
                                let mut use_custom = self.temp_badge_font_bold.is_some();
                                if ui.checkbox(&mut use_custom, "").changed() {
                                    if use_custom {
                                        self.temp_badge_font_bold = Some(true);
                                    } else {
                                        self.temp_badge_font_bold = None;
                                    }
                                }
                                if let Some(ref mut bold) = self.temp_badge_font_bold {
                                    ui.checkbox(bold, "Bold text");
                                } else {
                                    ui.label(
                                        egui::RichText::new("(use global)")
                                            .small()
                                            .color(egui::Color32::GRAY),
                                    );
                                }
                            });
                            ui.end_row();

                            // Badge top margin
                            ui.label("Top Margin:");
                            ui.horizontal(|ui| {
                                let mut use_custom = self.temp_badge_top_margin.is_some();
                                if ui.checkbox(&mut use_custom, "").changed() {
                                    if use_custom {
                                        self.temp_badge_top_margin = Some(0.0);
                                    } else {
                                        self.temp_badge_top_margin = None;
                                    }
                                }
                                if let Some(ref mut margin) = self.temp_badge_top_margin {
                                    ui.add(egui::DragValue::new(margin).range(0.0..=100.0).suffix(" px"));
                                } else {
                                    ui.label(
                                        egui::RichText::new("(use global)")
                                            .small()
                                            .color(egui::Color32::GRAY),
                                    );
                                }
                            });
                            ui.end_row();

                            // Badge right margin
                            ui.label("Right Margin:");
                            ui.horizontal(|ui| {
                                let mut use_custom = self.temp_badge_right_margin.is_some();
                                if ui.checkbox(&mut use_custom, "").changed() {
                                    if use_custom {
                                        self.temp_badge_right_margin = Some(16.0);
                                    } else {
                                        self.temp_badge_right_margin = None;
                                    }
                                }
                                if let Some(ref mut margin) = self.temp_badge_right_margin {
                                    ui.add(egui::DragValue::new(margin).range(0.0..=100.0).suffix(" px"));
                                } else {
                                    ui.label(
                                        egui::RichText::new("(use global)")
                                            .small()
                                            .color(egui::Color32::GRAY),
                                    );
                                }
                            });
                            ui.end_row();

                            // Badge max width
                            ui.label("Max Width:");
                            ui.horizontal(|ui| {
                                let mut use_custom = self.temp_badge_max_width.is_some();
                                if ui.checkbox(&mut use_custom, "").changed() {
                                    if use_custom {
                                        self.temp_badge_max_width = Some(0.5);
                                    } else {
                                        self.temp_badge_max_width = None;
                                    }
                                }
                                if let Some(ref mut width) = self.temp_badge_max_width {
                                    ui.add(
                                        egui::Slider::new(width, 0.1..=1.0)
                                            .step_by(0.05)
                                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                                    );
                                } else {
                                    ui.label(
                                        egui::RichText::new("(use global)")
                                            .small()
                                            .color(egui::Color32::GRAY),
                                    );
                                }
                            });
                            ui.end_row();

                            // Badge max height
                            ui.label("Max Height:");
                            ui.horizontal(|ui| {
                                let mut use_custom = self.temp_badge_max_height.is_some();
                                if ui.checkbox(&mut use_custom, "").changed() {
                                    if use_custom {
                                        self.temp_badge_max_height = Some(0.2);
                                    } else {
                                        self.temp_badge_max_height = None;
                                    }
                                }
                                if let Some(ref mut height) = self.temp_badge_max_height {
                                    ui.add(
                                        egui::Slider::new(height, 0.05..=0.5)
                                            .step_by(0.05)
                                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                                    );
                                } else {
                                    ui.label(
                                        egui::RichText::new("(use global)")
                                            .small()
                                            .color(egui::Color32::GRAY),
                                    );
                                }
                            });
                            ui.end_row();
                        });

                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Check boxes to override global badge settings for this profile.")
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                });

                // SSH Connection section
                ui.add_space(8.0);
                egui::CollapsingHeader::new(
                    egui::RichText::new("SSH Connection")
                        .strong()
                        .color(egui::Color32::LIGHT_BLUE),
                )
                .default_open(self.ssh_section_expanded)
                .show(ui, |ui| {
                    self.ssh_section_expanded = true;
                    ui.horizontal(|ui| {
                        ui.label("Host:");
                        ui.text_edit_singleline(&mut self.temp_ssh_host);
                    });
                    ui.horizontal(|ui| {
                        ui.label("User:");
                        ui.text_edit_singleline(&mut self.temp_ssh_user);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Port:");
                        ui.add(egui::TextEdit::singleline(&mut self.temp_ssh_port).desired_width(PROFILE_SSH_PORT_FIELD_WIDTH));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Identity File:");
                        ui.text_edit_singleline(&mut self.temp_ssh_identity_file);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Extra Args:");
                        ui.text_edit_singleline(&mut self.temp_ssh_extra_args);
                    });
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("When SSH Host is set, opening this profile connects via SSH instead of launching a shell.")
                            .weak()
                            .size(11.0),
                    );
                });

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

    /// Render the parent profile selector dropdown
    fn render_parent_selector(&mut self, ui: &mut egui::Ui) {
        // Get valid parents (excludes self and profiles that would create cycles)
        let current_id = self.editing_id;
        let valid_parents: Vec<_> = self
            .working_profiles
            .iter()
            .filter(|p| {
                // Cannot select self as parent
                if Some(p.id) == current_id {
                    return false;
                }
                // Prevent cycles: reject if this candidate has current profile as ancestor
                if let Some(cid) = current_id
                    && self.has_ancestor(p.id, cid)
                {
                    return false;
                }
                true
            })
            .map(|p| (p.id, p.display_label()))
            .collect();

        let selected_label = self
            .temp_parent_id
            .and_then(|id| self.working_profiles.iter().find(|p| p.id == id))
            .map(|p| p.display_label())
            .unwrap_or_else(|| "(None)".to_string());

        egui::ComboBox::from_id_salt("parent_profile_selector")
            .selected_text(&selected_label)
            .show_ui(ui, |ui| {
                // Option to clear parent
                if ui
                    .selectable_label(self.temp_parent_id.is_none(), "(None)")
                    .clicked()
                {
                    self.temp_parent_id = None;
                }
                // List valid parents
                for (id, label) in valid_parents {
                    if ui
                        .selectable_label(self.temp_parent_id == Some(id), &label)
                        .clicked()
                    {
                        self.temp_parent_id = Some(id);
                    }
                }
            });
    }
}

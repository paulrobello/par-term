//! Profile list view rendering for the profile modal.

use super::state::{ProfileModalAction, ProfileModalUI};

impl ProfileModalUI {
    /// Render the list view
    pub(crate) fn render_list_view(&mut self, ui: &mut egui::Ui) -> ProfileModalAction {
        let mut action = ProfileModalAction::None;

        // Header with create button
        ui.horizontal(|ui| {
            ui.heading("Profiles");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("+ New Profile").clicked() {
                    self.start_create();
                }
            });
        });
        ui.separator();

        // Profile list
        let available_height = ui.available_height() - 50.0; // Reserve space for footer
        egui::ScrollArea::vertical()
            .max_height(available_height)
            .show(ui, |ui| {
                if self.working_profiles.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            egui::RichText::new("No profiles yet")
                                .italics()
                                .color(egui::Color32::GRAY),
                        );
                        ui.add_space(10.0);
                        ui.label("Click '+ New Profile' to create one");
                    });
                } else {
                    for (idx, profile) in self.working_profiles.clone().iter().enumerate() {
                        let is_selected = self.selected_id == Some(profile.id);

                        // Use push_id with profile.id to ensure stable widget ID for double-click detection
                        ui.push_id(profile.id, |ui| {
                            let bg_color = if is_selected {
                                egui::Color32::from_rgba_unmultiplied(70, 100, 140, 150)
                            } else {
                                egui::Color32::TRANSPARENT
                            };

                            let frame = egui::Frame::NONE
                                .fill(bg_color)
                                .inner_margin(egui::Margin::symmetric(8, 4))
                                .corner_radius(4.0);

                            frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Reorder buttons
                                    ui.add_enabled_ui(idx > 0, |ui| {
                                        if ui.small_button("Up").clicked() {
                                            self.move_up(profile.id);
                                        }
                                    });
                                    ui.add_enabled_ui(
                                        idx < self.working_profiles.len() - 1,
                                        |ui| {
                                            if ui.small_button("Dn").clicked() {
                                                self.move_down(profile.id);
                                            }
                                        },
                                    );

                                    // Icon and name
                                    if let Some(icon) = &profile.icon {
                                        ui.label(icon);
                                    }
                                    let name_response =
                                        ui.selectable_label(is_selected, &profile.name);
                                    if name_response.clicked() {
                                        self.selected_id = Some(profile.id);
                                    }
                                    if name_response.double_clicked() {
                                        self.start_edit(profile.id);
                                    }

                                    // Dynamic profile indicator
                                    if profile.source.is_dynamic() {
                                        ui.label(
                                            egui::RichText::new("[dynamic]")
                                                .color(egui::Color32::from_rgb(100, 180, 255))
                                                .small(),
                                        );
                                    }

                                    // Spacer
                                    let is_dynamic = profile.source.is_dynamic();
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            // Delete button (disabled for dynamic profiles)
                                            ui.add_enabled_ui(!is_dynamic, |ui| {
                                                if ui.small_button("🗑").clicked() {
                                                    self.request_delete(
                                                        profile.id,
                                                        profile.name.clone(),
                                                    );
                                                }
                                            });
                                            // Edit/View button
                                            let edit_label =
                                                if is_dynamic { "👁" } else { "✏" };
                                            if ui.small_button(edit_label).clicked() {
                                                self.start_edit(profile.id);
                                            }
                                        },
                                    );
                                });
                            });
                        });
                    }
                }
            });

        // Footer buttons
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                action = ProfileModalAction::Save;
                // Don't call close() here - the caller needs to get working_profiles first
                // The caller will close the modal after retrieving the profiles
                self.visible = false;
            }
            if ui.button("Cancel").clicked() {
                action = ProfileModalAction::Cancel;
                self.close();
            }

            if self.has_changes {
                ui.colored_label(egui::Color32::YELLOW, "* Unsaved changes");
            }
        });

        action
    }
}

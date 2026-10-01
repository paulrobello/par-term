//! Profile list view for `ProfileModalUI`. Rows use the shared list-editor
//! buttons (UX.md SC4); a delete is confirmed by the row's second click.

use super::{ProfileModalAction, ProfileModalUI};

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

        // Profile list. No scroll area of its own: the Settings content area
        // already scrolls (UX.md SC9).
        ui.scope(|ui| {
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
                let len = self.working_profiles.len();
                let mut row_action = None;
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
                                // Icon and name
                                if let Some(icon) = &profile.icon {
                                    ui.label(icon);
                                }
                                let name_response = ui.selectable_label(is_selected, &profile.name);
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

                                // Dynamic profiles are read-only: View, no
                                // delete or duplicate (UX.md SC4 row).
                                let is_dynamic = profile.source.is_dynamic();
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if is_dynamic {
                                            if ui.small_button("View").clicked() {
                                                self.start_edit(profile.id);
                                            }
                                            return;
                                        }
                                        let key = profile.id.to_string();
                                        if let Some(a) = crate::list_editor::row_actions(
                                            ui,
                                            &mut self.pending_row_delete,
                                            crate::list_editor::Row {
                                                index: idx,
                                                len,
                                                list: "profile",
                                                key: &key,
                                                delete_label: "Delete",
                                            },
                                            crate::list_editor::RowButtons::ALL,
                                        ) {
                                            row_action = Some(a);
                                        }
                                    },
                                );
                            });
                        });
                    });
                }
                if let Some(a) = row_action {
                    self.apply_row_action(a);
                }
            }
        });

        // Profile edits join the Settings window's Save / Revert (UX.md SS5),
        // so the list has no Save or Cancel of its own. Emptying a non-empty
        // list still asks first; confirming runs the global Save.
        if self.confirm_empty_save {
            ui.separator();
            ui.group(|ui| {
                ui.label(
                    egui::RichText::new("⚠ Save with no profiles?")
                        .strong()
                        .color(egui::Color32::from_rgb(244, 67, 54)),
                );
                ui.label(format!(
                    "This deletes all {} saved profiles.",
                    self.baseline_profiles.len()
                ));
                ui.horizontal(|ui| {
                    if ui.button("Delete All Profiles").clicked() {
                        action = self.confirm_empty_list_save();
                    }
                    if ui.button("Cancel").clicked() {
                        self.confirm_empty_save = false;
                    }
                });
            });
        }

        // The legacy modal window hides on Save; the caller reads
        // working_profiles before anything clears them.
        if action == ProfileModalAction::Save {
            self.visible = false;
        }

        action
    }
}

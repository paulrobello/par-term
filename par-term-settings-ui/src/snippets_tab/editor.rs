//! Snippet edit form — used for both new snippets and editing existing ones.

use super::SettingsUI;
use crate::input_tab::capture_key_combo;
use crate::section::collapsing_section;
use par_term_config::snippets::SnippetConfig;
use std::collections::HashSet;

/// Show the snippet edit form (for both new and existing snippets).
pub(super) fn show_snippet_edit_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    edit_index: Option<usize>,
    collapsed: &mut HashSet<String>,
) {
    ui.separator();

    // Buttons at TOP - always visible first
    ui.horizontal(|ui| {
        if ui.button("Save").clicked() {
            let snippet = SnippetConfig {
                id: settings.temp_snippet_id.clone(),
                title: settings.temp_snippet_title.clone(),
                content: settings.temp_snippet_content.clone(),
                keybinding: if settings.temp_snippet_keybinding.is_empty() {
                    None
                } else {
                    Some(settings.temp_snippet_keybinding.clone())
                },
                keybinding_enabled: settings.temp_snippet_keybinding_enabled,
                folder: if settings.temp_snippet_folder.is_empty() {
                    None
                } else {
                    Some(settings.temp_snippet_folder.clone())
                },
                enabled: true,
                description: if settings.temp_snippet_description.is_empty() {
                    None
                } else {
                    Some(settings.temp_snippet_description.clone())
                },
                auto_execute: settings.temp_snippet_auto_execute,
                variables: settings.temp_snippet_variables.iter().cloned().collect(),
            };

            if let Some(i) = edit_index {
                // Update existing snippet
                settings.config.snippets[i] = snippet;
            } else {
                // Add new snippet
                settings.config.snippets.push(snippet);
            }

            settings.has_changes = true;
            *changes_this_frame = true;
            settings.editing_snippet_index = None;
            settings.adding_new_snippet = false;
        }

        if ui.button("Cancel").clicked() {
            settings.editing_snippet_index = None;
            settings.adding_new_snippet = false;
        }
    });

    ui.separator();

    // Scrollable area for form fields
    egui::ScrollArea::vertical()
        .max_height(300.0)
        .show(ui, |ui| {
            ui.label("Title:");
            if ui
                .text_edit_singleline(&mut settings.temp_snippet_title)
                .changed()
            {
                *changes_this_frame = true;
            }

            ui.label("ID:");
            ui.label(
                egui::RichText::new(&settings.temp_snippet_id)
                    .monospace()
                    .small(),
            );

            ui.label("Content:");
            if ui
                .text_edit_multiline(&mut settings.temp_snippet_content)
                .changed()
            {
                *changes_this_frame = true;
            }

            ui.horizontal(|ui| {
                if ui
                    .checkbox(&mut settings.temp_snippet_auto_execute, "Auto-execute")
                    .changed()
                {
                    *changes_this_frame = true;
                }
                ui.label(
                    egui::RichText::new("⚡ run immediately")
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });

            ui.label("Keybinding:");
            ui.horizontal(|ui| {
                // Check for recording state
                if settings.recording_snippet_keybinding {
                    // Show recording indicator and capture key combo
                    ui.label(egui::RichText::new("🔴 Recording...").color(egui::Color32::RED));
                    if let Some(combo) = capture_key_combo(ui) {
                        settings.snippet_recorded_combo = Some(combo.clone());
                        settings.temp_snippet_keybinding = combo;
                        settings.recording_snippet_keybinding = false;
                        *changes_this_frame = true;
                    }
                } else {
                    // Show text input and record button
                    if ui
                        .text_edit_singleline(&mut settings.temp_snippet_keybinding)
                        .changed()
                    {
                        *changes_this_frame = true;
                    }

                    // Check for conflicts
                    if !settings.temp_snippet_keybinding.is_empty() {
                        let exclude_id = if let Some(i) = edit_index {
                            settings.config.snippets.get(i).map(|s| s.id.as_ref())
                        } else {
                            None
                        };

                        if let Some(conflict) = settings.check_keybinding_conflict(
                            &settings.temp_snippet_keybinding,
                            exclude_id,
                        ) {
                            ui.label(
                                egui::RichText::new(format!("⚠️ {}", conflict))
                                    .color(egui::Color32::from_rgb(255, 180, 0))
                                    .small(),
                            );
                        }
                    }

                    // Record button
                    if ui
                        .small_button("🎤")
                        .on_hover_text("Record keybinding")
                        .clicked()
                    {
                        settings.recording_snippet_keybinding = true;
                        settings.snippet_recorded_combo = None;
                    }
                }
            });

            // Show keybinding enabled checkbox if keybinding is set
            if !settings.temp_snippet_keybinding.is_empty() {
                ui.horizontal(|ui| {
                    if ui
                        .checkbox(&mut settings.temp_snippet_keybinding_enabled, "Enabled")
                        .changed()
                    {
                        *changes_this_frame = true;
                    }
                    ui.label(
                        egui::RichText::new("(disable without removing)")
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                });
            }

            ui.label("Folder:");
            if ui
                .text_edit_singleline(&mut settings.temp_snippet_folder)
                .changed()
            {
                *changes_this_frame = true;
            }

            ui.label("Description:");
            if ui
                .text_edit_singleline(&mut settings.temp_snippet_description)
                .changed()
            {
                *changes_this_frame = true;
            }

            // Custom Variables section
            let var_count = settings.temp_snippet_variables.len();
            let header_text = if var_count > 0 {
                format!("Custom Variables ({})", var_count)
            } else {
                "Custom Variables".to_string()
            };
            ui.add_space(4.0);
            collapsing_section(
                ui,
                &header_text,
                "snippet_custom_variables",
                true,
                collapsed,
                |ui| {
                    let mut delete_var_index: Option<usize> = None;

                    if !settings.temp_snippet_variables.is_empty() {
                        egui::Grid::new("snippet_variables_edit_grid")
                            .num_columns(3)
                            .spacing([8.0, 4.0])
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("Name").small().strong());
                                ui.label(egui::RichText::new("Value").small().strong());
                                ui.label(""); // Delete column header
                                ui.end_row();

                                for i in 0..settings.temp_snippet_variables.len() {
                                    let (ref mut name, ref mut value) =
                                        settings.temp_snippet_variables[i];

                                    let name_response = ui.add(
                                        egui::TextEdit::singleline(name)
                                            .desired_width(120.0)
                                            .hint_text("name"),
                                    );
                                    if name_response.changed() {
                                        *changes_this_frame = true;
                                    }

                                    let value_response = ui.add(
                                        egui::TextEdit::singleline(value)
                                            .desired_width(160.0)
                                            .hint_text("value"),
                                    );
                                    if value_response.changed() {
                                        *changes_this_frame = true;
                                    }

                                    if ui
                                        .small_button(
                                            egui::RichText::new("✕")
                                                .color(egui::Color32::from_rgb(200, 80, 80)),
                                        )
                                        .on_hover_text("Remove variable")
                                        .clicked()
                                    {
                                        delete_var_index = Some(i);
                                    }
                                    ui.end_row();

                                    // Warn on empty name
                                    if name.is_empty() {
                                        ui.label(
                                            egui::RichText::new("⚠ Name required")
                                                .small()
                                                .color(egui::Color32::from_rgb(255, 180, 0)),
                                        );
                                        ui.label("");
                                        ui.label("");
                                        ui.end_row();
                                    }
                                }
                            });
                    }

                    if let Some(idx) = delete_var_index {
                        settings.temp_snippet_variables.remove(idx);
                        *changes_this_frame = true;
                    }

                    if ui.small_button("+ Add Variable").clicked() {
                        settings
                            .temp_snippet_variables
                            .push((String::new(), String::new()));
                        *changes_this_frame = true;
                    }

                    ui.label(
                        egui::RichText::new(
                            "Use \\(name) in content to reference custom variables",
                        )
                        .small()
                        .color(egui::Color32::GRAY),
                    );
                },
            );

            // Show variable hint
            ui.label(
                egui::RichText::new("💡 Variables: \\(date), \\(time), \\(session.path), etc.")
                    .small()
                    .color(egui::Color32::GRAY),
            );
        });

    ui.separator();
}

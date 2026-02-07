//! Snippets settings tab.
//!
//! Contains:
//! - Text snippet management (CRUD operations)
//! - Snippet editor with variable substitution support
//! - Keybinding assignment for snippets
//! - Folder organization for snippets
//! - Import/export functionality

use super::SettingsUI;
use super::section::collapsing_section;
use crate::config::snippets::SnippetConfig;
use crate::settings_ui::input_tab::capture_key_combo;
use std::collections::HashMap;

/// Show the snippets tab content.
pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    let query = settings.search_query.trim().to_lowercase();

    // Snippets section
    if section_matches(
        &query,
        "Snippets",
        &[
            "snippet",
            "text",
            "insert",
            "template",
            "variable",
            "keybinding",
            "folder",
        ],
    ) {
        show_snippets_section(ui, settings, changes_this_frame);
    }

    // Variables reference section (collapsed by default)
    if section_matches(
        &query,
        "Variables Reference",
        &[
            "variable",
            "builtin",
            "built-in",
            "reference",
            "date",
            "time",
            "hostname",
        ],
    ) {
        show_variables_reference_section(ui, settings);
    }
}

fn section_matches(query: &str, title: &str, keywords: &[&str]) -> bool {
    if query.is_empty() {
        return true;
    }
    if title.to_lowercase().contains(query) {
        return true;
    }
    keywords.iter().any(|k| k.to_lowercase().contains(query))
}

// ============================================================================
// Snippets Section
// ============================================================================

fn show_snippets_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Snippets", "snippets_list", true, |ui| {
        ui.label("Saved text blocks for quick insertion. Supports variable substitution.");
        ui.add_space(4.0);

        // Collect mutations to apply after iteration
        let mut delete_index: Option<usize> = None;
        let mut toggle_index: Option<usize> = None;
        let mut start_edit_index: Option<usize> = None;

        // Group snippets by folder
        let mut folders: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, snippet) in settings.config.snippets.iter().enumerate() {
            let folder = snippet.folder.as_deref().unwrap_or("");
            folders.entry(folder.to_string()).or_default().push(i);
        }

        // Sort folders: unsorted first, then alphabetically
        let mut folder_names: Vec<String> = folders.keys().cloned().collect();
        folder_names.sort_by(|a, b| {
            if a.is_empty() {
                std::cmp::Ordering::Less
            } else if b.is_empty() {
                std::cmp::Ordering::Greater
            } else {
                a.cmp(b)
            }
        });

        // Show snippets grouped by folder
        for folder_name in folder_names {
            let indices = &folders[&folder_name];

            // Show folder header if not empty
            if !folder_name.is_empty() {
                ui.separator();
                ui.label(egui::RichText::new(&folder_name).strong());
            }

            for &i in indices {
                let snippet = &settings.config.snippets[i];
                let is_editing =
                    settings.editing_snippet_index == Some(i) && !settings.adding_new_snippet;

                if is_editing {
                    // Show inline edit form for this snippet
                    show_snippet_edit_form(ui, settings, changes_this_frame, Some(i));
                } else {
                    // Show snippet summary row
                    ui.horizontal(|ui| {
                        // Enabled checkbox
                        let mut enabled = snippet.enabled;
                        if ui.checkbox(&mut enabled, "").changed() {
                            toggle_index = Some(i);
                        }

                        // Title (bold)
                        ui.label(egui::RichText::new(&snippet.title).strong());

                        // Keybinding (if any)
                        if let Some(keybinding) = &snippet.keybinding {
                            ui.label(
                                egui::RichText::new(format!("[{}]", keybinding))
                                    .monospace()
                                    .color(egui::Color32::from_rgb(150, 150, 200)),
                            );
                        }

                        // Content preview (truncated)
                        let preview = if snippet.content.len() > 40 {
                            format!("{}...", &snippet.content[..40])
                        } else {
                            snippet.content.clone()
                        };
                        ui.label(
                            egui::RichText::new(preview)
                                .monospace()
                                .color(egui::Color32::GRAY),
                        );

                        // Edit button
                        if ui.small_button("Edit").clicked() {
                            start_edit_index = Some(i);
                        }

                        // Delete button
                        if ui
                            .small_button(
                                egui::RichText::new("Delete")
                                    .color(egui::Color32::from_rgb(200, 80, 80)),
                            )
                            .clicked()
                        {
                            delete_index = Some(i);
                        }
                    });
                }
            }
        }

        // Apply mutations after iteration
        if let Some(i) = delete_index {
            settings.config.snippets.remove(i);
            settings.has_changes = true;
            *changes_this_frame = true;
            // Reset editing state if we deleted the item being edited
            if settings.editing_snippet_index == Some(i) {
                settings.editing_snippet_index = None;
                settings.adding_new_snippet = false;
            }
        }

        if let Some(i) = toggle_index {
            settings.config.snippets[i].enabled = !settings.config.snippets[i].enabled;
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if let Some(i) = start_edit_index {
            settings.editing_snippet_index = Some(i);
            settings.adding_new_snippet = false;
            // Populate temp fields with current values
            let snippet = &settings.config.snippets[i];
            settings.temp_snippet_id = snippet.id.clone();
            settings.temp_snippet_title = snippet.title.clone();
            settings.temp_snippet_content = snippet.content.clone();
            settings.temp_snippet_keybinding = snippet.keybinding.clone().unwrap_or_default();
            settings.temp_snippet_folder = snippet.folder.clone().unwrap_or_default();
            settings.temp_snippet_description = snippet.description.clone().unwrap_or_default();
            settings.temp_snippet_keybinding_enabled = snippet.keybinding_enabled;
        }

        ui.separator();

        // Add new snippet button or form
        if settings.adding_new_snippet {
            show_snippet_edit_form(ui, settings, changes_this_frame, None);
        } else {
            if ui.button("+ Add Snippet").clicked() {
                settings.adding_new_snippet = true;
                settings.editing_snippet_index = None;
                // Clear temp fields
                settings.temp_snippet_id = format!("snippet_{}", uuid::Uuid::new_v4());
                settings.temp_snippet_title = String::new();
                settings.temp_snippet_content = String::new();
                settings.temp_snippet_keybinding = String::new();
                settings.temp_snippet_folder = String::new();
                settings.temp_snippet_description = String::new();
                settings.temp_snippet_keybinding_enabled = true;
            }
        }
    });
}

/// Show the snippet edit form (for both new and existing snippets).
fn show_snippet_edit_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    edit_index: Option<usize>,
) {
    ui.horizontal(|ui| {
        ui.label("Title:");
        if ui.text_edit_singleline(&mut settings.temp_snippet_title).changed() {
            *changes_this_frame = true;
        }
    });

    ui.horizontal(|ui| {
        ui.label("ID:");
        ui.label(egui::RichText::new(&settings.temp_snippet_id).monospace().small());
    });

    ui.horizontal(|ui| {
        ui.label("Content:");
        if ui.text_edit_multiline(&mut settings.temp_snippet_content).changed() {
            *changes_this_frame = true;
        }
    });

    ui.horizontal(|ui| {
        ui.label("Keybinding (optional):");

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
            if ui.text_edit_singleline(&mut settings.temp_snippet_keybinding).changed() {
                *changes_this_frame = true;
            }

            // Check for conflicts
            if !settings.temp_snippet_keybinding.is_empty() {
                let exclude_id = if let Some(i) = edit_index {
                    settings.config.snippets.get(i).map(|s| s.id.as_ref())
                } else {
                    None
                };

                if let Some(conflict) = settings.check_keybinding_conflict(&settings.temp_snippet_keybinding, exclude_id) {
                    ui.label(
                        egui::RichText::new(format!("⚠️ {}", conflict))
                            .color(egui::Color32::from_rgb(255, 180, 0))
                            .small(),
                    );
                }
            }

            // Record button
            if ui.small_button("🎤 Record").clicked() {
                settings.recording_snippet_keybinding = true;
                settings.snippet_recorded_combo = None;
            }
        }
    });

    // Show keybinding enabled checkbox if keybinding is set
    if !settings.temp_snippet_keybinding.is_empty() {
        ui.horizontal(|ui| {
            if ui.checkbox(&mut settings.temp_snippet_keybinding_enabled, "Enable keybinding").changed() {
                *changes_this_frame = true;
            }
            ui.label(egui::RichText::new("(uncheck to disable without removing)")
                .small()
                .color(egui::Color32::GRAY));
        });
    }

    ui.horizontal(|ui| {
        ui.label("Folder (optional):");
        if ui.text_edit_singleline(&mut settings.temp_snippet_folder).changed() {
            *changes_this_frame = true;
        }
    });

    ui.horizontal(|ui| {
        ui.label("Description (optional):");
        if ui.text_edit_singleline(&mut settings.temp_snippet_description).changed() {
            *changes_this_frame = true;
        }
    });

    // Show variable hint
    ui.label(
        egui::RichText::new("💡 Use \\(variable) for substitution: \\(date), \\(time), \\(user), \\(path), etc.")
            .small()
            .color(egui::Color32::GRAY),
    );

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
                variables: HashMap::new(), // TODO: Add custom variables UI
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
}

// ============================================================================
// Variables Reference Section
// ============================================================================

fn show_variables_reference_section(ui: &mut egui::Ui, _settings: &mut SettingsUI) {
    collapsing_section(ui, "Variables Reference", "snippets_variables", false, |ui| {
        ui.label("Built-in variables available for use in snippets:");
        ui.add_space(4.0);

        use crate::config::snippets::BuiltInVariable;

        egui::Grid::new("snippet_variables_grid")
            .num_columns(2)
            .spacing([20.0, 4.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Variable").strong());
                ui.label(egui::RichText::new("Description").strong());
                ui.end_row();

                for (name, description) in BuiltInVariable::all() {
                    ui.label(egui::RichText::new(format!("\\({})", name)).monospace());
                    ui.label(egui::RichText::new(*description).small().color(egui::Color32::GRAY));
                    ui.end_row();
                }
            });

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Example: \"echo 'Report for \\(user) on \\(date)'\"")
                .monospace()
                .small(),
        );
    });
}

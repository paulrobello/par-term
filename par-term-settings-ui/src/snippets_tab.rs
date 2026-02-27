//! Snippets settings tab.
//!
//! Contains:
//! - Text snippet management (CRUD operations)
//! - Snippet editor with variable substitution support
//! - Keybinding assignment for snippets
//! - Folder organization for snippets
//! - Import/export functionality

use super::SettingsUI;
use super::section::{collapsing_section, collapsing_section_with_state, section_matches};
use crate::input_tab::{capture_key_combo, display_key_combo};
use par_term_config::snippets::{SnippetConfig, SnippetLibrary};
use std::collections::HashMap;
use std::collections::HashSet;

/// Show the snippets tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
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
            "shortcut",
            "quick insert",
            "auto-execute",
        ],
    ) {
        show_snippets_section(ui, settings, changes_this_frame, collapsed);
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
        show_variables_reference_section(ui, settings, collapsed);
    }
}

// ============================================================================
// Snippets Section
// ============================================================================

fn show_snippets_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section_with_state(
        ui,
        "Snippets",
        "snippets_list",
        true,
        collapsed,
        |ui, collapsed| {
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
                        show_snippet_edit_form(
                            ui,
                            settings,
                            changes_this_frame,
                            Some(i),
                            collapsed,
                        );
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
                                    egui::RichText::new(format!(
                                        "[{}]",
                                        display_key_combo(keybinding)
                                    ))
                                    .monospace()
                                    .color(egui::Color32::from_rgb(150, 150, 200)),
                                );
                            }

                            // Right-aligned buttons + truncated preview for remaining space
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // Delete button (rightmost)
                                    if ui
                                        .small_button(
                                            egui::RichText::new("Delete")
                                                .color(egui::Color32::from_rgb(200, 80, 80)),
                                        )
                                        .clicked()
                                    {
                                        delete_index = Some(i);
                                    }

                                    // Edit button
                                    if ui.small_button("Edit").clicked() {
                                        start_edit_index = Some(i);
                                    }

                                    // Content preview (truncated to remaining space)
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&snippet.content)
                                                .monospace()
                                                .color(egui::Color32::GRAY),
                                        )
                                        .truncate(),
                                    );
                                },
                            );
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
                settings.temp_snippet_auto_execute = snippet.auto_execute;
                settings.temp_snippet_variables = snippet
                    .variables
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
            }

            ui.separator();

            // Add new snippet button or form
            if settings.adding_new_snippet {
                show_snippet_edit_form(ui, settings, changes_this_frame, None, collapsed);
            } else {
                ui.horizontal(|ui| {
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
                        settings.temp_snippet_auto_execute = false;
                        settings.temp_snippet_variables = Vec::new();
                    }

                    ui.separator();

                    if ui
                        .button("Export")
                        .on_hover_text("Export all snippets to a YAML file")
                        .clicked()
                    {
                        export_snippets(settings);
                    }

                    if ui
                        .button("Import")
                        .on_hover_text("Import snippets from a YAML file")
                        .clicked()
                    {
                        import_snippets(settings, changes_this_frame);
                    }
                });
            }
        },
    );
}

/// Show the snippet edit form (for both new and existing snippets).
fn show_snippet_edit_form(
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

// ============================================================================
// Import / Export
// ============================================================================

/// Export all snippets to a YAML file via a save dialog.
fn export_snippets(settings: &mut SettingsUI) {
    let path = rfd::FileDialog::new()
        .set_title("Export Snippets")
        .add_filter("YAML", &["yaml", "yml"])
        .set_file_name("snippets.yaml")
        .save_file();

    if let Some(path) = path {
        let library = SnippetLibrary {
            snippets: settings.config.snippets.clone(),
        };
        match serde_yml::to_string(&library) {
            Ok(yaml) => {
                if let Err(e) = std::fs::write(&path, yaml) {
                    log::error!("Failed to write snippet library: {}", e);
                } else {
                    log::info!(
                        "Exported {} snippets to {}",
                        library.snippets.len(),
                        path.display()
                    );
                }
            }
            Err(e) => {
                log::error!("Failed to serialize snippet library: {}", e);
            }
        }
    }
}

/// Import snippets from a YAML file via an open dialog.
///
/// Merges imported snippets with existing ones, skipping duplicates by ID.
fn import_snippets(settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    let path = rfd::FileDialog::new()
        .set_title("Import Snippets")
        .add_filter("YAML", &["yaml", "yml"])
        .pick_file();

    if let Some(path) = path {
        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_yml::from_str::<SnippetLibrary>(&content) {
                Ok(library) => {
                    let existing_ids: std::collections::HashSet<String> = settings
                        .config
                        .snippets
                        .iter()
                        .map(|s| s.id.clone())
                        .collect();

                    let mut imported = 0usize;
                    let mut skipped = 0usize;

                    for mut snippet in library.snippets {
                        if existing_ids.contains(&snippet.id) {
                            skipped += 1;
                            continue;
                        }

                        // Clear keybinding if it conflicts with an existing one
                        if let Some(ref kb) = snippet.keybinding
                            && settings.check_keybinding_conflict(kb, None).is_some()
                        {
                            snippet.keybinding = None;
                        }

                        settings.config.snippets.push(snippet);
                        imported += 1;
                    }

                    if imported > 0 {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }

                    log::info!(
                        "Imported {} snippets ({} skipped as duplicates) from {}",
                        imported,
                        skipped,
                        path.display()
                    );
                }
                Err(e) => {
                    log::error!("Failed to parse snippet library: {}", e);
                }
            },
            Err(e) => {
                log::error!("Failed to read snippet file: {}", e);
            }
        }
    }
}

// ============================================================================
// Variables Reference Section
// ============================================================================

fn show_variables_reference_section(
    ui: &mut egui::Ui,
    _settings: &mut SettingsUI,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Variables Reference",
        "snippets_variables",
        false,
        collapsed,
        |ui| {
            ui.label("Built-in variables available for use in snippets:");
            ui.add_space(4.0);

            use par_term_config::snippets::BuiltInVariable;

            egui::Grid::new("snippet_variables_grid")
                .num_columns(2)
                .spacing([20.0, 4.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Variable").strong());
                    ui.label(egui::RichText::new("Description").strong());
                    ui.end_row();

                    for (name, description) in BuiltInVariable::all() {
                        ui.label(egui::RichText::new(format!("\\({})", name)).monospace());
                        ui.label(
                            egui::RichText::new(*description)
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                        ui.end_row();
                    }
                });

            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Example: \"echo 'Report for \\(user) on \\(date)'\"")
                    .monospace()
                    .small(),
            );
        },
    );
}

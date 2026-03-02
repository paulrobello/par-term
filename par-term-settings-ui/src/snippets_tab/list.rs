//! Snippet list rendering — grouped by folder, with edit/delete/toggle actions.

use super::SettingsUI;
use crate::input_tab::display_key_combo;
use std::collections::HashMap;
use std::collections::HashSet;

/// Render the scrollable snippet list grouped by folder.
///
/// Returns deferred mutations: (delete_index, toggle_index, start_edit_index).
pub(super) fn render_snippet_list(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
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
                super::editor::show_snippet_edit_form(
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
                            egui::RichText::new(format!("[{}]", display_key_combo(keybinding)))
                                .monospace()
                                .color(egui::Color32::from_rgb(150, 150, 200)),
                        );
                    }

                    // Right-aligned buttons + truncated preview for remaining space
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                    });
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
}

/// Render the "Add Snippet / Export / Import" footer bar.
pub(super) fn render_add_import_bar(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    if settings.adding_new_snippet {
        // edit form is rendered from the caller (mod.rs)
        return;
    }

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
            super::io::export_snippets(settings);
        }

        if ui
            .button("Import")
            .on_hover_text("Import snippets from a YAML file")
            .clicked()
        {
            super::io::import_snippets(settings, changes_this_frame);
        }
    });
}

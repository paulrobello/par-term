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
    let mut row_action: Option<crate::list_editor::RowAction> = None;
    let snippet_count = settings.config.snippets.len();

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
            let is_editing = settings.snippets_tab.editing_snippet_index == Some(i)
                && !settings.snippets_tab.adding_new_snippet;

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
                        // Reorder is off: rows are displayed grouped by folder, so a
                        // swap in `config.snippets` would not move a row visibly.
                        match crate::list_editor::row_actions(
                            ui,
                            &mut settings.pending_list_delete,
                            crate::list_editor::Row {
                                index: i,
                                len: snippet_count,
                                list: "snippet",
                                key: &snippet.id,
                                delete_label: "Delete",
                            },
                            crate::list_editor::RowButtons {
                                reorder: false,
                                duplicate: true,
                                edit: true,
                                delete: true,
                            },
                        ) {
                            Some(crate::list_editor::RowAction::Delete(i)) => {
                                delete_index = Some(i);
                            }
                            Some(crate::list_editor::RowAction::Edit(i)) => {
                                start_edit_index = Some(i);
                            }
                            Some(action) => row_action = Some(action),
                            None => {}
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
        if settings.snippets_tab.editing_snippet_index == Some(i) {
            settings.snippets_tab.editing_snippet_index = None;
            settings.snippets_tab.adding_new_snippet = false;
        }
    }

    if let Some(action) = row_action
        && crate::list_editor::apply_move_or_duplicate(
            &mut settings.config.snippets,
            action,
            |copy| {
                copy.id = format!("snippet_{}", uuid::Uuid::new_v4());
                copy.title = format!("{} copy", copy.title);
                // Two snippets must never share a chord.
                copy.keybinding = None;
            },
        )
    {
        settings.has_changes = true;
        *changes_this_frame = true;
        // Rows below the copy shifted down by one.
        if let Some(editing) = settings.snippets_tab.editing_snippet_index
            && let crate::list_editor::RowAction::Duplicate(i) = action
            && editing > i
        {
            settings.snippets_tab.editing_snippet_index = Some(editing + 1);
        }
    }

    if let Some(i) = toggle_index {
        settings.config.snippets[i].enabled = !settings.config.snippets[i].enabled;
        settings.has_changes = true;
        *changes_this_frame = true;
    }

    if let Some(i) = start_edit_index {
        settings.snippets_tab.editing_snippet_index = Some(i);
        settings.snippets_tab.adding_new_snippet = false;
        // Populate temp fields with current values
        let snippet = &settings.config.snippets[i];
        settings.snippets_tab.temp_snippet_id = snippet.id.clone();
        settings.snippets_tab.temp_snippet_title = snippet.title.clone();
        settings.snippets_tab.temp_snippet_content = snippet.content.clone();
        settings.snippets_tab.temp_snippet_keybinding =
            snippet.keybinding.clone().unwrap_or_default();
        settings.snippets_tab.temp_snippet_folder = snippet.folder.clone().unwrap_or_default();
        settings.snippets_tab.temp_snippet_description =
            snippet.description.clone().unwrap_or_default();
        settings.snippets_tab.temp_snippet_keybinding_enabled = snippet.keybinding_enabled;
        settings.snippets_tab.temp_snippet_auto_execute = snippet.auto_execute;
        settings.snippets_tab.temp_snippet_variables = snippet
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
    if settings.snippets_tab.adding_new_snippet {
        // edit form is rendered from the caller (mod.rs)
        return;
    }

    ui.horizontal(|ui| {
        if ui.button("+ Add Snippet").clicked() {
            settings.snippets_tab.adding_new_snippet = true;
            settings.snippets_tab.editing_snippet_index = None;
            // Clear temp fields
            settings.snippets_tab.temp_snippet_id = format!("snippet_{}", uuid::Uuid::new_v4());
            settings.snippets_tab.temp_snippet_title = String::new();
            settings.snippets_tab.temp_snippet_content = String::new();
            settings.snippets_tab.temp_snippet_keybinding = String::new();
            settings.snippets_tab.temp_snippet_folder = String::new();
            settings.snippets_tab.temp_snippet_description = String::new();
            settings.snippets_tab.temp_snippet_keybinding_enabled = true;
            settings.snippets_tab.temp_snippet_auto_execute = false;
            settings.snippets_tab.temp_snippet_variables = Vec::new();
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

#[cfg(test)]
mod tests {
    use super::*;
    use par_term_config::Config;
    use par_term_config::snippets::SnippetConfig;

    fn render(settings: &mut SettingsUI, click: Option<egui::Pos2>) {
        let ctx = egui::Context::default();
        let hover = || {
            click
                .map(|p| vec![egui::Event::PointerMoved(p)])
                .unwrap_or_default()
        };
        let button = |pressed| {
            click
                .map(|pos| {
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::default(),
                        },
                    ]
                })
                .unwrap_or_default()
        };
        for events in [hover(), hover(), button(true), button(false)] {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600.0, 400.0),
                )),
                events,
                ..Default::default()
            };
            let mut changed = false;
            let mut collapsed = HashSet::new();
            let mut output = ctx.run_ui(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    render_snippet_list(ui, settings, &mut changed, &mut collapsed);
                });
            });
            output.textures_delta.clear();
        }
    }

    fn settings_with_one_snippet() -> SettingsUI {
        let config = Config {
            snippets: vec![SnippetConfig::new(
                "s1".to_string(),
                "Greeting".to_string(),
                "hello".to_string(),
            )],
            ..Config::default()
        };
        SettingsUI::new_for_tests(config)
    }

    #[test]
    fn rendering_never_deletes_without_the_confirm_click() {
        let mut settings = settings_with_one_snippet();
        render(&mut settings, None);
        assert_eq!(settings.config.snippets.len(), 1);

        // Another row armed: this row still shows plain Delete and keeps the snippet.
        settings.pending_list_delete = Some(("snippet", "other".to_string()));
        render(&mut settings, None);
        assert_eq!(settings.config.snippets.len(), 1);
    }
}

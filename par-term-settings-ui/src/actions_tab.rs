//! Actions settings tab.
//!
//! Contains:
//! - Custom action management (shell commands, text insertion, key sequences)
//! - Action editor with type selection
//! - Keybinding assignment for actions

use super::SettingsUI;
use super::section::{collapsing_section, section_matches};
use crate::input_tab::capture_key_combo;
use par_term_config::snippets::CustomActionConfig;
use std::collections::HashSet;

/// Show the actions tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // Actions section
    if section_matches(
        &query,
        "Actions",
        &[
            "action",
            "custom",
            "shell",
            "command",
            "text",
            "insert",
            "key",
            "sequence",
            "macro",
            "keybinding",
            "execute",
            "run",
        ],
    ) {
        show_actions_section(ui, settings, changes_this_frame, collapsed);
    }
}

// ============================================================================
// Actions Section
// ============================================================================

fn show_actions_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Custom Actions",
        "actions_list",
        true,
        collapsed,
        |ui| {
            ui.label("Custom actions for shell commands, text insertion, or key sequences.");
            ui.add_space(4.0);

            // Collect mutations to apply after iteration
            let mut delete_index: Option<usize> = None;
            let mut start_edit_index: Option<usize> = None;

            // List existing actions
            let action_count = settings.config.actions.len();
            for i in 0..action_count {
                let action = &settings.config.actions[i];
                let is_editing =
                    settings.editing_action_index == Some(i) && !settings.adding_new_action;

                if is_editing {
                    // Show inline edit form for this action
                    show_action_edit_form(ui, settings, changes_this_frame, Some(i));
                } else {
                    // Show action summary row
                    ui.horizontal(|ui| {
                        // Title (bold)
                        ui.label(egui::RichText::new(action.title()).strong());

                        // Type indicator
                        let type_label = match action {
                            CustomActionConfig::ShellCommand { .. } => "Shell",
                            CustomActionConfig::InsertText { .. } => "Text",
                            CustomActionConfig::KeySequence { .. } => "Keys",
                        };
                        ui.label(
                            egui::RichText::new(format!("[{}]", type_label))
                                .monospace()
                                .color(egui::Color32::from_rgb(150, 150, 200)),
                        );

                        // Right-aligned buttons + truncated detail for remaining space
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

                            // Type-specific details (truncated to remaining space)
                            let detail_text = match action {
                                CustomActionConfig::ShellCommand { command, .. } => {
                                    command.to_string()
                                }
                                CustomActionConfig::InsertText { text, .. } => text.clone(),
                                CustomActionConfig::KeySequence { keys, .. } => {
                                    format!("[{}]", keys)
                                }
                            };
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(detail_text)
                                        .monospace()
                                        .color(egui::Color32::GRAY),
                                )
                                .truncate(),
                            );
                        });
                    });
                }
            }

            // Apply mutations after iteration
            if let Some(i) = delete_index {
                settings.config.actions.remove(i);
                settings.has_changes = true;
                *changes_this_frame = true;
                // Reset editing state if we deleted the item being edited
                if settings.editing_action_index == Some(i) {
                    settings.editing_action_index = None;
                    settings.adding_new_action = false;
                }
            }

            if let Some(i) = start_edit_index {
                settings.editing_action_index = Some(i);
                settings.adding_new_action = false;
                // Populate temp fields with current values
                let action = &settings.config.actions[i];
                settings.temp_action_id = action.id().to_string();
                settings.temp_action_title = action.title().to_string();
                settings.temp_action_keybinding =
                    action.keybinding().unwrap_or_default().to_string();
                match action {
                    CustomActionConfig::ShellCommand {
                        command,
                        args,
                        notify_on_success: _,
                        ..
                    } => {
                        settings.temp_action_type = 0;
                        settings.temp_action_command = command.clone();
                        settings.temp_action_args = args.join(" ");
                    }
                    CustomActionConfig::InsertText { text, .. } => {
                        settings.temp_action_type = 1;
                        settings.temp_action_text = text.clone();
                    }
                    CustomActionConfig::KeySequence { keys, .. } => {
                        settings.temp_action_type = 2;
                        settings.temp_action_keys = keys.clone();
                    }
                }
            }

            ui.separator();

            // Add new action button or form
            if settings.adding_new_action {
                show_action_edit_form(ui, settings, changes_this_frame, None);
            } else if ui.button("+ Add Action").clicked() {
                settings.adding_new_action = true;
                settings.editing_action_index = None;
                // Clear temp fields
                settings.temp_action_id = format!("action_{}", uuid::Uuid::new_v4());
                settings.temp_action_title = String::new();
                settings.temp_action_type = 0;
                settings.temp_action_command = String::new();
                settings.temp_action_args = String::new();
                settings.temp_action_text = String::new();
                settings.temp_action_keys = String::new();
                settings.temp_action_keybinding = String::new();
            }
        },
    );
}

/// Show the action edit form (for both new and existing actions).
fn show_action_edit_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    edit_index: Option<usize>,
) {
    ui.separator();

    // Buttons at TOP - always visible first
    ui.horizontal(|ui| {
        if ui.button("Save").clicked() {
            let keybinding = if settings.temp_action_keybinding.is_empty() {
                None
            } else {
                Some(settings.temp_action_keybinding.clone())
            };

            let action = match settings.temp_action_type {
                0 => CustomActionConfig::ShellCommand {
                    id: settings.temp_action_id.clone(),
                    title: settings.temp_action_title.clone(),
                    command: settings.temp_action_command.clone(),
                    args: if settings.temp_action_args.is_empty() {
                        Vec::new()
                    } else {
                        settings
                            .temp_action_args
                            .split_whitespace()
                            .map(|s| s.to_string())
                            .collect()
                    },
                    notify_on_success: false,
                    timeout_secs: 30, // Default timeout
                    keybinding,
                    keybinding_enabled: true,
                    description: None,
                },
                1 => CustomActionConfig::InsertText {
                    id: settings.temp_action_id.clone(),
                    title: settings.temp_action_title.clone(),
                    text: settings.temp_action_text.clone(),
                    variables: std::collections::HashMap::new(),
                    keybinding,
                    keybinding_enabled: true,
                    description: None,
                },
                2 => CustomActionConfig::KeySequence {
                    id: settings.temp_action_id.clone(),
                    title: settings.temp_action_title.clone(),
                    keys: settings.temp_action_keys.clone(),
                    keybinding,
                    keybinding_enabled: true,
                    description: None,
                },
                _ => unreachable!(),
            };

            // Save the action
            if let Some(i) = edit_index {
                // Update existing action
                settings.config.actions[i] = action;
            } else {
                // Add new action
                settings.config.actions.push(action);
            }

            settings.has_changes = true;
            *changes_this_frame = true;
            settings.editing_action_index = None;
            settings.adding_new_action = false;
        }

        if ui.button("Cancel").clicked() {
            settings.editing_action_index = None;
            settings.adding_new_action = false;
            // Also clear recording state if active
            settings.recording_action_keybinding = false;
            settings.action_recorded_combo = None;
        }
    });

    ui.separator();

    // Scrollable area for form fields
    egui::ScrollArea::vertical()
        .max_height(300.0)
        .show(ui, |ui| {
            ui.label("Title:");
            if ui
                .text_edit_singleline(&mut settings.temp_action_title)
                .changed()
            {
                *changes_this_frame = true;
            }

            ui.label("ID:");
            ui.label(
                egui::RichText::new(&settings.temp_action_id)
                    .monospace()
                    .small(),
            );

            ui.label("Type:");
            let types = ["Shell Command", "Insert Text", "Key Sequence"];
            egui::ComboBox::from_id_salt("action_type")
                .selected_text(types[settings.temp_action_type])
                .width(150.0)
                .show_ui(ui, |ui| {
                    for (i, &type_name) in types.iter().enumerate() {
                        if ui
                            .selectable_label(settings.temp_action_type == i, type_name)
                            .clicked()
                        {
                            settings.temp_action_type = i;
                            *changes_this_frame = true;
                        }
                    }
                });

            ui.label("Keybinding:");
            ui.horizontal(|ui| {
                // Check for recording state
                if settings.recording_action_keybinding {
                    // Show recording indicator and capture key combo
                    ui.label(egui::RichText::new("🔴 Recording...").color(egui::Color32::RED));
                    if let Some(combo) = capture_key_combo(ui) {
                        settings.action_recorded_combo = Some(combo.clone());
                        settings.temp_action_keybinding = combo;
                        settings.recording_action_keybinding = false;
                        *changes_this_frame = true;
                    }
                } else {
                    // Show text input and record button
                    if ui
                        .text_edit_singleline(&mut settings.temp_action_keybinding)
                        .changed()
                    {
                        *changes_this_frame = true;
                    }

                    // Check for conflicts
                    if !settings.temp_action_keybinding.is_empty() {
                        let exclude_id = if let Some(i) = edit_index {
                            settings.config.actions.get(i).map(|a| a.id())
                        } else {
                            None
                        };

                        if let Some(conflict) = settings
                            .check_keybinding_conflict(&settings.temp_action_keybinding, exclude_id)
                        {
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
                        settings.recording_action_keybinding = true;
                        settings.action_recorded_combo = None;
                    }
                }
            });

            // Type-specific fields
            match settings.temp_action_type {
                0 => {
                    // Shell Command
                    ui.label("Command:");
                    if ui
                        .text_edit_singleline(&mut settings.temp_action_command)
                        .changed()
                    {
                        *changes_this_frame = true;
                    }
                    ui.label("Arguments (space-separated):");
                    if ui
                        .text_edit_singleline(&mut settings.temp_action_args)
                        .changed()
                    {
                        *changes_this_frame = true;
                    }
                }
                1 => {
                    // Insert Text
                    ui.label("Text to insert:");
                    if ui
                        .text_edit_multiline(&mut settings.temp_action_text)
                        .changed()
                    {
                        *changes_this_frame = true;
                    }
                }
                2 => {
                    // Key Sequence
                    ui.label("Key sequence:");
                    if ui
                        .text_edit_singleline(&mut settings.temp_action_keys)
                        .changed()
                    {
                        *changes_this_frame = true;
                    }
                }
                _ => {}
            }
        });

    ui.separator();
}

/// Search keywords for the Actions settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        "action",
        "actions",
        "custom action",
        "shell command",
        "text insert",
        "key sequence",
        "macro",
        "automation",
        "shortcut",
        // Action details
        "keybinding",
        "binding",
        "record",
        "title",
        "name",
        "arguments",
    ]
}

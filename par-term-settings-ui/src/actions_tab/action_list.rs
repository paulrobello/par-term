//! Action list display, drag-to-reorder, delete, clone, and edit-state population.

use crate::SettingsUI;
use crate::input_tab::capture_key_combo;
use par_term_config::snippets::CustomActionConfig;

use super::action_editor::show_action_edit_form;

/// Show the actions section containing the prefix key config and the action list.
pub fn show_actions_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut std::collections::HashSet<String>,
) {
    crate::section::collapsing_section(
        ui,
        "Custom Actions",
        "actions_list",
        true,
        collapsed,
        |ui| {
            ui.label("Custom actions for shell commands, text insertion, or key sequences.");
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.add_sized(
                    [80.0, ui.spacing().interact_size.y],
                    egui::Label::new(egui::RichText::new("Prefix key:").strong()),
                );

                if settings.recording_custom_action_prefix_key {
                    ui.label(egui::RichText::new("🔴 Recording...").color(egui::Color32::RED));
                    if let Some(combo) = capture_key_combo(ui) {
                        settings.custom_action_prefix_key_recorded_combo = Some(combo.clone());
                        settings.config.custom_action_prefix_key = combo;
                        settings.recording_custom_action_prefix_key = false;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                } else {
                    if ui
                        .text_edit_singleline(&mut settings.config.custom_action_prefix_key)
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }

                    if ui
                        .small_button("🎤")
                        .on_hover_text("Record prefix key")
                        .clicked()
                    {
                        settings.recording_custom_action_prefix_key = true;
                        settings.custom_action_prefix_key_recorded_combo = None;
                    }
                }

                ui.label(
                    egui::RichText::new("Press this first, then a per-action prefix char")
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });

            if !settings.config.custom_action_prefix_key.trim().is_empty() {
                if let Some(conflict) = settings
                    .check_keybinding_conflict(&settings.config.custom_action_prefix_key, None)
                {
                    ui.label(
                        egui::RichText::new(format!("⚠️ {}", conflict))
                            .color(egui::Color32::from_rgb(255, 180, 0))
                            .small(),
                    );
                }

                if settings.config.tmux_enabled
                    && settings.config.custom_action_prefix_key == settings.config.tmux_prefix_key
                {
                    ui.label(
                        egui::RichText::new(
                            "⚠️ Matches the tmux prefix key while tmux integration is enabled",
                        )
                        .color(egui::Color32::from_rgb(255, 180, 0))
                        .small(),
                    );
                }
            }

            ui.add_space(8.0);

            // Collect mutations to apply after iteration
            let mut delete_index: Option<usize> = None;
            let mut clone_index: Option<usize> = None;
            let mut start_edit_index: Option<usize> = None;

            let action_count = settings.config.actions.len();
            if action_count > 0 {
                egui::Frame::group(ui.style())
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        // List existing actions
                        for i in 0..action_count {
                            let action = &settings.config.actions[i];
                            let is_editing =
                                settings.editing_action_index == Some(i) && !settings.adding_new_action;

                            if is_editing {
                                // Show inline edit form for this action
                                show_action_edit_form(ui, settings, changes_this_frame, Some(i));
                            } else {
                                let type_label = match action {
                                    CustomActionConfig::ShellCommand { .. } => "Shell".to_string(),
                                    CustomActionConfig::NewTab { .. } => "NewTab".to_string(),
                                    CustomActionConfig::InsertText { .. } => "Text".to_string(),
                                    CustomActionConfig::KeySequence { .. } => "Keys".to_string(),
                                    CustomActionConfig::SplitPane {
                                        direction,
                                        split_percent,
                                        ..
                                    } => {
                                        let dir = match direction {
                                            par_term_config::snippets::ActionSplitDirection::Horizontal => {
                                                "horiz"
                                            }
                                            par_term_config::snippets::ActionSplitDirection::Vertical => "vert",
                                        };
                                        format!("Split-{}-{}", dir, split_percent)
                                    }
                                    CustomActionConfig::Sequence { steps, .. } => {
                                        format!("Sequence ({} steps)", steps.len())
                                    }
                                    CustomActionConfig::Condition { check, .. } => {
                                        let check_label = match check {
                                            par_term_config::snippets::ConditionCheck::ExitCode { .. } => "exit_code",
                                            par_term_config::snippets::ConditionCheck::OutputContains { .. } => "output_contains",
                                            par_term_config::snippets::ConditionCheck::EnvVar { .. } => "env_var",
                                            par_term_config::snippets::ConditionCheck::DirMatches { .. } => "dir_matches",
                                            par_term_config::snippets::ConditionCheck::GitBranch { .. } => "git_branch",
                                        };
                                        format!("Condition ({})", check_label)
                                    }
                                    CustomActionConfig::Repeat { count, .. } => {
                                        format!("Repeat \u{d7}{}", count)
                                    }
                                };
                                let detail_text = match action {
                                    CustomActionConfig::ShellCommand { command, .. } => {
                                        command.to_string()
                                    }
                                    CustomActionConfig::NewTab { command, .. } => {
                                        command.clone().unwrap_or_default()
                                    }
                                    CustomActionConfig::InsertText { text, .. } => text.clone(),
                                    CustomActionConfig::KeySequence { keys, .. } => {
                                        format!("[{}]", keys)
                                    }
                                    CustomActionConfig::SplitPane { command, .. } => {
                                        command.clone().unwrap_or_default()
                                    }
                                    CustomActionConfig::Sequence { .. } => String::new(),
                                    CustomActionConfig::Condition { .. } => String::new(),
                                    CustomActionConfig::Repeat { action_id, .. } => {
                                        action_id.clone()
                                    }
                                };

                                // Reserve a fixed area for action buttons so the text segment
                                // can't push them outside the visible row.
                                ui.horizontal(|ui| {
                                    let button_area_width = 165.0;
                                    let row_height = ui.spacing().interact_size.y;
                                    let text_area_width =
                                        (ui.available_width() - button_area_width).max(0.0);

                                    ui.allocate_ui_with_layout(
                                        egui::vec2(text_area_width, row_height),
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |ui| {
                                            ui.label(egui::RichText::new(action.title()).strong());
                                            ui.label(
                                                egui::RichText::new(format!("[{}]", type_label))
                                                    .monospace()
                                                    .color(egui::Color32::from_rgb(150, 150, 200)),
                                            );
                                            if let Some(ch) = action.prefix_char() {
                                                ui.label(
                                                    egui::RichText::new(format!("pre:{}", ch))
                                                        .monospace()
                                                        .color(egui::Color32::from_rgb(100, 200, 100)),
                                                );
                                            }

                                            if !detail_text.is_empty() {
                                                ui.add(
                                                    egui::Label::new(
                                                        egui::RichText::new(detail_text)
                                                            .monospace()
                                                            .color(egui::Color32::GRAY),
                                                    )
                                                    .truncate(),
                                                );
                                            }
                                        },
                                    );

                                    ui.allocate_ui_with_layout(
                                        egui::vec2(button_area_width, row_height),
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .small_button(
                                                    egui::RichText::new("Delete")
                                                        .color(egui::Color32::from_rgb(200, 80, 80)),
                                                )
                                                .clicked()
                                            {
                                                delete_index = Some(i);
                                            }

                                            if ui
                                                .small_button("Clone")
                                                .on_hover_text("Duplicate this action")
                                                .clicked()
                                            {
                                                clone_index = Some(i);
                                            }

                                            if ui.small_button("Edit").clicked() {
                                                start_edit_index = Some(i);
                                            }
                                        },
                                    );
                                });
                            }
                        }
                    });
                ui.add_space(6.0);
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

            if let Some(i) = clone_index {
                let cloned = clone_action(&settings.config.actions[i]);
                settings.config.actions.insert(i + 1, cloned);
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if let Some(i) = start_edit_index {
                populate_edit_fields(settings, i);
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
                settings.temp_action_new_tab_command = String::new();
                settings.temp_action_text = String::new();
                settings.temp_action_keys = String::new();
                settings.temp_action_keybinding = String::new();
                settings.temp_action_prefix_char = String::new();
                settings.temp_action_split_direction = 0;
                settings.temp_action_split_command = String::new();
                settings.temp_action_split_command_is_direct = false;
                settings.temp_action_split_focus_new = true;
                settings.temp_action_split_delay_ms = 200;
                settings.temp_action_split_percent = 66;
                settings.temp_action_keybinding_enabled = true;
                settings.temp_action_steps = Vec::new();
                settings.temp_action_check_type = 0;
                settings.temp_action_check_value = String::new();
                settings.temp_action_case_sensitive = false;
                settings.temp_action_env_name = String::new();
                settings.temp_action_env_value = String::new();
                settings.temp_action_env_check_existence = false;
                settings.temp_action_on_true_id = String::new();
                settings.temp_action_on_false_id = String::new();
                settings.temp_action_repeat_action_id = String::new();
                settings.temp_action_repeat_count = 3;
                settings.temp_action_repeat_delay_ms = 0;
                settings.temp_action_stop_on_success = false;
                settings.temp_action_stop_on_failure = false;
                settings.temp_action_capture_output = false;
            }
        },
    );
}

/// Populate temporary edit fields from an existing action at `index`.
fn populate_edit_fields(settings: &mut SettingsUI, index: usize) {
    settings.editing_action_index = Some(index);
    settings.adding_new_action = false;
    // Populate temp fields with current values
    let action = &settings.config.actions[index];
    settings.temp_action_id = action.id().to_string();
    settings.temp_action_title = action.title().to_string();
    settings.temp_action_keybinding =
        action.keybinding().unwrap_or_default().to_string();
    settings.temp_action_prefix_char = action
        .prefix_char()
        .map(|c| c.to_string())
        .unwrap_or_default();
    match action {
        CustomActionConfig::ShellCommand {
            command,
            args,
            capture_output,
            notify_on_success: _,
            ..
        } => {
            settings.temp_action_type = 0;
            settings.temp_action_command = command.clone();
            settings.temp_action_args = args.join(" ");
            settings.temp_action_capture_output = *capture_output;
        }
        CustomActionConfig::NewTab { command, .. } => {
            settings.temp_action_type = 1;
            settings.temp_action_new_tab_command = command.clone().unwrap_or_default();
        }
        CustomActionConfig::InsertText { text, .. } => {
            settings.temp_action_type = 2;
            settings.temp_action_text = text.clone();
        }
        CustomActionConfig::KeySequence { keys, .. } => {
            settings.temp_action_type = 3;
            settings.temp_action_keys = keys.clone();
        }
        CustomActionConfig::SplitPane {
            direction,
            command,
            command_is_direct,
            focus_new_pane,
            delay_ms,
            split_percent,
            ..
        } => {
            settings.temp_action_type = 4;
            settings.temp_action_split_direction = match direction {
                par_term_config::snippets::ActionSplitDirection::Horizontal => 0,
                par_term_config::snippets::ActionSplitDirection::Vertical => 1,
            };
            settings.temp_action_split_command = command.clone().unwrap_or_default();
            settings.temp_action_split_command_is_direct = *command_is_direct;
            settings.temp_action_split_focus_new = *focus_new_pane;
            settings.temp_action_split_delay_ms = *delay_ms;
            settings.temp_action_split_percent = *split_percent;
        }
        CustomActionConfig::Sequence {
            steps,
            keybinding_enabled,
            ..
        } => {
            settings.temp_action_type = 5;
            settings.temp_action_keybinding_enabled = *keybinding_enabled;
            settings.temp_action_steps = steps
                .iter()
                .map(|s| (s.action_id.clone(), s.delay_ms, s.on_failure))
                .collect();
            // Reset non-sequence fields
            settings.temp_action_check_type = 0;
            settings.temp_action_check_value = String::new();
            settings.temp_action_case_sensitive = false;
            settings.temp_action_env_name = String::new();
            settings.temp_action_env_value = String::new();
            settings.temp_action_env_check_existence = false;
            settings.temp_action_on_true_id = String::new();
            settings.temp_action_on_false_id = String::new();
            settings.temp_action_repeat_action_id = String::new();
            settings.temp_action_repeat_count = 3;
            settings.temp_action_repeat_delay_ms = 0;
            settings.temp_action_stop_on_success = false;
            settings.temp_action_stop_on_failure = false;
        }
        CustomActionConfig::Condition {
            check,
            on_true_id,
            on_false_id,
            keybinding_enabled,
            ..
        } => {
            settings.temp_action_type = 6;
            settings.temp_action_keybinding_enabled = *keybinding_enabled;
            match check {
                par_term_config::snippets::ConditionCheck::ExitCode { value } => {
                    settings.temp_action_check_type = 0;
                    settings.temp_action_check_value = value.to_string();
                    settings.temp_action_case_sensitive = false;
                }
                par_term_config::snippets::ConditionCheck::OutputContains {
                    pattern,
                    case_sensitive,
                } => {
                    settings.temp_action_check_type = 1;
                    settings.temp_action_check_value = pattern.clone();
                    settings.temp_action_case_sensitive = *case_sensitive;
                }
                par_term_config::snippets::ConditionCheck::EnvVar { name, value } => {
                    settings.temp_action_check_type = 2;
                    settings.temp_action_env_name = name.clone();
                    settings.temp_action_env_check_existence = value.is_none();
                    settings.temp_action_env_value = value.clone().unwrap_or_default();
                }
                par_term_config::snippets::ConditionCheck::DirMatches { pattern } => {
                    settings.temp_action_check_type = 3;
                    settings.temp_action_check_value = pattern.clone();
                }
                par_term_config::snippets::ConditionCheck::GitBranch { pattern } => {
                    settings.temp_action_check_type = 4;
                    settings.temp_action_check_value = pattern.clone();
                }
            }
            settings.temp_action_on_true_id = on_true_id.clone().unwrap_or_default();
            settings.temp_action_on_false_id = on_false_id.clone().unwrap_or_default();
            // Reset non-condition fields
            settings.temp_action_steps = Vec::new();
            settings.temp_action_env_name = if settings.temp_action_check_type != 2 {
                String::new()
            } else {
                settings.temp_action_env_name.clone()
            };
            settings.temp_action_repeat_action_id = String::new();
            settings.temp_action_repeat_count = 3;
            settings.temp_action_repeat_delay_ms = 0;
            settings.temp_action_stop_on_success = false;
            settings.temp_action_stop_on_failure = false;
        }
        CustomActionConfig::Repeat {
            action_id,
            count,
            delay_ms,
            stop_on_success,
            stop_on_failure,
            keybinding_enabled,
            ..
        } => {
            settings.temp_action_type = 7;
            settings.temp_action_keybinding_enabled = *keybinding_enabled;
            settings.temp_action_repeat_action_id = action_id.clone();
            settings.temp_action_repeat_count = *count;
            settings.temp_action_repeat_delay_ms = *delay_ms;
            settings.temp_action_stop_on_success = *stop_on_success;
            settings.temp_action_stop_on_failure = *stop_on_failure;
            // Reset non-repeat fields
            settings.temp_action_steps = Vec::new();
            settings.temp_action_check_type = 0;
            settings.temp_action_check_value = String::new();
            settings.temp_action_case_sensitive = false;
            settings.temp_action_env_name = String::new();
            settings.temp_action_env_value = String::new();
            settings.temp_action_env_check_existence = false;
            settings.temp_action_on_true_id = String::new();
            settings.temp_action_on_false_id = String::new();
        }
    }
}

/// Create a duplicate of `action` with a fresh id, title suffixed with "-copy",
/// and keybinding/prefix_char cleared to avoid immediate conflicts.
pub fn clone_action(action: &CustomActionConfig) -> CustomActionConfig {
    let new_id = format!("action_{}", uuid::Uuid::new_v4());
    match action {
        CustomActionConfig::ShellCommand {
            title,
            command,
            args,
            notify_on_success,
            timeout_secs,
            capture_output,
            keybinding_enabled,
            description,
            ..
        } => CustomActionConfig::ShellCommand {
            id: new_id,
            title: format!("{}-copy", title),
            command: command.clone(),
            args: args.clone(),
            notify_on_success: *notify_on_success,
            timeout_secs: *timeout_secs,
            capture_output: *capture_output,
            keybinding: None,
            prefix_char: None,
            keybinding_enabled: *keybinding_enabled,
            description: description.clone(),
        },
        CustomActionConfig::NewTab {
            title,
            command,
            keybinding_enabled,
            description,
            ..
        } => CustomActionConfig::NewTab {
            id: new_id,
            title: format!("{}-copy", title),
            command: command.clone(),
            keybinding: None,
            prefix_char: None,
            keybinding_enabled: *keybinding_enabled,
            description: description.clone(),
        },
        CustomActionConfig::InsertText {
            title,
            text,
            variables,
            keybinding_enabled,
            description,
            ..
        } => CustomActionConfig::InsertText {
            id: new_id,
            title: format!("{}-copy", title),
            text: text.clone(),
            variables: variables.clone(),
            keybinding: None,
            prefix_char: None,
            keybinding_enabled: *keybinding_enabled,
            description: description.clone(),
        },
        CustomActionConfig::KeySequence {
            title,
            keys,
            keybinding_enabled,
            description,
            ..
        } => CustomActionConfig::KeySequence {
            id: new_id,
            title: format!("{}-copy", title),
            keys: keys.clone(),
            keybinding: None,
            prefix_char: None,
            keybinding_enabled: *keybinding_enabled,
            description: description.clone(),
        },
        CustomActionConfig::SplitPane {
            title,
            direction,
            command,
            command_is_direct,
            focus_new_pane,
            delay_ms,
            split_percent,
            keybinding_enabled,
            description,
            ..
        } => CustomActionConfig::SplitPane {
            id: new_id,
            title: format!("{}-copy", title),
            direction: *direction,
            command: command.clone(),
            command_is_direct: *command_is_direct,
            focus_new_pane: *focus_new_pane,
            delay_ms: *delay_ms,
            split_percent: *split_percent,
            keybinding: None,
            prefix_char: None,
            keybinding_enabled: *keybinding_enabled,
            description: description.clone(),
        },
        CustomActionConfig::Sequence {
            title,
            keybinding_enabled,
            description,
            steps,
            ..
        } => CustomActionConfig::Sequence {
            id: new_id,
            title: format!("{}-copy", title),
            keybinding: None,
            prefix_char: None,
            keybinding_enabled: *keybinding_enabled,
            description: description.clone(),
            steps: steps.clone(),
        },
        CustomActionConfig::Condition {
            title,
            keybinding_enabled,
            description,
            check,
            on_true_id,
            on_false_id,
            ..
        } => CustomActionConfig::Condition {
            id: new_id,
            title: format!("{}-copy", title),
            keybinding: None,
            prefix_char: None,
            keybinding_enabled: *keybinding_enabled,
            description: description.clone(),
            check: check.clone(),
            on_true_id: on_true_id.clone(),
            on_false_id: on_false_id.clone(),
        },
        CustomActionConfig::Repeat {
            title,
            keybinding_enabled,
            description,
            action_id,
            count,
            delay_ms,
            stop_on_success,
            stop_on_failure,
            ..
        } => CustomActionConfig::Repeat {
            id: new_id,
            title: format!("{}-copy", title),
            keybinding: None,
            prefix_char: None,
            keybinding_enabled: *keybinding_enabled,
            description: description.clone(),
            action_id: action_id.clone(),
            count: *count,
            delay_ms: *delay_ms,
            stop_on_success: *stop_on_success,
            stop_on_failure: *stop_on_failure,
        },
    }
}

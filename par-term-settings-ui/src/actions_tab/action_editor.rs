//! Action editor form — handles both new action creation and existing action editing.
//!
//! Contains save/cancel logic, common fields (title, type, keybinding, prefix char),
//! and delegates type-specific fields to `action_forms`.

use crate::SettingsUI;
use crate::input_tab::capture_key_combo;
use par_term_config::snippets::CustomActionConfig;

use super::action_forms::*;

/// Show the action edit form (for both new and existing actions).
pub fn show_action_edit_form(
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
            let prefix_char = settings.temp_action_prefix_char.chars().next();

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
                    capture_output: settings.temp_action_capture_output,
                    keybinding,
                    prefix_char,
                    keybinding_enabled: settings.temp_action_keybinding_enabled,
                    description: None,
                },
                1 => CustomActionConfig::NewTab {
                    id: settings.temp_action_id.clone(),
                    title: settings.temp_action_title.clone(),
                    command: if settings.temp_action_new_tab_command.is_empty() {
                        None
                    } else {
                        Some(settings.temp_action_new_tab_command.clone())
                    },
                    keybinding,
                    prefix_char,
                    keybinding_enabled: true,
                    description: None,
                },
                2 => CustomActionConfig::InsertText {
                    id: settings.temp_action_id.clone(),
                    title: settings.temp_action_title.clone(),
                    text: settings.temp_action_text.clone(),
                    variables: std::collections::HashMap::new(),
                    keybinding,
                    prefix_char,
                    keybinding_enabled: true,
                    description: None,
                },
                3 => CustomActionConfig::KeySequence {
                    id: settings.temp_action_id.clone(),
                    title: settings.temp_action_title.clone(),
                    keys: settings.temp_action_keys.clone(),
                    keybinding,
                    prefix_char,
                    keybinding_enabled: true,
                    description: None,
                },
                4 => CustomActionConfig::SplitPane {
                    id: settings.temp_action_id.clone(),
                    title: settings.temp_action_title.clone(),
                    direction: if settings.temp_action_split_direction == 0 {
                        par_term_config::snippets::ActionSplitDirection::Horizontal
                    } else {
                        par_term_config::snippets::ActionSplitDirection::Vertical
                    },
                    command: if settings.temp_action_split_command.is_empty() {
                        None
                    } else {
                        Some(settings.temp_action_split_command.clone())
                    },
                    command_is_direct: settings.temp_action_split_command_is_direct,
                    focus_new_pane: settings.temp_action_split_focus_new,
                    delay_ms: settings.temp_action_split_delay_ms,
                    split_percent: settings.temp_action_split_percent,
                    keybinding,
                    prefix_char,
                    keybinding_enabled: true,
                    description: None,
                },
                5 => {
                    let steps = settings
                        .temp_action_steps
                        .iter()
                        .map(
                            |(id, delay, behavior)| par_term_config::snippets::SequenceStep {
                                action_id: id.clone(),
                                delay_ms: *delay,
                                on_failure: *behavior,
                            },
                        )
                        .collect();
                    CustomActionConfig::Sequence {
                        id: settings.temp_action_id.clone(),
                        title: settings.temp_action_title.clone(),
                        keybinding,
                        prefix_char,
                        keybinding_enabled: settings.temp_action_keybinding_enabled,
                        description: None,
                        steps,
                    }
                }
                6 => {
                    use par_term_config::snippets::ConditionCheck;
                    let check = match settings.temp_action_check_type {
                        0 => ConditionCheck::ExitCode {
                            value: settings.temp_action_check_value.parse().unwrap_or(0),
                        },
                        1 => ConditionCheck::OutputContains {
                            pattern: settings.temp_action_check_value.clone(),
                            case_sensitive: settings.temp_action_case_sensitive,
                        },
                        2 => ConditionCheck::EnvVar {
                            name: settings.temp_action_env_name.clone(),
                            value: if settings.temp_action_env_check_existence {
                                None
                            } else {
                                Some(settings.temp_action_env_value.clone())
                            },
                        },
                        3 => ConditionCheck::DirMatches {
                            pattern: settings.temp_action_check_value.clone(),
                        },
                        4 => ConditionCheck::GitBranch {
                            pattern: settings.temp_action_check_value.clone(),
                        },
                        _ => ConditionCheck::ExitCode { value: 0 },
                    };
                    CustomActionConfig::Condition {
                        id: settings.temp_action_id.clone(),
                        title: settings.temp_action_title.clone(),
                        keybinding,
                        prefix_char,
                        keybinding_enabled: settings.temp_action_keybinding_enabled,
                        description: None,
                        check,
                        on_true_id: if settings.temp_action_on_true_id.is_empty() {
                            None
                        } else {
                            Some(settings.temp_action_on_true_id.clone())
                        },
                        on_false_id: if settings.temp_action_on_false_id.is_empty() {
                            None
                        } else {
                            Some(settings.temp_action_on_false_id.clone())
                        },
                    }
                }
                7 => CustomActionConfig::Repeat {
                    id: settings.temp_action_id.clone(),
                    title: settings.temp_action_title.clone(),
                    keybinding,
                    prefix_char,
                    keybinding_enabled: settings.temp_action_keybinding_enabled,
                    description: None,
                    action_id: settings.temp_action_repeat_action_id.clone(),
                    count: settings.temp_action_repeat_count.clamp(1, 100),
                    delay_ms: settings.temp_action_repeat_delay_ms,
                    stop_on_success: settings.temp_action_stop_on_success,
                    stop_on_failure: settings.temp_action_stop_on_failure,
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
            let types = [
                "Shell Command",
                "New Tab",
                "Insert Text",
                "Key Sequence",
                "Split Pane",
                "Sequence",
                "Condition",
                "Repeat",
            ];
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

            // Conflict warning — shown below the keybinding row so it doesn't push the record button off-screen
            if !settings.recording_action_keybinding && !settings.temp_action_keybinding.is_empty()
            {
                let exclude_id = if let Some(i) = edit_index {
                    settings.config.actions.get(i).map(|a| a.id())
                } else {
                    None
                };
                if let Some(conflict) =
                    settings.check_keybinding_conflict(&settings.temp_action_keybinding, exclude_id)
                {
                    ui.label(
                        egui::RichText::new(format!("⚠️ {}", conflict))
                            .color(egui::Color32::from_rgb(255, 180, 0))
                            .small(),
                    );
                }
            }

            ui.label("Prefix char:");
            if ui
                .text_edit_singleline(&mut settings.temp_action_prefix_char)
                .changed()
            {
                settings.temp_action_prefix_char = settings
                    .temp_action_prefix_char
                    .chars()
                    .find(|ch| !ch.is_whitespace())
                    .map(|ch| ch.to_string())
                    .unwrap_or_default();
                *changes_this_frame = true;
            }

            if let Some(prefix_char) = settings.temp_action_prefix_char.chars().next() {
                let exclude_id = if let Some(i) = edit_index {
                    settings.config.actions.get(i).map(|a| a.id())
                } else {
                    None
                };

                if let Some(conflict) =
                    settings.check_action_prefix_char_conflict(prefix_char, exclude_id)
                {
                    ui.label(
                        egui::RichText::new(format!("⚠️ {}", conflict))
                            .color(egui::Color32::from_rgb(255, 180, 0))
                            .small(),
                    );
                }
            }

            // Type-specific fields — each action type is rendered by a dedicated helper.
            match settings.temp_action_type {
                0 => show_shell_command_form(ui, settings, changes_this_frame),
                1 => show_new_tab_form(ui, settings, changes_this_frame),
                2 => show_insert_text_form(ui, settings, changes_this_frame),
                3 => show_key_sequence_form(ui, settings, changes_this_frame),
                4 => show_split_pane_form(ui, settings, changes_this_frame),
                5 => show_sequence_form(ui, settings, changes_this_frame),
                6 => show_condition_form(ui, settings, changes_this_frame),
                7 => show_repeat_form(ui, settings, changes_this_frame),
                _ => {}
            }
        });

    ui.separator();
}

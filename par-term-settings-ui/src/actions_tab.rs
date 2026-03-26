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
            "prefix",
            "execute",
            "run",
            "split",
            "pane",
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
                settings.editing_action_index = Some(i);
                settings.adding_new_action = false;
                // Populate temp fields with current values
                let action = &settings.config.actions[i];
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

// ---------------------------------------------------------------------------
// Per-action-type form helpers (extracted from show_action_edit_form — QA-007)
// ---------------------------------------------------------------------------

/// Render form fields for the ShellCommand action type.
fn show_shell_command_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
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
    if ui
        .checkbox(
            &mut settings.temp_action_capture_output,
            "Capture output (makes exit code and stdout available to Condition checks)",
        )
        .changed()
    {
        *changes_this_frame = true;
    }
}

/// Render form fields for the NewTab action type.
fn show_new_tab_form(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.label("Command to run in the new tab (optional):");
    if ui
        .add(
            egui::TextEdit::multiline(&mut settings.temp_action_new_tab_command)
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        )
        .changed()
    {
        *changes_this_frame = true;
    }
    ui.label(
        egui::RichText::new("Leave empty to open a normal shell tab with no startup command.")
            .small()
            .color(egui::Color32::GRAY),
    );
}

/// Render form fields for the InsertText action type.
fn show_insert_text_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    ui.label("Text to insert:");
    if ui
        .text_edit_multiline(&mut settings.temp_action_text)
        .changed()
    {
        *changes_this_frame = true;
    }
}

/// Render form fields for the KeySequence action type.
fn show_key_sequence_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    ui.label("Key sequence:");
    if ui
        .text_edit_singleline(&mut settings.temp_action_keys)
        .changed()
    {
        *changes_this_frame = true;
    }
}

/// Render form fields for the SplitPane action type.
fn show_split_pane_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    ui.label("Direction:");
    let dir_labels = ["Horizontal (below)", "Vertical (right)"];
    egui::ComboBox::from_id_salt("split_direction")
        .selected_text(dir_labels[settings.temp_action_split_direction])
        .width(160.0)
        .show_ui(ui, |ui| {
            for (i, &label) in dir_labels.iter().enumerate() {
                if ui
                    .selectable_label(settings.temp_action_split_direction == i, label)
                    .clicked()
                {
                    settings.temp_action_split_direction = i;
                    *changes_this_frame = true;
                }
            }
        });

    ui.label("Command (optional):");
    if ui
        .text_edit_singleline(&mut settings.temp_action_split_command)
        .changed()
    {
        *changes_this_frame = true;
    }

    if !settings.temp_action_split_command.is_empty() {
        ui.horizontal(|ui| {
            if ui
                .checkbox(
                    &mut settings.temp_action_split_command_is_direct,
                    "Run as pane command (pane closes when done)",
                )
                .on_hover_text(
                    "When checked, the command is the pane's initial process \
                     (like running htop directly). The pane closes when it exits.\n\
                     When unchecked, the command is sent as text to the shell.",
                )
                .changed()
            {
                *changes_this_frame = true;
            }
        });
    }

    ui.horizontal(|ui| {
        if ui
            .checkbox(&mut settings.temp_action_split_focus_new, "Focus new pane")
            .changed()
        {
            *changes_this_frame = true;
        }
    });

    ui.horizontal(|ui| {
        ui.label("Split percent (existing pane):");
        let mut pct = settings.temp_action_split_percent as u32;
        if ui
            .add(egui::DragValue::new(&mut pct).range(10..=90).suffix("%"))
            .on_hover_text(
                "Percentage of the current pane that the existing pane retains.\n\
                 The new pane receives the remainder.\n\
                 Default: 66% (existing keeps 2/3, new gets 1/3).",
            )
            .changed()
        {
            settings.temp_action_split_percent = pct as u8;
            *changes_this_frame = true;
        }
    });

    if !settings.temp_action_split_command.is_empty()
        && !settings.temp_action_split_command_is_direct
    {
        ui.horizontal(|ui| {
            ui.label("Command delay (ms):");
            let mut delay_str = settings.temp_action_split_delay_ms.to_string();
            if ui.text_edit_singleline(&mut delay_str).changed() {
                if let Ok(v) = delay_str.parse::<u64>() {
                    settings.temp_action_split_delay_ms = v;
                }
                *changes_this_frame = true;
            }
        });
    }
}

/// Render form fields for the Sequence action type.
fn show_sequence_form(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.label(egui::RichText::new("Steps:").strong());
    let action_ids: Vec<(String, String)> = settings
        .config
        .actions
        .iter()
        .map(|a| (a.id().to_string(), a.title().to_string()))
        .collect();
    let mut step_to_delete: Option<usize> = None;
    let mut step_to_move_up: Option<usize> = None;
    let mut step_to_move_down: Option<usize> = None;
    let step_count = settings.temp_action_steps.len();
    for step_idx in 0..step_count {
        ui.horizontal(|ui| {
            ui.label(format!("{}.", step_idx + 1));
            let current_id = settings.temp_action_steps[step_idx].0.clone();
            let display_text = action_ids
                .iter()
                .find(|(id, _)| id == &current_id)
                .map(|(_, title)| format!("{} ({})", title, current_id))
                .unwrap_or_else(|| {
                    if current_id.is_empty() {
                        "(none)".to_string()
                    } else {
                        current_id.clone()
                    }
                });
            egui::ComboBox::from_id_salt(format!("seq_step_action_{}", step_idx))
                .selected_text(display_text)
                .width(200.0)
                .show_ui(ui, |ui| {
                    for (id, title) in &action_ids {
                        let label = format!("{} ({})", title, id);
                        if ui
                            .selectable_label(settings.temp_action_steps[step_idx].0 == *id, &label)
                            .clicked()
                        {
                            settings.temp_action_steps[step_idx].0 = id.clone();
                            *changes_this_frame = true;
                        }
                    }
                });
            ui.label("delay ms:");
            let mut delay = settings.temp_action_steps[step_idx].1;
            if ui
                .add(egui::DragValue::new(&mut delay).range(0..=60000))
                .changed()
            {
                settings.temp_action_steps[step_idx].1 = delay;
                *changes_this_frame = true;
            }
            let behavior_labels = ["Abort", "Stop", "Continue"];
            let cur_behavior = settings.temp_action_steps[step_idx].2;
            let cur_behavior_idx = match cur_behavior {
                par_term_config::snippets::SequenceStepBehavior::Abort => 0,
                par_term_config::snippets::SequenceStepBehavior::Stop => 1,
                par_term_config::snippets::SequenceStepBehavior::Continue => 2,
            };
            egui::ComboBox::from_id_salt(format!("seq_step_fail_{}", step_idx))
                .selected_text(behavior_labels[cur_behavior_idx])
                .width(90.0)
                .show_ui(ui, |ui| {
                    for (i, &label) in behavior_labels.iter().enumerate() {
                        let behavior = match i {
                            0 => par_term_config::snippets::SequenceStepBehavior::Abort,
                            1 => par_term_config::snippets::SequenceStepBehavior::Stop,
                            _ => par_term_config::snippets::SequenceStepBehavior::Continue,
                        };
                        if ui.selectable_label(cur_behavior_idx == i, label).clicked() {
                            settings.temp_action_steps[step_idx].2 = behavior;
                            *changes_this_frame = true;
                        }
                    }
                });
            if step_idx > 0
                && ui
                    .small_button("\u{f062}")
                    .on_hover_text("Move up")
                    .clicked()
            {
                step_to_move_up = Some(step_idx);
            }
            if step_idx + 1 < step_count
                && ui
                    .small_button("\u{f063}")
                    .on_hover_text("Move down")
                    .clicked()
            {
                step_to_move_down = Some(step_idx);
            }
            if ui
                .small_button(
                    egui::RichText::new("\u{f00d}").color(egui::Color32::from_rgb(200, 80, 80)),
                )
                .on_hover_text("Remove step")
                .clicked()
            {
                step_to_delete = Some(step_idx);
            }
        });
    }
    if let Some(i) = step_to_delete {
        settings.temp_action_steps.remove(i);
        *changes_this_frame = true;
    } else if let Some(i) = step_to_move_up {
        settings.temp_action_steps.swap(i - 1, i);
        *changes_this_frame = true;
    } else if let Some(i) = step_to_move_down {
        settings.temp_action_steps.swap(i, i + 1);
        *changes_this_frame = true;
    }
    if ui.button("+ Add Step").clicked() {
        settings.temp_action_steps.push((
            String::new(),
            0u64,
            par_term_config::snippets::SequenceStepBehavior::Abort,
        ));
        *changes_this_frame = true;
    }
}

/// Render form fields for the Condition action type.
fn show_condition_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    let check_labels = [
        "exit_code",
        "output_contains",
        "env_var",
        "dir_matches",
        "git_branch",
    ];
    ui.horizontal(|ui| {
        ui.label("Check type:");
        egui::ComboBox::from_id_salt("condition_check_type")
            .selected_text(check_labels[settings.temp_action_check_type.min(4)])
            .width(150.0)
            .show_ui(ui, |ui| {
                for (i, &label) in check_labels.iter().enumerate() {
                    if ui
                        .selectable_label(settings.temp_action_check_type == i, label)
                        .clicked()
                    {
                        settings.temp_action_check_type = i;
                        *changes_this_frame = true;
                    }
                }
            });
    });
    match settings.temp_action_check_type {
        0 => {
            ui.horizontal(|ui| {
                ui.label("Exit code:");
                if ui
                    .text_edit_singleline(&mut settings.temp_action_check_value)
                    .changed()
                {
                    *changes_this_frame = true;
                }
            });
        }
        1 => {
            ui.label("Pattern:");
            if ui
                .text_edit_singleline(&mut settings.temp_action_check_value)
                .changed()
            {
                *changes_this_frame = true;
            }
            ui.horizontal(|ui| {
                if ui
                    .checkbox(&mut settings.temp_action_case_sensitive, "Case sensitive")
                    .changed()
                {
                    *changes_this_frame = true;
                }
            });
        }
        2 => {
            ui.horizontal(|ui| {
                ui.label("Var name:");
                if ui
                    .text_edit_singleline(&mut settings.temp_action_env_name)
                    .changed()
                {
                    *changes_this_frame = true;
                }
            });
            ui.horizontal(|ui| {
                if ui
                    .checkbox(
                        &mut settings.temp_action_env_check_existence,
                        "Check existence only",
                    )
                    .changed()
                {
                    *changes_this_frame = true;
                }
            });
            if !settings.temp_action_env_check_existence {
                ui.horizontal(|ui| {
                    ui.label("Expected value:");
                    if ui
                        .text_edit_singleline(&mut settings.temp_action_env_value)
                        .changed()
                    {
                        *changes_this_frame = true;
                    }
                });
            }
        }
        3 => {
            ui.label("Pattern (glob):");
            if ui
                .text_edit_singleline(&mut settings.temp_action_check_value)
                .on_hover_text("e.g. /home/user/projects/*")
                .changed()
            {
                *changes_this_frame = true;
            }
        }
        4 => {
            ui.label("Pattern (glob):");
            if ui
                .text_edit_singleline(&mut settings.temp_action_check_value)
                .on_hover_text("e.g. feature/*")
                .changed()
            {
                *changes_this_frame = true;
            }
        }
        _ => {}
    }

    let action_ids: Vec<(String, String)> = settings
        .config
        .actions
        .iter()
        .map(|a| (a.id().to_string(), a.title().to_string()))
        .collect();

    ui.horizontal(|ui| {
        ui.label("On True:");
        let true_display = if settings.temp_action_on_true_id.is_empty() {
            "(none)".to_string()
        } else {
            action_ids
                .iter()
                .find(|(id, _)| id == &settings.temp_action_on_true_id)
                .map(|(_, title)| format!("{} ({})", title, settings.temp_action_on_true_id))
                .unwrap_or_else(|| settings.temp_action_on_true_id.clone())
        };
        egui::ComboBox::from_id_salt("condition_on_true")
            .selected_text(true_display)
            .width(200.0)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(settings.temp_action_on_true_id.is_empty(), "(none)")
                    .clicked()
                {
                    settings.temp_action_on_true_id = String::new();
                    *changes_this_frame = true;
                }
                for (id, title) in &action_ids {
                    let label = format!("{} ({})", title, id);
                    if ui
                        .selectable_label(settings.temp_action_on_true_id == *id, &label)
                        .clicked()
                    {
                        settings.temp_action_on_true_id = id.clone();
                        *changes_this_frame = true;
                    }
                }
            });
        ui.label(
            egui::RichText::new("(standalone only)")
                .small()
                .color(egui::Color32::GRAY),
        );
    });
    ui.horizontal(|ui| {
        ui.label("On False:");
        let false_display = if settings.temp_action_on_false_id.is_empty() {
            "(none)".to_string()
        } else {
            action_ids
                .iter()
                .find(|(id, _)| id == &settings.temp_action_on_false_id)
                .map(|(_, title)| format!("{} ({})", title, settings.temp_action_on_false_id))
                .unwrap_or_else(|| settings.temp_action_on_false_id.clone())
        };
        egui::ComboBox::from_id_salt("condition_on_false")
            .selected_text(false_display)
            .width(200.0)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(settings.temp_action_on_false_id.is_empty(), "(none)")
                    .clicked()
                {
                    settings.temp_action_on_false_id = String::new();
                    *changes_this_frame = true;
                }
                for (id, title) in &action_ids {
                    let label = format!("{} ({})", title, id);
                    if ui
                        .selectable_label(settings.temp_action_on_false_id == *id, &label)
                        .clicked()
                    {
                        settings.temp_action_on_false_id = id.clone();
                        *changes_this_frame = true;
                    }
                }
            });
        ui.label(
            egui::RichText::new("(standalone only)")
                .small()
                .color(egui::Color32::GRAY),
        );
    });
}

/// Render form fields for the Repeat action type.
fn show_repeat_form(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    let action_ids: Vec<(String, String)> = settings
        .config
        .actions
        .iter()
        .map(|a| (a.id().to_string(), a.title().to_string()))
        .collect();
    ui.horizontal(|ui| {
        ui.label("Action:");
        let repeat_display = if settings.temp_action_repeat_action_id.is_empty() {
            "(none)".to_string()
        } else {
            action_ids
                .iter()
                .find(|(id, _)| id == &settings.temp_action_repeat_action_id)
                .map(|(_, title)| format!("{} ({})", title, settings.temp_action_repeat_action_id))
                .unwrap_or_else(|| settings.temp_action_repeat_action_id.clone())
        };
        egui::ComboBox::from_id_salt("repeat_action_id")
            .selected_text(repeat_display)
            .width(200.0)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(settings.temp_action_repeat_action_id.is_empty(), "(none)")
                    .clicked()
                {
                    settings.temp_action_repeat_action_id = String::new();
                    *changes_this_frame = true;
                }
                for (id, title) in &action_ids {
                    let label = format!("{} ({})", title, id);
                    if ui
                        .selectable_label(settings.temp_action_repeat_action_id == *id, &label)
                        .clicked()
                    {
                        settings.temp_action_repeat_action_id = id.clone();
                        *changes_this_frame = true;
                    }
                }
            });
    });
    ui.horizontal(|ui| {
        ui.label("Count (1-100):");
        if ui
            .add(egui::DragValue::new(&mut settings.temp_action_repeat_count).range(1..=100))
            .changed()
        {
            *changes_this_frame = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label("Delay between (ms):");
        if ui
            .add(egui::DragValue::new(&mut settings.temp_action_repeat_delay_ms).range(0..=60000))
            .changed()
        {
            *changes_this_frame = true;
        }
    });
    ui.horizontal(|ui| {
        if ui
            .checkbox(&mut settings.temp_action_stop_on_success, "Stop on success")
            .changed()
        {
            *changes_this_frame = true;
        }
    });
    ui.horizontal(|ui| {
        if ui
            .checkbox(&mut settings.temp_action_stop_on_failure, "Stop on failure")
            .changed()
        {
            *changes_this_frame = true;
        }
    });
}

/// Create a duplicate of `action` with a fresh id, title suffixed with "-copy",
/// and keybinding/prefix_char cleared to avoid immediate conflicts.
fn clone_action(action: &CustomActionConfig) -> CustomActionConfig {
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

/// Search keywords for the Actions settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        "action",
        "actions",
        "custom action",
        "shell command",
        "new tab",
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
        "split",
        "split pane",
        "pane",
        "horizontal",
        "vertical",
        // Workflow action types
        "workflow",
        "sequence",
        "condition",
        "repeat",
        "capture output",
        "capture_output",
        "exit code",
        "step",
    ]
}

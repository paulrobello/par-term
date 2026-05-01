//! Per-action-type form helpers.
//!
//! Each function renders the type-specific fields for one action variant.

use crate::SettingsUI;

// ---------------------------------------------------------------------------
// Per-action-type form helpers (extracted from show_action_edit_form)
// ---------------------------------------------------------------------------

/// Render form fields for the ShellCommand action type.
pub fn show_shell_command_form(
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
pub fn show_new_tab_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
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
pub fn show_insert_text_form(
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
pub fn show_key_sequence_form(
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
pub fn show_split_pane_form(
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
pub fn show_sequence_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
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
pub fn show_condition_form(
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
pub fn show_repeat_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
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

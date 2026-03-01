//! Triggers section of the automation settings tab.

use crate::SettingsUI;
use crate::section::{collapsing_section, section_matches};
use par_term_config::automation::{TriggerActionConfig, TriggerConfig};
use par_term_config::color_u8_to_f32;
use std::collections::HashSet;

/// Return a human-readable label for a trigger action variant.
fn action_type_label(action: &TriggerActionConfig) -> &'static str {
    match action {
        TriggerActionConfig::Highlight { .. } => "Highlight",
        TriggerActionConfig::Notify { .. } => "Notify",
        TriggerActionConfig::MarkLine { .. } => "Mark Line",
        TriggerActionConfig::SetVariable { .. } => "Set Variable",
        TriggerActionConfig::RunCommand { .. } => "Run Command",
        TriggerActionConfig::PlaySound { .. } => "Play Sound",
        TriggerActionConfig::SendText { .. } => "Send Text",
        TriggerActionConfig::Prettify { .. } => "Prettify",
    }
}

/// Create a default action for the given type index.
fn default_action_for_type(type_index: usize) -> TriggerActionConfig {
    match type_index {
        0 => TriggerActionConfig::Highlight {
            fg: None,
            bg: Some([255, 255, 0]),
            duration_ms: 5000,
        },
        1 => TriggerActionConfig::Notify {
            title: "Trigger".to_string(),
            message: "Pattern matched".to_string(),
        },
        2 => TriggerActionConfig::MarkLine {
            label: None,
            color: Some([0, 180, 255]),
        },
        3 => TriggerActionConfig::SetVariable {
            name: String::new(),
            value: String::new(),
        },
        4 => TriggerActionConfig::RunCommand {
            command: String::new(),
            args: Vec::new(),
        },
        5 => TriggerActionConfig::PlaySound {
            sound_id: String::new(),
            volume: 50,
        },
        6 => TriggerActionConfig::SendText {
            text: String::new(),
            delay_ms: 0,
        },
        7 => TriggerActionConfig::Prettify {
            format: "json".to_string(),
            scope: crate::config::automation::PrettifyScope::default(),
            block_end: None,
            sub_format: None,
            command_filter: None,
        },
        _ => TriggerActionConfig::Highlight {
            fg: None,
            bg: Some([255, 255, 0]),
            duration_ms: 5000,
        },
    }
}

const ACTION_TYPE_NAMES: &[&str] = &[
    "Highlight",
    "Notify",
    "Mark Line",
    "Set Variable",
    "Run Command",
    "Play Sound",
    "Send Text",
    "Prettify",
];

pub(super) fn show_triggers_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    if section_matches(
        &settings.search_query.trim().to_lowercase(),
        "Triggers",
        &[
            "trigger",
            "regex",
            "pattern",
            "match",
            "action",
            "highlight",
            "notify",
            "badge",
            "set variable",
            "automatic",
        ],
    ) {
        show_triggers_collapsing(ui, settings, changes_this_frame, collapsed);
    }
}

fn show_triggers_collapsing(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Triggers",
        "automation_triggers",
        true,
        collapsed,
        |ui| {
            ui.label("Define regex patterns to match terminal output and trigger actions.");
            ui.add_space(4.0);

            // Collect mutations to apply after iteration
            let mut delete_index: Option<usize> = None;
            let mut toggle_index: Option<usize> = None;
            let mut start_edit_index: Option<usize> = None;

            // List existing triggers
            let trigger_count = settings.config.triggers.len();
            for i in 0..trigger_count {
                let trigger = &settings.config.triggers[i];
                let is_editing =
                    settings.editing_trigger_index == Some(i) && !settings.adding_new_trigger;

                if is_editing {
                    // Show inline edit form for this trigger
                    show_trigger_edit_form(ui, settings, changes_this_frame, Some(i));
                } else {
                    // Show trigger summary row
                    ui.horizontal(|ui| {
                        // Enabled checkbox
                        let mut enabled = trigger.enabled;
                        if ui.checkbox(&mut enabled, "").changed() {
                            toggle_index = Some(i);
                        }

                        // Name (bold)
                        ui.label(egui::RichText::new(&trigger.name).strong());

                        // Pattern (monospace)
                        ui.label(
                            egui::RichText::new(format!("/{}/", &trigger.pattern))
                                .monospace()
                                .color(egui::Color32::from_rgb(150, 150, 200)),
                        );

                        // Action count
                        let action_count = trigger.actions.len();
                        ui.label(
                            egui::RichText::new(format!(
                                "{} action{}",
                                action_count,
                                if action_count == 1 { "" } else { "s" }
                            ))
                            .color(egui::Color32::GRAY),
                        );

                        // Security indicator: warn if trigger allows dangerous
                        // actions from terminal output
                        if !trigger.require_user_action
                            && trigger.actions.iter().any(|a| a.is_dangerous())
                        {
                            ui.label(
                                egui::RichText::new("[unsafe]")
                                    .small()
                                    .color(egui::Color32::from_rgb(220, 160, 50)),
                            )
                            .on_hover_text(
                                "This trigger can execute dangerous actions \
                                 (RunCommand/SendText) from passive terminal output",
                            );
                        }

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

            // Apply mutations after iteration
            if let Some(i) = toggle_index {
                settings.config.triggers[i].enabled = !settings.config.triggers[i].enabled;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            if let Some(i) = delete_index {
                settings.config.triggers.remove(i);
                settings.has_changes = true;
                *changes_this_frame = true;
                // If we were editing this trigger, cancel the edit
                if settings.editing_trigger_index == Some(i) {
                    settings.editing_trigger_index = None;
                }
            }
            if let Some(i) = start_edit_index {
                let trigger = &settings.config.triggers[i];
                settings.editing_trigger_index = Some(i);
                settings.adding_new_trigger = false;
                settings.temp_trigger_name = trigger.name.clone();
                settings.temp_trigger_pattern = trigger.pattern.clone();
                settings.temp_trigger_actions = trigger.actions.clone();
                settings.temp_trigger_require_user_action = trigger.require_user_action;
                settings.trigger_pattern_error = None;
            }

            ui.add_space(4.0);

            // Add new trigger button / form
            if settings.adding_new_trigger {
                ui.separator();
                ui.label(egui::RichText::new("New Trigger").strong());
                show_trigger_edit_form(ui, settings, changes_this_frame, None);
            } else if settings.editing_trigger_index.is_none()
                && ui
                    .button("+ Add Trigger")
                    .on_hover_text("Add a new trigger definition")
                    .clicked()
            {
                settings.adding_new_trigger = true;
                settings.editing_trigger_index = None;
                settings.temp_trigger_name = String::new();
                settings.temp_trigger_pattern = String::new();
                settings.temp_trigger_actions = Vec::new();
                settings.temp_trigger_require_user_action = true;
                settings.trigger_pattern_error = None;
            }
        },
    );
}

fn show_trigger_edit_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    edit_index: Option<usize>,
) {
    ui.indent("trigger_edit_form", |ui| {
        // Name field
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut settings.temp_trigger_name);
        });

        // Pattern field with regex validation
        ui.horizontal(|ui| {
            ui.label("Pattern:");
            let response = ui.text_edit_singleline(&mut settings.temp_trigger_pattern);
            if response.changed() {
                // Validate regex
                settings.trigger_pattern_error =
                    match regex::Regex::new(&settings.temp_trigger_pattern) {
                        Ok(_) => None,
                        Err(e) => Some(format!("Invalid regex: {e}")),
                    };
            }
        });
        if let Some(ref err) = settings.trigger_pattern_error {
            ui.colored_label(egui::Color32::RED, err);
        }

        // Security: require_user_action checkbox
        // Only shown when the trigger has dangerous actions (RunCommand, SendText).
        let has_dangerous = settings
            .temp_trigger_actions
            .iter()
            .any(|a| a.is_dangerous());
        if has_dangerous {
            ui.add_space(4.0);
            ui.checkbox(
                &mut settings.temp_trigger_require_user_action,
                "Require user action (safe default)",
            )
            .on_hover_text(
                "When checked, RunCommand and SendText actions are blocked when triggered \
                 by passive terminal output. Uncheck ONLY if you trust all terminal output \
                 sources — malicious content could exploit pattern matching to run commands.",
            );
            if !settings.temp_trigger_require_user_action {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 160, 50),
                    "Warning: Dangerous actions can be triggered by terminal output. \
                     A command denylist and rate limiter still apply.",
                );
            }
        }

        // Actions list
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Actions:").strong());

        let mut action_delete_index: Option<usize> = None;
        for (j, action) in settings.temp_trigger_actions.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{}.", j + 1));
                ui.label(
                    egui::RichText::new(action_type_label(action))
                        .color(egui::Color32::from_rgb(120, 180, 255)),
                );
                show_action_fields(ui, action);
                if ui
                    .small_button(
                        egui::RichText::new("x").color(egui::Color32::from_rgb(200, 80, 80)),
                    )
                    .clicked()
                {
                    action_delete_index = Some(j);
                }
            });
        }

        if let Some(j) = action_delete_index {
            settings.temp_trigger_actions.remove(j);
        }

        // Add action combo
        ui.horizontal(|ui| {
            let combo_id = if edit_index.is_some() {
                "trigger_add_action_edit"
            } else {
                "trigger_add_action_new"
            };
            let mut selected_type: usize = 0;
            egui::ComboBox::from_id_salt(combo_id)
                .selected_text("+ Add action...")
                .show_ui(ui, |ui| {
                    for (idx, name) in ACTION_TYPE_NAMES.iter().enumerate() {
                        if ui
                            .selectable_value(&mut selected_type, idx + 1, *name)
                            .clicked()
                        {
                            // selected_type is set by selectable_value
                        }
                    }
                });
            if selected_type > 0 {
                settings
                    .temp_trigger_actions
                    .push(default_action_for_type(selected_type - 1));
            }
        });

        ui.add_space(4.0);

        // Save / Cancel buttons
        ui.horizontal(|ui| {
            let can_save = !settings.temp_trigger_name.trim().is_empty()
                && !settings.temp_trigger_pattern.trim().is_empty()
                && settings.trigger_pattern_error.is_none();

            if ui
                .add_enabled(can_save, egui::Button::new("Save"))
                .clicked()
            {
                let new_trigger = TriggerConfig {
                    name: settings.temp_trigger_name.trim().to_string(),
                    pattern: settings.temp_trigger_pattern.trim().to_string(),
                    enabled: true,
                    actions: settings.temp_trigger_actions.clone(),
                    require_user_action: settings.temp_trigger_require_user_action,
                };

                if let Some(i) = edit_index {
                    // Update existing
                    let was_enabled = settings.config.triggers[i].enabled;
                    settings.config.triggers[i] = new_trigger;
                    settings.config.triggers[i].enabled = was_enabled;
                } else {
                    // Add new
                    settings.config.triggers.push(new_trigger);
                }

                settings.has_changes = true;
                *changes_this_frame = true;
                settings.editing_trigger_index = None;
                settings.adding_new_trigger = false;
                settings.trigger_resync_requested = true;
            }

            if ui.button("Cancel").clicked() {
                settings.editing_trigger_index = None;
                settings.adding_new_trigger = false;
                settings.trigger_pattern_error = None;
            }
        });
    });
}

/// Show inline fields for a trigger action (for editing within the action row).
fn show_action_fields(ui: &mut egui::Ui, action: &mut TriggerActionConfig) {
    match action {
        TriggerActionConfig::Highlight {
            fg,
            bg,
            duration_ms,
        } => {
            // Background color picker
            if let Some(bg_color) = bg {
                let mut color = egui::Color32::from_rgb(bg_color[0], bg_color[1], bg_color[2]);
                if egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut color,
                    egui::color_picker::Alpha::Opaque,
                )
                .changed()
                {
                    *bg_color = [color.r(), color.g(), color.b()];
                }
            }
            // Foreground color picker
            if let Some(fg_color) = fg {
                let mut color = egui::Color32::from_rgb(fg_color[0], fg_color[1], fg_color[2]);
                ui.label("fg:");
                if egui::color_picker::color_edit_button_srgba(
                    ui,
                    &mut color,
                    egui::color_picker::Alpha::Opaque,
                )
                .changed()
                {
                    *fg_color = [color.r(), color.g(), color.b()];
                }
            }
            ui.label("ms:");
            ui.add(
                egui::DragValue::new(duration_ms)
                    .range(100..=60000)
                    .speed(100.0),
            );
        }
        TriggerActionConfig::Notify { title, message } => {
            ui.label("title:");
            ui.add(egui::TextEdit::singleline(title).desired_width(80.0));
            ui.label("msg:");
            ui.add(egui::TextEdit::singleline(message).desired_width(100.0));
        }
        TriggerActionConfig::MarkLine { label, color } => {
            ui.label("label:");
            let mut label_text = label.clone().unwrap_or_default();
            if ui
                .add(egui::TextEdit::singleline(&mut label_text).desired_width(80.0))
                .changed()
            {
                *label = if label_text.is_empty() {
                    None
                } else {
                    Some(label_text)
                };
            }
            ui.label("color:");
            // Ensure color is always set (backfill for configs created before
            // the color field was added)
            let c = color.get_or_insert([0, 180, 255]);
            let mut color_f = color_u8_to_f32(*c);
            if ui.color_edit_button_rgb(&mut color_f).changed() {
                *c = [
                    (color_f[0] * 255.0) as u8,
                    (color_f[1] * 255.0) as u8,
                    (color_f[2] * 255.0) as u8,
                ];
            }
        }
        TriggerActionConfig::SetVariable { name, value } => {
            ui.label("name:");
            ui.add(egui::TextEdit::singleline(name).desired_width(80.0));
            ui.label("=");
            ui.add(egui::TextEdit::singleline(value).desired_width(80.0));
        }
        TriggerActionConfig::RunCommand { command, args } => {
            ui.label("cmd:");
            ui.add(egui::TextEdit::singleline(command).desired_width(100.0));
            ui.label("args:");
            let mut args_str = args.join(" ");
            if ui
                .add(egui::TextEdit::singleline(&mut args_str).desired_width(80.0))
                .changed()
            {
                *args = args_str.split_whitespace().map(|s| s.to_string()).collect();
            }
        }
        TriggerActionConfig::PlaySound { sound_id, volume } => {
            ui.label("sound:");
            ui.add(egui::TextEdit::singleline(sound_id).desired_width(80.0));
            if ui.button("Browse...").clicked() {
                let sounds_dir = dirs::config_dir()
                    .map(|d| d.join("par-term").join("sounds"))
                    .unwrap_or_default();
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Select sound file")
                    .set_directory(&sounds_dir)
                    .add_filter("Audio", &["wav", "mp3", "ogg", "flac", "aac", "m4a"])
                    .pick_file()
                {
                    // If the file is inside the sounds directory, store just the filename;
                    // otherwise store the full path so play_sound_file can find it.
                    *sound_id = path
                        .strip_prefix(&sounds_dir)
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| path.display().to_string());
                }
            }
            ui.label("vol:");
            ui.add(egui::DragValue::new(volume).range(0..=100).speed(1.0));
        }
        TriggerActionConfig::SendText { text, delay_ms } => {
            ui.label("text:");
            ui.add(egui::TextEdit::singleline(text).desired_width(100.0));
            ui.label("delay:");
            ui.add(egui::DragValue::new(delay_ms).range(0..=10000).speed(10.0));
        }
        TriggerActionConfig::Prettify {
            format,
            scope,
            block_end,
            sub_format,
            command_filter,
        } => {
            ui.label("format:");
            ui.add(egui::TextEdit::singleline(format).desired_width(60.0));
            ui.label("scope:");
            egui::ComboBox::from_id_salt("prettify_scope")
                .selected_text(match scope {
                    crate::config::automation::PrettifyScope::Line => "Line",
                    crate::config::automation::PrettifyScope::Block => "Block",
                    crate::config::automation::PrettifyScope::CommandOutput => "Command Output",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        scope,
                        crate::config::automation::PrettifyScope::Line,
                        "Line",
                    );
                    ui.selectable_value(
                        scope,
                        crate::config::automation::PrettifyScope::Block,
                        "Block",
                    );
                    ui.selectable_value(
                        scope,
                        crate::config::automation::PrettifyScope::CommandOutput,
                        "Command Output",
                    );
                });

            // Optional fields shown inline.
            if let Some(be) = block_end {
                ui.label("end:");
                ui.add(egui::TextEdit::singleline(be).desired_width(60.0));
            }
            if let Some(sf) = sub_format {
                ui.label("sub:");
                ui.add(egui::TextEdit::singleline(sf).desired_width(60.0));
            }
            if let Some(cf) = command_filter {
                ui.label("cmd filter:");
                ui.add(egui::TextEdit::singleline(cf).desired_width(60.0));
            }
        }
    }
}

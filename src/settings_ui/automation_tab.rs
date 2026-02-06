//! Automation settings tab.
//!
//! Contains:
//! - Trigger definitions (regex patterns with actions)
//! - Coprocess definitions (external processes piped to terminal)

use super::SettingsUI;
use super::section::collapsing_section;
use crate::config::automation::{
    CoprocessDefConfig, RestartPolicy, TriggerActionConfig, TriggerConfig,
};

/// Show the automation tab content.
pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    let query = settings.search_query.trim().to_lowercase();

    // Triggers section
    if section_matches(
        &query,
        "Triggers",
        &[
            "trigger",
            "regex",
            "pattern",
            "match",
            "action",
            "highlight",
            "notify",
        ],
    ) {
        show_triggers_section(ui, settings, changes_this_frame);
    }

    // Coprocesses section
    if section_matches(
        &query,
        "Coprocesses",
        &[
            "coprocess",
            "pipe",
            "subprocess",
            "auto start",
            "auto-start",
            "restart",
            "restart policy",
            "restart delay",
        ],
    ) {
        show_coprocesses_section(ui, settings, changes_this_frame);
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
];

// ============================================================================
// Triggers Section
// ============================================================================

fn show_triggers_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Triggers", "automation_triggers", true, |ui| {
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
            settings.trigger_pattern_error = None;
        }
    });
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
            let mut color_f = [
                c[0] as f32 / 255.0,
                c[1] as f32 / 255.0,
                c[2] as f32 / 255.0,
            ];
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
    }
}

// ============================================================================
// Coprocesses Section
// ============================================================================

fn show_coprocesses_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Coprocesses", "automation_coprocesses", true, |ui| {
        ui.label("Define external processes that can exchange data with the terminal.");
        ui.add_space(4.0);

        // Collect mutations to apply after iteration
        let mut delete_index: Option<usize> = None;
        let mut start_edit_index: Option<usize> = None;

        // List existing coprocesses
        let coproc_count = settings.config.coprocesses.len();
        for i in 0..coproc_count {
            let coproc = &settings.config.coprocesses[i];
            let is_editing =
                settings.editing_coprocess_index == Some(i) && !settings.adding_new_coprocess;

            if is_editing {
                show_coprocess_edit_form(ui, settings, changes_this_frame, Some(i));
            } else {
                let is_running = settings
                    .coprocess_running
                    .get(i)
                    .copied()
                    .unwrap_or(false);

                // First row: status + name + buttons (right-aligned)
                ui.horizontal(|ui| {
                    // Running/stopped status indicator
                    if is_running {
                        ui.label(
                            egui::RichText::new("●")
                                .color(egui::Color32::from_rgb(100, 200, 100)),
                        );
                    } else if coproc.auto_start {
                        ui.label(
                            egui::RichText::new("[auto]")
                                .color(egui::Color32::from_rgb(100, 200, 100))
                                .small(),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new("○")
                                .color(egui::Color32::GRAY),
                        );
                    }

                    // Name (bold)
                    ui.label(egui::RichText::new(&coproc.name).strong());

                    // Right-align buttons
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

                        // Start/Stop button
                        if is_running {
                            if ui
                                .small_button(
                                    egui::RichText::new("Stop")
                                        .color(egui::Color32::from_rgb(220, 160, 50)),
                                )
                                .clicked()
                            {
                                settings.pending_coprocess_actions.push((i, false));
                            }
                        } else if ui.small_button("Start").clicked() {
                            log::debug!("Coprocess Start button clicked for index {}", i);
                            // Clear any previous error message
                            if let Some(err) = settings.coprocess_errors.get_mut(i) {
                                err.clear();
                            }
                            settings.pending_coprocess_actions.push((i, true));
                        }
                    });
                });

                // Second row: command (indented, truncated if long)
                let cmd_display = if coproc.args.is_empty() {
                    coproc.command.clone()
                } else {
                    format!("{} {}", coproc.command, coproc.args.join(" "))
                };
                ui.indent(format!("coproc_cmd_{}", i), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(&cmd_display)
                                .monospace()
                                .small()
                                .color(egui::Color32::from_rgb(150, 150, 200)),
                        );
                        // Show restart policy info
                        if coproc.restart_policy != RestartPolicy::Never {
                            let restart_text = if coproc.restart_delay_ms > 0 {
                                format!(
                                    "[restart: {}, delay: {}ms]",
                                    coproc.restart_policy.display_name(),
                                    coproc.restart_delay_ms
                                )
                            } else {
                                format!(
                                    "[restart: {}]",
                                    coproc.restart_policy.display_name()
                                )
                            };
                            ui.label(
                                egui::RichText::new(restart_text)
                                    .small()
                                    .color(egui::Color32::from_rgb(180, 180, 100)),
                            );
                        }
                    });
                });

                // Show error message if coprocess died with stderr output
                let has_error = settings
                    .coprocess_errors
                    .get(i)
                    .is_some_and(|e| !e.is_empty());
                if has_error && !is_running {
                    let err_text = &settings.coprocess_errors[i];
                    ui.indent(format!("coproc_err_{}", i), |ui| {
                        ui.label(
                            egui::RichText::new(format!("Error: {}", err_text))
                                .small()
                                .color(egui::Color32::from_rgb(220, 80, 80)),
                        );
                    });
                }

                // Output viewer (collapsible)
                let has_output = settings
                    .coprocess_output
                    .get(i)
                    .is_some_and(|lines| !lines.is_empty());
                if has_output {
                    let is_expanded = settings
                        .coprocess_output_expanded
                        .get(i)
                        .copied()
                        .unwrap_or(false);
                    let line_count = settings.coprocess_output[i].len();
                    ui.indent(format!("coproc_out_{}", i), |ui| {
                        let toggle_text = if is_expanded {
                            format!("▼ Output ({} lines)", line_count)
                        } else {
                            format!("▶ Output ({} lines)", line_count)
                        };
                        if ui
                            .small_button(
                                egui::RichText::new(&toggle_text)
                                    .small()
                                    .color(egui::Color32::from_rgb(140, 180, 140)),
                            )
                            .clicked()
                            && let Some(expanded) =
                                settings.coprocess_output_expanded.get_mut(i)
                        {
                            *expanded = !*expanded;
                        }
                        if is_expanded {
                            let output_text = settings.coprocess_output[i].join("\n");
                            egui::ScrollArea::vertical()
                                .id_salt(format!("coproc_output_scroll_{}", i))
                                .max_height(150.0)
                                .stick_to_bottom(true)
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(&output_text)
                                            .monospace()
                                            .small()
                                            .color(egui::Color32::from_rgb(180, 180, 180)),
                                    );
                                });
                            if ui.small_button("Clear").clicked() {
                                settings.coprocess_output[i].clear();
                            }
                        }
                    });
                }

                ui.add_space(2.0);
            }
        }

        // Apply mutations
        if let Some(i) = delete_index {
            settings.config.coprocesses.remove(i);
            settings.has_changes = true;
            *changes_this_frame = true;
            if settings.editing_coprocess_index == Some(i) {
                settings.editing_coprocess_index = None;
            }
        }
        if let Some(i) = start_edit_index {
            let coproc = &settings.config.coprocesses[i];
            settings.editing_coprocess_index = Some(i);
            settings.adding_new_coprocess = false;
            settings.temp_coprocess_name = coproc.name.clone();
            settings.temp_coprocess_command = coproc.command.clone();
            settings.temp_coprocess_args = coproc.args.join(" ");
            settings.temp_coprocess_auto_start = coproc.auto_start;
            settings.temp_coprocess_copy_output = coproc.copy_terminal_output;
            settings.temp_coprocess_restart_policy = coproc.restart_policy;
            settings.temp_coprocess_restart_delay_ms = coproc.restart_delay_ms;
        }

        ui.add_space(4.0);

        // Add new coprocess button / form
        if settings.adding_new_coprocess {
            ui.separator();
            ui.label(egui::RichText::new("New Coprocess").strong());
            show_coprocess_edit_form(ui, settings, changes_this_frame, None);
        } else if settings.editing_coprocess_index.is_none()
            && ui
                .button("+ Add Coprocess")
                .on_hover_text("Add a new coprocess definition")
                .clicked()
        {
            settings.adding_new_coprocess = true;
            settings.editing_coprocess_index = None;
            settings.temp_coprocess_name = String::new();
            settings.temp_coprocess_command = String::new();
            settings.temp_coprocess_args = String::new();
            settings.temp_coprocess_auto_start = false;
            settings.temp_coprocess_copy_output = true;
            settings.temp_coprocess_restart_policy = RestartPolicy::Never;
            settings.temp_coprocess_restart_delay_ms = 0;
        }
    });
}

fn show_coprocess_edit_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    edit_index: Option<usize>,
) {
    ui.indent("coprocess_edit_form", |ui| {
        // Name field
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut settings.temp_coprocess_name);
        });

        // Command field
        ui.horizontal(|ui| {
            ui.label("Command:");
            ui.text_edit_singleline(&mut settings.temp_coprocess_command);
        });

        // Args field
        ui.horizontal(|ui| {
            ui.label("Arguments:");
            ui.text_edit_singleline(&mut settings.temp_coprocess_args);
        });

        // Options (use persistent temp fields so checkbox state survives across frames)
        ui.checkbox(
            &mut settings.temp_coprocess_auto_start,
            "Auto-start with terminal",
        )
        .on_hover_text("Start this coprocess automatically when a new tab is opened");
        ui.checkbox(
            &mut settings.temp_coprocess_copy_output,
            "Copy terminal output",
        )
        .on_hover_text("Send terminal output to the coprocess stdin");

        // Restart policy
        ui.horizontal(|ui| {
            ui.label("Restart policy:");
            egui::ComboBox::from_id_salt(if edit_index.is_some() {
                "coproc_restart_policy_edit"
            } else {
                "coproc_restart_policy_new"
            })
            .selected_text(settings.temp_coprocess_restart_policy.display_name())
            .show_ui(ui, |ui| {
                for &policy in RestartPolicy::all() {
                    ui.selectable_value(
                        &mut settings.temp_coprocess_restart_policy,
                        policy,
                        policy.display_name(),
                    );
                }
            });
        });

        // Restart delay (only shown when restart policy is not Never)
        if settings.temp_coprocess_restart_policy != RestartPolicy::Never {
            ui.horizontal(|ui| {
                ui.label("Restart delay (ms):");
                ui.add(
                    egui::DragValue::new(&mut settings.temp_coprocess_restart_delay_ms)
                        .range(0..=60000)
                        .speed(100.0),
                );
            });
        }

        ui.add_space(4.0);

        // Save / Cancel
        ui.horizontal(|ui| {
            let can_save = !settings.temp_coprocess_name.trim().is_empty()
                && !settings.temp_coprocess_command.trim().is_empty();

            if ui
                .add_enabled(can_save, egui::Button::new("Save"))
                .clicked()
            {
                let args: Vec<String> = settings
                    .temp_coprocess_args
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();

                let new_coproc = CoprocessDefConfig {
                    name: settings.temp_coprocess_name.trim().to_string(),
                    command: settings.temp_coprocess_command.trim().to_string(),
                    args,
                    auto_start: settings.temp_coprocess_auto_start,
                    copy_terminal_output: settings.temp_coprocess_copy_output,
                    restart_policy: settings.temp_coprocess_restart_policy,
                    restart_delay_ms: settings.temp_coprocess_restart_delay_ms,
                };

                if let Some(i) = edit_index {
                    settings.config.coprocesses[i] = new_coproc;
                } else {
                    settings.config.coprocesses.push(new_coproc);
                }

                settings.has_changes = true;
                *changes_this_frame = true;
                settings.editing_coprocess_index = None;
                settings.adding_new_coprocess = false;
            }

            if ui.button("Cancel").clicked() {
                settings.editing_coprocess_index = None;
                settings.adding_new_coprocess = false;
            }
        });
    });
}

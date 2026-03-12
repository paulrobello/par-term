//! Inline field rendering for each trigger action variant.

use par_term_config::automation::{
    SplitPaneCommand, TriggerActionConfig, TriggerSplitDirection, TriggerSplitTarget,
};
use par_term_config::color_u8_to_f32;

/// Show inline fields for a trigger action (for editing within the action row).
pub(super) fn show_action_fields(ui: &mut egui::Ui, action: &mut TriggerActionConfig) {
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
        TriggerActionConfig::SplitPane {
            direction,
            command,
            focus_new_pane,
            target,
        } => {
            ui.vertical(|ui| {
                // Direction row
                ui.horizontal(|ui| {
                    ui.label("Direction:");
                    egui::ComboBox::from_id_salt("split_pane_direction")
                        .selected_text(match direction {
                            TriggerSplitDirection::Horizontal => "Horizontal (new pane below)",
                            TriggerSplitDirection::Vertical => "Vertical (new pane to the right)",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                direction,
                                TriggerSplitDirection::Horizontal,
                                "Horizontal (new pane below)",
                            );
                            ui.selectable_value(
                                direction,
                                TriggerSplitDirection::Vertical,
                                "Vertical (new pane to the right)",
                            );
                        });
                });

                // Target row
                ui.horizontal(|ui| {
                    ui.label("Target:");
                    egui::ComboBox::from_id_salt("split_pane_target")
                        .selected_text(match target {
                            TriggerSplitTarget::Active => "Active Pane",
                            TriggerSplitTarget::Source => "Source Pane (when available)",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(target, TriggerSplitTarget::Active, "Active Pane");
                            ui.selectable_value(
                                target,
                                TriggerSplitTarget::Source,
                                "Source Pane (when available)",
                            );
                        });
                });

                // Focus new pane checkbox
                ui.checkbox(focus_new_pane, "Focus new pane after splitting");

                // Command type selector
                // Determine current command type index: 0=None, 1=SendText, 2=InitialCommand
                let current_cmd_type: usize = match command {
                    None => 0,
                    Some(SplitPaneCommand::SendText { .. }) => 1,
                    Some(SplitPaneCommand::InitialCommand { .. }) => 2,
                };
                let mut new_cmd_type = current_cmd_type;

                ui.horizontal(|ui| {
                    ui.label("Command:");
                    egui::ComboBox::from_id_salt("split_pane_cmd_type")
                        .selected_text(match current_cmd_type {
                            1 => "Send Text",
                            2 => "Initial Command",
                            _ => "None",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut new_cmd_type, 0, "None");
                            ui.selectable_value(&mut new_cmd_type, 1, "Send Text");
                            ui.selectable_value(&mut new_cmd_type, 2, "Initial Command");
                        });
                });

                // Apply command type change if user switched
                if new_cmd_type != current_cmd_type {
                    *command = match new_cmd_type {
                        0 => None,
                        1 => Some(SplitPaneCommand::SendText {
                            text: String::new(),
                            delay_ms: 200,
                        }),
                        2 => Some(SplitPaneCommand::InitialCommand {
                            command: String::new(),
                            args: Vec::new(),
                        }),
                        _ => None,
                    };
                }

                // Sub-fields for the selected command type
                match command {
                    Some(SplitPaneCommand::SendText { text, delay_ms }) => {
                        ui.horizontal(|ui| {
                            ui.label("Text to send:");
                            ui.text_edit_singleline(text);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Delay (ms):");
                            ui.add(egui::DragValue::new(delay_ms).range(0..=5000).speed(10.0));
                        });
                    }
                    Some(SplitPaneCommand::InitialCommand {
                        command: cmd_str,
                        args,
                    }) => {
                        ui.horizontal(|ui| {
                            ui.label("Command:");
                            ui.text_edit_singleline(cmd_str);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Arguments (space-separated):");
                            let mut args_str = args.join(" ");
                            if ui.text_edit_singleline(&mut args_str).changed() {
                                *args = args_str.split_whitespace().map(String::from).collect();
                            }
                        });
                    }
                    None => {}
                }
            });
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

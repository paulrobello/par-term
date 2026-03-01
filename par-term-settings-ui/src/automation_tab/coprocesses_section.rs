//! Coprocesses section of the automation settings tab.

use crate::SettingsUI;
use crate::section::{collapsing_section, section_matches};
use par_term_config::automation::{CoprocessDefConfig, RestartPolicy};
use std::collections::HashSet;

pub(super) fn show_coprocesses_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    if section_matches(
        &settings.search_query.trim().to_lowercase(),
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
            "output",
            "filter",
        ],
    ) {
        show_coprocesses_collapsing(ui, settings, changes_this_frame, collapsed);
    }
}

fn show_coprocesses_collapsing(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Coprocesses",
        "automation_coprocesses",
        true,
        collapsed,
        |ui| {
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
                    let is_running = settings.coprocess_running.get(i).copied().unwrap_or(false);

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
                            ui.label(egui::RichText::new("○").color(egui::Color32::GRAY));
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
                                    format!("[restart: {}]", coproc.restart_policy.display_name())
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
        },
    );
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

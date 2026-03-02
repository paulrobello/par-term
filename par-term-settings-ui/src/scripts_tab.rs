//! Scripts settings tab.
//!
//! Contains management for external observer scripts that receive terminal events
//! via JSON protocol and can send commands back.

use super::SettingsUI;
use super::section::{collapsing_section, collapsing_section_with_state, section_matches};
use par_term_config::automation::RestartPolicy;
use par_term_config::scripting::ScriptConfig;
use std::collections::HashSet;

/// Show the scripts tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    if section_matches(
        &query,
        "Scripts",
        &[
            "script",
            "scripting",
            "observer",
            "event",
            "subprocess",
            "python",
            "panel",
        ],
    ) {
        show_scripts_section(ui, settings, changes_this_frame, collapsed);
    }
}

fn show_scripts_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section_with_state(
        ui,
        "Observer Scripts",
        "scripts_list",
        true,
        collapsed,
        |ui, collapsed| {
            ui.label(
                "Define external scripts that receive terminal events and can send commands back.",
            );
            ui.add_space(4.0);

            // Collect mutations to apply after iteration
            let mut delete_index: Option<usize> = None;
            let mut start_edit_index: Option<usize> = None;
            let mut toggle_index: Option<usize> = None;

            // List existing scripts
            let script_count = settings.config.scripts.len();
            for i in 0..script_count {
                let script = &settings.config.scripts[i];
                let is_editing =
                    settings.editing_script_index == Some(i) && !settings.adding_new_script;

                if is_editing {
                    show_script_edit_form(ui, settings, changes_this_frame, Some(i));
                } else {
                    let is_running = settings.script_running.get(i).copied().unwrap_or(false);
                    let has_error = settings.script_errors.get(i).is_some_and(|e| !e.is_empty());

                    // First row: status + enabled + name + buttons
                    ui.horizontal(|ui| {
                        // Status indicator: green=running, red=error, gray=stopped
                        if is_running {
                            ui.label(
                                egui::RichText::new("\u{25cf}")
                                    .color(egui::Color32::from_rgb(100, 200, 100)),
                            );
                        } else if has_error {
                            ui.label(
                                egui::RichText::new("\u{25cf}")
                                    .color(egui::Color32::from_rgb(220, 80, 80)),
                            );
                        } else if script.auto_start {
                            ui.label(
                                egui::RichText::new("[auto]")
                                    .color(egui::Color32::from_rgb(100, 200, 100))
                                    .small(),
                            );
                        } else {
                            ui.label(egui::RichText::new("\u{25cb}").color(egui::Color32::GRAY));
                        }

                        // Enabled checkbox
                        let mut enabled = script.enabled;
                        if ui.checkbox(&mut enabled, "").changed() {
                            toggle_index = Some(i);
                        }

                        // Name (bold)
                        ui.label(egui::RichText::new(&script.name).strong());

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
                                    settings.pending_script_actions.push((i, false));
                                }
                            } else if ui.small_button("Start").clicked() {
                                log::debug!("Script Start button clicked for index {}", i);
                                // Clear any previous error message
                                if let Some(err) = settings.script_errors.get_mut(i) {
                                    err.clear();
                                }
                                settings.pending_script_actions.push((i, true));
                            }
                        });
                    });

                    // Second row: script path (indented, monospace)
                    let path_display = if script.args.is_empty() {
                        script.script_path.clone()
                    } else {
                        format!("{} {}", script.script_path, script.args.join(" "))
                    };
                    ui.indent(format!("script_path_{}", i), |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&path_display)
                                    .monospace()
                                    .small()
                                    .color(egui::Color32::from_rgb(150, 150, 200)),
                            );
                            // Show restart policy info
                            if script.restart_policy != RestartPolicy::Never {
                                let restart_text = if script.restart_delay_ms > 0 {
                                    format!(
                                        "[restart: {}, delay: {}ms]",
                                        script.restart_policy.display_name(),
                                        script.restart_delay_ms
                                    )
                                } else {
                                    format!("[restart: {}]", script.restart_policy.display_name())
                                };
                                ui.label(
                                    egui::RichText::new(restart_text)
                                        .small()
                                        .color(egui::Color32::from_rgb(180, 180, 100)),
                                );
                            }
                            // Show subscriptions if any
                            if !script.subscriptions.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "[{}]",
                                        script.subscriptions.join(", ")
                                    ))
                                    .small()
                                    .color(egui::Color32::from_rgb(140, 180, 220)),
                                );
                            }
                        });
                    });

                    // Show error message if script died with stderr output
                    if has_error && !is_running {
                        let err_text = &settings.script_errors[i];
                        ui.indent(format!("script_err_{}", i), |ui| {
                            ui.label(
                                egui::RichText::new(format!("Error: {}", err_text))
                                    .small()
                                    .color(egui::Color32::from_rgb(220, 80, 80)),
                            );
                        });
                    }

                    // Output viewer (collapsible)
                    let has_output = settings
                        .script_output
                        .get(i)
                        .is_some_and(|lines| !lines.is_empty());
                    if has_output {
                        let is_expanded = settings
                            .script_output_expanded
                            .get(i)
                            .copied()
                            .unwrap_or(false);
                        let line_count = settings.script_output[i].len();
                        ui.indent(format!("script_out_{}", i), |ui| {
                            let toggle_text = if is_expanded {
                                format!("\u{25bc} Output ({} lines)", line_count)
                            } else {
                                format!("\u{25b6} Output ({} lines)", line_count)
                            };
                            if ui
                                .small_button(
                                    egui::RichText::new(&toggle_text)
                                        .small()
                                        .color(egui::Color32::from_rgb(140, 180, 140)),
                                )
                                .clicked()
                                && let Some(expanded) = settings.script_output_expanded.get_mut(i)
                            {
                                *expanded = !*expanded;
                            }
                            if is_expanded {
                                let output_text = settings.script_output[i].join("\n");
                                egui::ScrollArea::vertical()
                                    .id_salt(format!("script_output_scroll_{}", i))
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
                                    settings.script_output[i].clear();
                                }
                            }
                        });
                    }

                    // Panel viewer (collapsible)
                    if let Some(Some((title, content))) = settings.script_panels.get(i) {
                        let panel_title = format!("Panel: {}", title);
                        let panel_id = format!("script_panel_{}", i);
                        let panel_scroll_id = format!("script_panel_scroll_{}", i);
                        ui.indent(&panel_id, |ui| {
                            collapsing_section(
                                ui,
                                &panel_title,
                                &panel_id,
                                false,
                                collapsed,
                                |ui| {
                                    egui::ScrollArea::vertical()
                                        .id_salt(&panel_scroll_id)
                                        .max_height(200.0)
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(content)
                                                    .monospace()
                                                    .small()
                                                    .color(egui::Color32::from_rgb(200, 200, 200)),
                                            );
                                        });
                                },
                            );
                        });
                    }

                    ui.add_space(2.0);
                }
            }

            // Apply mutations
            if let Some(i) = toggle_index {
                settings.config.scripts[i].enabled = !settings.config.scripts[i].enabled;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            if let Some(i) = delete_index {
                settings.config.scripts.remove(i);
                settings.has_changes = true;
                *changes_this_frame = true;
                if settings.editing_script_index == Some(i) {
                    settings.editing_script_index = None;
                }
            }
            if let Some(i) = start_edit_index {
                let script = &settings.config.scripts[i];
                settings.editing_script_index = Some(i);
                settings.adding_new_script = false;
                settings.temp_script_name = script.name.clone();
                settings.temp_script_path = script.script_path.clone();
                settings.temp_script_args = script.args.join(" ");
                settings.temp_script_auto_start = script.auto_start;
                settings.temp_script_enabled = script.enabled;
                settings.temp_script_restart_policy = script.restart_policy;
                settings.temp_script_restart_delay_ms = script.restart_delay_ms;
                settings.temp_script_subscriptions = script.subscriptions.join(", ");
                settings.temp_script_allow_write_text = script.allow_write_text;
                settings.temp_script_allow_run_command = script.allow_run_command;
                settings.temp_script_allow_change_config = script.allow_change_config;
                settings.temp_script_write_text_rate_limit = script.write_text_rate_limit;
                settings.temp_script_run_command_rate_limit = script.run_command_rate_limit;
            }

            ui.add_space(4.0);

            // Add new script button / form
            if settings.adding_new_script {
                ui.separator();
                ui.label(egui::RichText::new("New Script").strong());
                show_script_edit_form(ui, settings, changes_this_frame, None);
            } else if settings.editing_script_index.is_none()
                && ui
                    .button("+ Add Script")
                    .on_hover_text("Add a new observer script definition")
                    .clicked()
            {
                settings.adding_new_script = true;
                settings.editing_script_index = None;
                settings.temp_script_name = String::new();
                settings.temp_script_path = String::new();
                settings.temp_script_args = String::new();
                settings.temp_script_auto_start = false;
                settings.temp_script_enabled = true;
                settings.temp_script_restart_policy = RestartPolicy::Never;
                settings.temp_script_restart_delay_ms = 0;
                settings.temp_script_subscriptions = String::new();
                settings.temp_script_allow_write_text = false;
                settings.temp_script_allow_run_command = false;
                settings.temp_script_allow_change_config = false;
                settings.temp_script_write_text_rate_limit = 0;
                settings.temp_script_run_command_rate_limit = 0;
            }
        },
    );
}

fn show_script_edit_form(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    edit_index: Option<usize>,
) {
    ui.indent("script_edit_form", |ui| {
        // Name field
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut settings.temp_script_name);
        });

        // Script path field
        ui.horizontal(|ui| {
            ui.label("Script path:");
            ui.text_edit_singleline(&mut settings.temp_script_path);
            if ui.button("Browse...").clicked()
                && let Some(path) = settings.pick_file_path("Select script file")
            {
                settings.temp_script_path = path;
            }
        });

        // Args field
        ui.horizontal(|ui| {
            ui.label("Arguments:");
            ui.text_edit_singleline(&mut settings.temp_script_args);
        });

        // Subscriptions
        ui.horizontal(|ui| {
            ui.label("Subscriptions:");
            ui.text_edit_singleline(&mut settings.temp_script_subscriptions)
                .on_hover_text(
                    "Comma-separated event types (e.g., bell_rang, cwd_changed). Empty = all events.",
                );
        });

        // Options
        ui.checkbox(&mut settings.temp_script_enabled, "Enabled")
            .on_hover_text("Whether this script is active");
        ui.checkbox(
            &mut settings.temp_script_auto_start,
            "Auto-start with terminal",
        )
        .on_hover_text("Start this script automatically when a new tab is opened");

        // Restart policy
        ui.horizontal(|ui| {
            ui.label("Restart policy:");
            egui::ComboBox::from_id_salt(if edit_index.is_some() {
                "script_restart_policy_edit"
            } else {
                "script_restart_policy_new"
            })
            .selected_text(settings.temp_script_restart_policy.display_name())
            .show_ui(ui, |ui| {
                for &policy in RestartPolicy::all() {
                    ui.selectable_value(
                        &mut settings.temp_script_restart_policy,
                        policy,
                        policy.display_name(),
                    );
                }
            });
        });

        // Restart delay (only shown when restart policy is not Never)
        if settings.temp_script_restart_policy != RestartPolicy::Never {
            ui.horizontal(|ui| {
                ui.label("Restart delay (ms):");
                ui.add(
                    egui::DragValue::new(&mut settings.temp_script_restart_delay_ms)
                        .range(0..=60000)
                        .speed(100.0),
                );
            });
        }

        ui.add_space(4.0);

        // Permissions section
        ui.label(egui::RichText::new("Permissions").strong().small());
        ui.indent("script_permissions", |ui| {
            ui.checkbox(
                &mut settings.temp_script_allow_write_text,
                "Allow WriteText",
            )
            .on_hover_text(
                "Allow this script to inject text into the active PTY. \
                 VT/ANSI escape sequences are stripped before writing.",
            );

            if settings.temp_script_allow_write_text {
                ui.indent("write_text_rate", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Rate limit (writes/sec, 0 = default 10/s):");
                        ui.add(
                            egui::DragValue::new(
                                &mut settings.temp_script_write_text_rate_limit,
                            )
                            .range(0..=100)
                            .speed(1.0),
                        );
                    });
                });
            }

            ui.checkbox(
                &mut settings.temp_script_allow_run_command,
                "Allow RunCommand",
            )
            .on_hover_text(
                "Allow this script to spawn external processes. \
                 Commands are checked against the denylist and tokenised without shell invocation.",
            );

            if settings.temp_script_allow_run_command {
                ui.indent("run_command_rate", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Rate limit (runs/sec, 0 = default 1/s):");
                        ui.add(
                            egui::DragValue::new(
                                &mut settings.temp_script_run_command_rate_limit,
                            )
                            .range(0..=10)
                            .speed(1.0),
                        );
                    });
                });
            }

            ui.checkbox(
                &mut settings.temp_script_allow_change_config,
                "Allow ChangeConfig",
            )
            .on_hover_text(
                "Allow this script to modify runtime configuration values. \
                 Only allowlisted keys (font_size, window_opacity, etc.) may be changed.",
            );
        });

        ui.add_space(4.0);

        // Save / Cancel
        ui.horizontal(|ui| {
            let can_save = !settings.temp_script_name.trim().is_empty()
                && !settings.temp_script_path.trim().is_empty();

            if ui
                .add_enabled(can_save, egui::Button::new("Save"))
                .clicked()
            {
                let args: Vec<String> = settings
                    .temp_script_args
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();

                let subscriptions: Vec<String> = settings
                    .temp_script_subscriptions
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                let new_script = ScriptConfig {
                    name: settings.temp_script_name.trim().to_string(),
                    enabled: settings.temp_script_enabled,
                    script_path: settings.temp_script_path.trim().to_string(),
                    args,
                    auto_start: settings.temp_script_auto_start,
                    restart_policy: settings.temp_script_restart_policy,
                    restart_delay_ms: settings.temp_script_restart_delay_ms,
                    subscriptions,
                    env_vars: if let Some(i) = edit_index {
                        settings.config.scripts[i].env_vars.clone()
                    } else {
                        std::collections::HashMap::new()
                    },
                    allow_write_text: settings.temp_script_allow_write_text,
                    allow_run_command: settings.temp_script_allow_run_command,
                    allow_change_config: settings.temp_script_allow_change_config,
                    write_text_rate_limit: settings.temp_script_write_text_rate_limit,
                    run_command_rate_limit: settings.temp_script_run_command_rate_limit,
                };

                if let Some(i) = edit_index {
                    settings.config.scripts[i] = new_script;
                } else {
                    settings.config.scripts.push(new_script);
                }

                settings.has_changes = true;
                *changes_this_frame = true;
                settings.editing_script_index = None;
                settings.adding_new_script = false;
            }

            if ui.button("Cancel").clicked() {
                settings.editing_script_index = None;
                settings.adding_new_script = false;
            }
        });
    });
}

/// Search keywords for the Scripts settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        "script",
        "scripting",
        "python",
        "automation",
        "observer",
        "event",
        "subprocess",
        "external",
        "panel",
        "subscriptions",
        // Script management
        "script path",
        "arguments",
        "args",
        "start",
        "stop",
        "auto-start",
        "auto start",
        "auto-launch",
        "restart",
        "restart policy",
        "restart delay",
        // Permissions
        "permission",
        "allow",
        "write text",
        "run command",
        "change config",
        "rate limit",
    ]
}

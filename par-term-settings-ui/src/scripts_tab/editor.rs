//! Script edit form: name, path, args, permissions, save/cancel.

use crate::settings_ui::SettingsUI;
use par_term_config::automation::RestartPolicy;
use par_term_config::scripting::ScriptConfig;

/// Show the inline edit form for a script.
///
/// `edit_index` is `Some(i)` when editing an existing script, `None` when adding a new one.
pub(super) fn show_script_edit_form(
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
        show_permissions_section(ui, settings);

        ui.add_space(4.0);

        // Save / Cancel
        show_save_cancel(ui, settings, changes_this_frame, edit_index);
    });
}

/// Show the permissions sub-section within the script edit form.
fn show_permissions_section(ui: &mut egui::Ui, settings: &mut SettingsUI) {
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
                        egui::DragValue::new(&mut settings.temp_script_write_text_rate_limit)
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
                        egui::DragValue::new(&mut settings.temp_script_run_command_rate_limit)
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
}

/// Show the Save / Cancel button row and apply the save if clicked.
fn show_save_cancel(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    edit_index: Option<usize>,
) {
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
}

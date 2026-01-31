use super::SettingsUI;

pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, _changes_this_frame: &mut bool) {
    ui.collapsing("Shell Configuration", |ui| {
        ui.horizontal(|ui| {
            ui.label("Custom shell (optional):");
            if ui
                .text_edit_singleline(&mut settings.temp_custom_shell)
                .changed()
            {
                settings.config.custom_shell = if settings.temp_custom_shell.is_empty()
                {
                    None
                } else {
                    Some(settings.temp_custom_shell.clone())
                };
                settings.has_changes = true;
            }

            if ui.button("Browse…").clicked()
                && let Some(path) = settings.pick_file_path("Select shell binary")
            {
                settings.temp_custom_shell = path.clone();
                settings.config.custom_shell = Some(path);
                settings.has_changes = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Shell args (space-separated):");
            if ui.text_edit_singleline(&mut settings.temp_shell_args).changed() {
                settings.config.shell_args = if settings.temp_shell_args.is_empty() {
                    None
                } else {
                    Some(
                        settings.temp_shell_args
                            .split_whitespace()
                            .map(String::from)
                            .collect(),
                    )
                };
                settings.has_changes = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Working directory (optional):");
            if ui
                .text_edit_singleline(&mut settings.temp_working_directory)
                .changed()
            {
                settings.config.working_directory =
                    if settings.temp_working_directory.is_empty() {
                        None
                    } else {
                        Some(settings.temp_working_directory.clone())
                    };
                settings.has_changes = true;
            }

            if ui.button("Browse…").clicked()
                && let Some(path) =
                    settings.pick_folder_path("Select working directory")
            {
                settings.temp_working_directory = path.clone();
                settings.config.working_directory = Some(path);
                settings.has_changes = true;
            }
        });

        if ui
            .checkbox(&mut settings.config.login_shell, "Login shell (-l)")
            .on_hover_text("Spawn shell as login shell. This ensures PATH is properly initialized from /etc/paths, ~/.zprofile, etc. Recommended on macOS.")
            .changed()
        {
            settings.has_changes = true;
        }
        ui.separator();

        ui.label("Initial text to send when a session starts:");
        if ui
            .text_edit_multiline(&mut settings.temp_initial_text)
            .changed()
        {
            settings.config.initial_text = settings.temp_initial_text.clone();
            settings.has_changes = true;
        }

        ui.horizontal(|ui| {
            ui.label("Delay (ms):");
            if ui
                .add(
                    egui::DragValue::new(&mut settings.config.initial_text_delay_ms)
                        .range(0..=5000),
                )
                .changed()
            {
                settings.has_changes = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.initial_text_send_newline,
                    "Append newline after text",
                )
                .changed()
            {
                settings.has_changes = true;
            }
        });

        ui.label("Supports \\n, \\r, \\t, \\xHH, \\e escape sequences.");
    });

    ui.collapsing("Anti-Idle Keep-Alive", |ui| {
        ui.label("Prevents SSH and connection timeouts by periodically sending invisible characters.");
        ui.add_space(4.0);

        if ui
            .checkbox(
                &mut settings.config.anti_idle_enabled,
                "Send code when idle",
            )
            .on_hover_text("Periodically send a character to keep connections alive")
            .changed()
        {
            settings.has_changes = true;
            *_changes_this_frame = true;
        }

        ui.horizontal(|ui| {
            ui.label("Seconds before sending:");
            if ui
                .add(
                    egui::DragValue::new(&mut settings.config.anti_idle_seconds)
                        .range(10..=3600)
                        .speed(1.0),
                )
                .on_hover_text("How long to wait before sending keep-alive (10-3600 seconds)")
                .changed()
            {
                settings.has_changes = true;
                *_changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Character to send:");
            egui::ComboBox::from_id_salt("anti_idle_code")
                .selected_text(match settings.config.anti_idle_code {
                    0 => "NUL (0x00)",
                    5 => "ENQ (0x05)",
                    27 => "ESC (0x1B)",
                    32 => "Space (0x20)",
                    _ => "Custom",
                })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(&mut settings.config.anti_idle_code, 0, "NUL (0x00) - Null character, most common")
                        .changed()
                    {
                        settings.has_changes = true;
                        *_changes_this_frame = true;
                    }
                    if ui
                        .selectable_value(&mut settings.config.anti_idle_code, 27, "ESC (0x1B) - Escape, safe for most apps")
                        .changed()
                    {
                        settings.has_changes = true;
                        *_changes_this_frame = true;
                    }
                    if ui
                        .selectable_value(&mut settings.config.anti_idle_code, 5, "ENQ (0x05) - Enquiry, may trigger answerback")
                        .changed()
                    {
                        settings.has_changes = true;
                        *_changes_this_frame = true;
                    }
                    if ui
                        .selectable_value(&mut settings.config.anti_idle_code, 32, "Space (0x20) - Visible but harmless")
                        .changed()
                    {
                        settings.has_changes = true;
                        *_changes_this_frame = true;
                    }
                });
        });

        ui.horizontal(|ui| {
            ui.label("Custom ASCII code:");
            if ui
                .add(
                    egui::DragValue::new(&mut settings.config.anti_idle_code)
                        .range(0..=127)
                        .speed(1.0),
                )
                .on_hover_text("ASCII code (0-127) to send as keep-alive")
                .changed()
            {
                settings.has_changes = true;
                *_changes_this_frame = true;
            }
            ui.label(format!("(0x{:02X})", settings.config.anti_idle_code));
        });
    });
}

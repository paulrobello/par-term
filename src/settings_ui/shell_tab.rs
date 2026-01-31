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
    });

    ui.add_space(8.0);

    ui.collapsing("Terminal Identification", |ui| {
        ui.horizontal(|ui| {
            ui.label("Answerback string:");
            if ui
                .text_edit_singleline(&mut settings.config.answerback_string)
                .on_hover_text(
                    "String sent in response to ENQ (0x05) control character.\n\
                     Used for legacy terminal identification.\n\
                     Leave empty (default) for security.\n\
                     Common values: \"par-term\", \"vt100\"",
                )
                .changed()
            {
                settings.has_changes = true;
            }
        });

        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "⚠ Security: Setting this may expose terminal identification to applications",
                )
                .small()
                .color(egui::Color32::YELLOW),
            );
        });
    });
}

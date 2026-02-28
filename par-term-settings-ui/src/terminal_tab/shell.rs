//! Shell section for the terminal settings tab.
//!
//! Covers: custom shell, shell args, login shell, startup directory mode.

use crate::SettingsUI;
use crate::section::{INPUT_WIDTH, collapsing_section};
use std::collections::HashSet;

pub(super) fn show_shell_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Shell", "terminal_shell", true, collapsed, |ui| {
        ui.horizontal(|ui| {
            ui.label("Custom shell (optional):");
            if ui
                .add(
                    egui::TextEdit::singleline(&mut settings.temp_custom_shell)
                        .desired_width(INPUT_WIDTH),
                )
                .changed()
            {
                settings.config.custom_shell = if settings.temp_custom_shell.is_empty() {
                    None
                } else {
                    Some(settings.temp_custom_shell.clone())
                };
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui.button("Browse...").clicked()
                && let Some(path) = settings.pick_file_path("Select shell binary")
            {
                settings.temp_custom_shell = path.clone();
                settings.config.custom_shell = Some(path);
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Shell args (space-separated):");
            if ui
                .add(
                    egui::TextEdit::singleline(&mut settings.temp_shell_args)
                        .desired_width(INPUT_WIDTH),
                )
                .changed()
            {
                settings.config.shell_args = if settings.temp_shell_args.is_empty() {
                    None
                } else {
                    Some(
                        settings
                            .temp_shell_args
                            .split_whitespace()
                            .map(String::from)
                            .collect(),
                    )
                };
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(&mut settings.config.login_shell, "Login shell (-l)")
            .on_hover_text(
                "Spawn shell as login shell. This ensures PATH is properly initialized from /etc/paths, ~/.zprofile, etc. Recommended on macOS.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Startup Directory").strong());

        // Startup directory mode dropdown
        ui.horizontal(|ui| {
            ui.label("Mode:");
            let mode_text = settings.config.startup_directory_mode.display_name();
            egui::ComboBox::from_id_salt("startup_directory_mode")
                .selected_text(mode_text)
                .show_ui(ui, |ui| {
                    use par_term_config::StartupDirectoryMode;
                    for mode in StartupDirectoryMode::all() {
                        if ui
                            .selectable_value(
                                &mut settings.config.startup_directory_mode,
                                *mode,
                                mode.display_name(),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                })
                .response
                .on_hover_text(
                    "Controls where new terminal sessions start:\n\
                     • Home: Start in your home directory\n\
                     • Previous Session: Remember and restore the last working directory\n\
                     • Custom: Start in a specific directory",
                );
        });

        // Custom directory path (only shown when mode is Custom)
        if settings.config.startup_directory_mode == par_term_config::StartupDirectoryMode::Custom {
            ui.horizontal(|ui| {
                ui.label("Custom directory:");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut settings.temp_startup_directory)
                            .desired_width(INPUT_WIDTH),
                    )
                    .changed()
                {
                    settings.config.startup_directory =
                        if settings.temp_startup_directory.is_empty() {
                            None
                        } else {
                            Some(settings.temp_startup_directory.clone())
                        };
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if ui.button("Browse...").clicked()
                    && let Some(path) = settings.pick_folder_path("Select startup directory")
                {
                    settings.temp_startup_directory = path.clone();
                    settings.config.startup_directory = Some(path);
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }

        // Show last working directory info when in Previous mode
        if settings.config.startup_directory_mode == par_term_config::StartupDirectoryMode::Previous
        {
            if let Some(ref last_dir) = settings.config.last_working_directory {
                ui.label(
                    egui::RichText::new(format!("Last session: {}", last_dir))
                        .small()
                        .weak(),
                );
            } else {
                ui.label(
                    egui::RichText::new("No previous session directory saved yet")
                        .small()
                        .weak(),
                );
            }
        }
    });
}

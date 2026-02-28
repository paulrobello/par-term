//! Session Logging section for the advanced settings tab.
//!
//! Covers: auto-log enable, log format, log directory, archive on close, redact passwords.

use crate::SettingsUI;
use crate::section::collapsing_section;
use par_term_config::SessionLogFormat;
use std::collections::HashSet;

pub(super) fn show_logging_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Session Logging",
        "advanced_logging",
        true,
        collapsed,
        |ui| {
            ui.label(
                "Automatically record terminal sessions for later review, debugging, or sharing.",
            );
            ui.add_space(8.0);

            let mut auto_log = settings.config.auto_log_sessions;
            if ui
                .checkbox(&mut auto_log, "Enable automatic session logging")
                .on_hover_text("When enabled, all terminal output is logged to files")
                .changed()
            {
                settings.config.auto_log_sessions = auto_log;
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Log format:");

                let current_format = settings.config.session_log_format;
                let format_name = current_format.display_name();

                egui::ComboBox::from_id_salt("advanced_session_log_format")
                    .width(180.0)
                    .selected_text(format_name)
                    .show_ui(ui, |ui| {
                        for format in SessionLogFormat::all() {
                            if ui
                                .selectable_label(current_format == *format, format.display_name())
                                .clicked()
                                && current_format != *format
                            {
                                settings.config.session_log_format = *format;
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(match settings.config.session_log_format {
                    SessionLogFormat::Plain => {
                        "Plain text without escape sequences - smallest files"
                    }
                    SessionLogFormat::Html => "HTML with colors preserved - viewable in browser",
                    SessionLogFormat::Asciicast => "asciinema format - can be replayed or shared",
                })
                .weak(),
            );

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Log directory:");
                let mut dir = settings.config.session_log_directory.clone();
                let response = ui.add(
                    egui::TextEdit::singleline(&mut dir)
                        .desired_width(300.0)
                        .hint_text("~/.local/share/par-term/logs/"),
                );
                if response.changed() {
                    settings.config.session_log_directory = dir;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            let resolved_path = settings.config.logs_dir();
            ui.label(
                egui::RichText::new(format!("Resolved: {}", resolved_path.display()))
                    .weak()
                    .small(),
            );

            ui.add_space(8.0);

            let mut archive = settings.config.archive_on_close;
            if ui
                .checkbox(&mut archive, "Archive session on tab close")
                .on_hover_text("Ensures session is fully written when tab closes")
                .changed()
            {
                settings.config.archive_on_close = archive;
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(8.0);

            let mut redact = settings.config.session_log_redact_passwords;
            if ui
                .checkbox(&mut redact, "Redact passwords in session logs")
                .on_hover_text(
                    "Detects password prompts (sudo, ssh, etc.) and replaces \
                     keyboard input with a redaction marker. Prevents passwords \
                     from being written to session log files on disk.",
                )
                .changed()
            {
                settings.config.session_log_redact_passwords = redact;
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if !settings.config.session_log_redact_passwords {
                ui.label(
                    egui::RichText::new(
                        "\u{26a0} Warning: Session logs may contain passwords and credentials",
                    )
                    .color(egui::Color32::from_rgb(255, 193, 7))
                    .small(),
                );
            }
        },
    );
}

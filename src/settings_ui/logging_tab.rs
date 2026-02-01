//! Session logging settings tab.

use super::SettingsUI;
use crate::config::SessionLogFormat;

pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Session Logging", |ui| {
        ui.label("Automatically record terminal sessions for later review, debugging, or sharing.");
        ui.add_space(8.0);

        // Enable/disable auto logging
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

        // Log format selector
        ui.horizontal(|ui| {
            ui.label("Log format:");

            let current_format = settings.config.session_log_format;
            let format_name = current_format.display_name();

            egui::ComboBox::from_id_salt("session_log_format")
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

        // Format descriptions
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(match settings.config.session_log_format {
                SessionLogFormat::Plain => "Plain text without escape sequences - smallest files",
                SessionLogFormat::Html => "HTML with colors preserved - viewable in browser",
                SessionLogFormat::Asciicast => "asciinema format - can be replayed or shared",
            })
            .weak(),
        );

        ui.add_space(8.0);

        // Log directory
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

        // Show resolved path
        let resolved_path = settings.config.logs_dir();
        ui.label(
            egui::RichText::new(format!("Resolved: {}", resolved_path.display()))
                .weak()
                .small(),
        );

        ui.add_space(8.0);

        // Archive on close
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

        // Info about existing logs
        let logs_dir = settings.config.logs_dir();
        if logs_dir.exists()
            && let Ok(entries) = std::fs::read_dir(&logs_dir)
        {
            let log_count = entries
                .filter_map(Result::ok)
                .filter(|e| {
                    e.path()
                        .extension()
                        .is_some_and(|ext| ext == "cast" || ext == "txt" || ext == "html")
                })
                .count();
            if log_count > 0 {
                ui.label(
                    egui::RichText::new(format!("{} session log(s) in directory", log_count))
                        .weak(),
                );
            }
        }
    });
}

//! Status bar widget options section (time format, git status display).

use crate::SettingsUI;
use crate::section::collapsing_section;
use std::collections::HashSet;

pub fn show_widget_options_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Widget Options",
        "status_bar_widget_options",
        true,
        collapsed,
        |ui| {
            // Time format
            ui.horizontal(|ui| {
                ui.label("Time format:");
                if ui
                    .add(
                        egui::TextEdit::singleline(
                            &mut settings.config.status_bar.status_bar_time_format,
                        )
                        .hint_text("%H:%M:%S")
                        .desired_width(120.0),
                    )
                    .on_hover_text(
                        "strftime format string for the Clock widget (e.g. %H:%M:%S, %I:%M %p, %H:%M)",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Expandable format reference
            ui.collapsing("Format codes reference", |ui| {
                let dim = egui::Color32::from_rgb(140, 140, 140);
                let bright = egui::Color32::from_rgb(210, 210, 210);
                egui::Grid::new("time_format_help")
                    .num_columns(2)
                    .spacing([16.0, 2.0])
                    .show(ui, |ui| {
                        let rows: &[(&str, &str)] = &[
                            ("%H", "Hour 00–23"),
                            ("%I", "Hour 01–12"),
                            ("%M", "Minute 00–59"),
                            ("%S", "Second 00–59"),
                            ("%p", "AM / PM"),
                            ("%P", "am / pm"),
                            ("%Y", "Year (2026)"),
                            ("%m", "Month 01–12"),
                            ("%d", "Day 01–31"),
                            ("%a", "Weekday (Mon)"),
                            ("%A", "Weekday (Monday)"),
                            ("%b", "Month (Jan)"),
                            ("%B", "Month (January)"),
                            ("%Z", "Timezone (UTC)"),
                            ("%%", "Literal %"),
                        ];
                        for (code, desc) in rows {
                            ui.label(egui::RichText::new(*code).color(bright).monospace());
                            ui.label(egui::RichText::new(*desc).color(dim).small());
                            ui.end_row();
                        }
                    });
            });

            ui.add_space(8.0);

            // Git show status
            if ui
                .checkbox(
                    &mut settings.config.status_bar.status_bar_git_show_status,
                    "Show git ahead/behind and dirty status",
                )
                .on_hover_text(
                    "Display commit counts ahead/behind upstream and a dirty indicator on the Git Branch widget",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        },
    );
}

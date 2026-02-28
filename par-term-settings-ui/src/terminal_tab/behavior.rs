//! Behavior section for the terminal settings tab.
//!
//! Covers: scrollback lines, shell exit action, close confirmation, jobs to ignore.

use crate::SettingsUI;
use crate::section::{INPUT_WIDTH, SLIDER_WIDTH, collapsing_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub(super) fn show_behavior_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Behavior", "terminal_behavior", true, collapsed, |ui| {
        ui.horizontal(|ui| {
            ui.label("Scrollback lines:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.scrollback_lines, 1000..=100000),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Shell exit action:");
            egui::ComboBox::from_id_salt("shell_exit_action")
                .selected_text(settings.config.shell_exit_action.display_name())
                .show_ui(ui, |ui| {
                    for action in par_term_config::ShellExitAction::all() {
                        if ui
                            .selectable_value(
                                &mut settings.config.shell_exit_action,
                                *action,
                                action.display_name(),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Close Confirmation").strong());

        if ui
            .checkbox(
                &mut settings.config.prompt_on_quit,
                "Confirm before quitting with open sessions",
            )
            .on_hover_text(
                "When enabled, closing the window will show a confirmation dialog\n\
                 if there are any open terminal sessions.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.confirm_close_running_jobs,
                "Confirm before closing tabs with running jobs",
            )
            .on_hover_text(
                "When enabled, closing a tab with a running command will show a confirmation dialog.\n\
                 Requires shell integration to detect running commands.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        // Jobs to ignore list (only shown when confirmation is enabled)
        if settings.config.confirm_close_running_jobs {
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("Jobs to ignore (won't trigger confirmation):").small(),
                    );
                    ui.horizontal(|ui| {
                        // Show current list as comma-separated
                        let mut jobs_text = settings.config.jobs_to_ignore.join(", ");
                        let response = ui
                            .add(
                                egui::TextEdit::singleline(&mut jobs_text)
                                    .desired_width(INPUT_WIDTH)
                                    .hint_text("bash, zsh, cat, sleep"),
                            )
                            .on_hover_text(
                                "Comma-separated list of process names.\n\
                                 These processes won't trigger the close confirmation.\n\
                                 Common shells and pagers are ignored by default.",
                            );
                        if response.changed() {
                            // Parse comma-separated list
                            settings.config.jobs_to_ignore = jobs_text
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });

                    // Reset to defaults button
                    if ui
                        .small_button("Reset to defaults")
                        .on_hover_text("Restore the default list of ignored jobs")
                        .clicked()
                    {
                        settings.config.jobs_to_ignore =
                            par_term_config::defaults::jobs_to_ignore();
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            });
        }
    });
}

//! Behavior section for the terminal settings tab.
//!
//! Covers: scrollback lines, shell exit action, close confirmation, jobs to ignore.

use crate::SettingsUI;
use crate::search::SearchTag;
use crate::section::{INPUT_WIDTH, SLIDER_WIDTH, keyword_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub(super) fn show_behavior_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "Behavior",
        "terminal_behavior",
        &["process names"],
        true,
        collapsed,
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Scrollback lines:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.scrollback.scrollback_lines,
                            1000..=100000,
                        )
                        .suffix(" lines"),
                    )
                    .search_tag(&["scrollback_lines"])
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                    &mut c.scrollback.scrollback_lines
                });
            });

            ui.horizontal(|ui| {
                ui.label("Shell exit action:");
                egui::ComboBox::from_id_salt("shell_exit_action")
                    .selected_text(settings.config.shell.shell_exit_action.display_name())
                    .show_ui(ui, |ui| {
                        for action in par_term_config::ShellExitAction::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.shell.shell_exit_action,
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
                    &mut settings.config.shell.prompt_on_quit,
                    "Confirm before quitting with open tabs",
                )
                .search_tag(&["prompt_on_quit"])
                .on_hover_text(
                    "When enabled, closing the window shows a confirmation dialog\n\
                 if there are any open tabs. Only applies when the\n\
                 multi-tab guard below is off — with it on, only a multi-tab\n\
                 window close asks.",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.shell.confirm_close_multiple_tabs,
                    "Confirm before closing a window with multiple tabs",
                )
                .search_tag(&["confirm_close_multiple_tabs"])
                .on_hover_text(
                    "When enabled, closing a window that holds more than one tab asks first.\n\
                 A single-tab window closes silently — Reopen Closed Tab restores it\n\
                 (with its running processes) within the reopen-closed-tab window.",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
            .checkbox(
                &mut settings.config.shell.confirm_close_running_jobs,
                "Confirm before closing tabs with running jobs",
            ).search_tag(&["confirm_close_running_jobs"])
            .on_hover_text(
                "When enabled, closing a tab with a running command will show a confirmation dialog.\n\
                 Requires shell integration to detect running commands.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

            crate::dependent::dependent(
                ui,
                settings.config.shell.confirm_close_running_jobs,
                "Confirm before closing tabs with running jobs",
                |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("Jobs to ignore (won't trigger confirmation):")
                                    .small(),
                            );
                            ui.horizontal(|ui| {
                                // Show current list as comma-separated
                                let mut jobs_text = settings.config.shell.jobs_to_ignore.join(", ");
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
                                    settings.config.shell.jobs_to_ignore = jobs_text
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
                                settings.config.shell.jobs_to_ignore =
                                    par_term_config::defaults::jobs_to_ignore();
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });
                    });
                },
            );
        },
    );
}

//! Scrollback (Advanced › Terminal Emulation) and Closing & Quitting
//! (General › Closing & Quitting, UX.md SX2).
//!
//! Both were one "Terminal › Behavior" section; its id, `terminal_behavior`,
//! is kept by the scrollback section so a persisted collapse state still
//! applies.

use crate::SettingsUI;
use crate::search::SearchTag;
use crate::section::{INPUT_WIDTH, SLIDER_WIDTH, keyword_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub(crate) fn show_scrollback_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "Scrollback",
        "terminal_behavior",
        &["history buffer"],
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
        },
    );
}

/// General › Closing & Quitting (UX.md SX2): quit and close confirmation,
/// jobs to ignore, what happens when the shell exits, and reopen closed
/// tab.
pub(crate) fn show_closing_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "Closing & Quitting",
        "general_closing",
        &[
            "process names",
            "close confirmation",
            "undo close",
            "undo timeout",
            "preserve shell",
        ],
        true,
        collapsed,
        |ui| {
            show_close_confirmation(ui, settings, changes_this_frame);
            ui.add_space(8.0);
            show_shell_exit(ui, settings, changes_this_frame);
            ui.add_space(8.0);
            show_reopen_closed_tab(ui, settings, changes_this_frame);
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(
                    "Closing the last tab of an attached par-mux session always asks \
                     whether to detach or end the session.",
                )
                .small()
                .weak(),
            );
        },
    );
}

fn show_close_confirmation(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
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
        )
        .search_tag(&["confirm_close_running_jobs"])
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
                        egui::RichText::new("Jobs to ignore (won't trigger confirmation):").small(),
                    );
                    ui.horizontal(|ui| {
                        let mut jobs_text = settings.config.shell.jobs_to_ignore.join(", ");
                        let response = ui
                            .add(
                                egui::TextEdit::singleline(&mut jobs_text)
                                    .desired_width(INPUT_WIDTH)
                                    .hint_text("bash, zsh, cat, sleep"),
                            )
                            .search_tag(&["jobs_to_ignore"])
                            .on_hover_text(
                                "Comma-separated list of process names.\n\
                                 These processes won't trigger the close confirmation.\n\
                                 Common shells and pagers are ignored by default.",
                            );
                        if response.changed() {
                            settings.config.shell.jobs_to_ignore = jobs_text
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });

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
}

fn show_shell_exit(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
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
            })
            .response
            .search_tag(&["shell_exit_action"]);
    });
}

fn show_reopen_closed_tab(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    ui.label(egui::RichText::new("Reopen Closed Tab").strong());
    ui.horizontal(|ui| {
        ui.label("Reopen closed tab for:");
        if ui
            .add(
                egui::DragValue::new(
                    &mut settings.config.session_restore.session_undo_timeout_secs,
                )
                .range(0..=60)
                .suffix(" s"),
            )
            .search_tag(&["session_undo_timeout_secs"])
            .on_hover_text(
                "How long closed tab metadata is kept for undo (reopen).\n\
                 Set to 0 to disable the feature entirely.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
        crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
            &mut c.session_restore.session_undo_timeout_secs
        });

        ui.label("Max entries:");
        if ui
            .add(
                egui::DragValue::new(&mut settings.config.session_restore.session_undo_max_entries)
                    .suffix(" tabs")
                    .range(1..=50),
            )
            .search_tag(&["session_undo_max_entries"])
            .on_hover_text("Maximum number of closed tabs to remember for undo.")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
        crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
            &mut c.session_restore.session_undo_max_entries
        });
    });

    if ui
        .checkbox(
            &mut settings.config.session_restore.session_undo_preserve_shell,
            "Keep the shell running while a closed tab can be reopened",
        )
        .search_tag(&["session_undo_preserve_shell"])
        .on_hover_text(
            "When enabled, closing a tab hides the shell instead of killing it.\n\
             Reopening restores the tab with its scrollback and running processes.\n\
             Uses more memory while hidden tabs are kept alive.",
        )
        .changed()
    {
        settings.has_changes = true;
        *changes_this_frame = true;
    }
}

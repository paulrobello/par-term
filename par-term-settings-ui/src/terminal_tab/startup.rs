//! Startup section (General › Startup & Restore).
//!
//! Covers: restore windows on launch, initial text, delay, newline. Reopen
//! closed tab lives on General › Closing & Quitting.

use crate::SettingsUI;
use crate::search::SearchTag;
use crate::section::keyword_section;
use std::collections::HashSet;

pub(crate) fn show_startup_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "Startup",
        "terminal_startup",
        &["launch"],
        true,
        collapsed,
        |ui| {
            if ui
                .checkbox(
                    &mut settings.config.session_restore.restore_session,
                    "Restore windows on launch",
                )
                .search_tag(&["restore_session"])
                .on_hover_text(
                    "When enabled, par-term will save your open tabs, pane layouts, and working\n\
                 directories when closing and restore them on next launch.\n\
                 An arrangement set to auto-restore below takes precedence.",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(8.0);
            ui.label("Initial text to send when a new shell starts:");
            if ui
                .text_edit_multiline(&mut settings.temp_initial_text)
                .search_tag(&["initial_text"])
                .changed()
            {
                settings.config.shell.initial_text = settings.temp_initial_text.clone();
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.horizontal(|ui| {
                ui.label("Delay:");
                if ui
                    .add(
                        egui::DragValue::new(&mut settings.config.shell.initial_text_delay_ms)
                            .suffix(" ms")
                            .range(0..=5000),
                    )
                    .search_tag(&["initial_text_delay_ms"])
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                    &mut c.shell.initial_text_delay_ms
                });

                if ui
                    .checkbox(
                        &mut settings.config.shell.initial_text_send_newline,
                        "Append newline after text",
                    )
                    .search_tag(&["initial_text_send_newline"])
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.label(
                egui::RichText::new("Supports \\n, \\r, \\t, \\xHH, \\e escape sequences.")
                    .small()
                    .weak(),
            );
        },
    );
}

//! Startup section for the terminal settings tab.
//!
//! Covers: restore session, undo close tab, initial text, delay, newline.

use crate::SettingsUI;
use crate::section::collapsing_section;
use std::collections::HashSet;

pub(super) fn show_startup_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Startup", "terminal_startup", false, collapsed, |ui| {
        if ui
            .checkbox(
                &mut settings.config.restore_session,
                "Restore previous session on startup",
            )
            .on_hover_text(
                "When enabled, par-term will save your open tabs, pane layouts, and working\n\
                 directories when closing and restore them on next launch.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Undo close tab timeout:");
            if ui
                .add(
                    egui::DragValue::new(&mut settings.config.session_undo_timeout_secs)
                        .range(0..=60)
                        .suffix("s"),
                )
                .on_hover_text(
                    "How long closed tab metadata is kept for undo (reopen).\n\
                     Set to 0 to disable the feature entirely.",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.label("Max entries:");
            if ui
                .add(
                    egui::DragValue::new(&mut settings.config.session_undo_max_entries)
                        .range(1..=50),
                )
                .on_hover_text("Maximum number of closed tabs to remember for undo.")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.session_undo_preserve_shell,
                "Preserve shell session on close",
            )
            .on_hover_text(
                "When enabled, closing a tab hides the shell instead of killing it.\n\
                 Undo restores the full session with scrollback and running processes.\n\
                 Uses more memory while hidden tabs are kept alive.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);
        ui.label("Initial text to send when a session starts:");
        if ui
            .text_edit_multiline(&mut settings.temp_initial_text)
            .changed()
        {
            settings.config.initial_text = settings.temp_initial_text.clone();
            settings.has_changes = true;
            *changes_this_frame = true;
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
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.initial_text_send_newline,
                    "Append newline after text",
                )
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
    });
}

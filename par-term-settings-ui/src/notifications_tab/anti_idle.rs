//! Anti-idle keep-alive settings — prevents SSH/connection timeouts.

use crate::SettingsUI;
use crate::section::collapsing_section;
use std::collections::HashSet;

pub(super) fn show_anti_idle_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Anti-Idle Keep-Alive",
        "notifications_anti_idle",
        false,
        collapsed,
        |ui| {
            ui.label(
                "Prevents SSH and connection timeouts by periodically sending invisible characters.",
            );
            ui.add_space(4.0);

            if ui
                .checkbox(
                    &mut settings.config.notifications.anti_idle_enabled,
                    "Send code when idle",
                )
                .on_hover_text("Periodically send a character to keep connections alive")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.horizontal(|ui| {
                ui.label("Seconds before sending:");
                if ui
                    .add(
                        egui::DragValue::new(&mut settings.config.notifications.anti_idle_seconds)
                            .range(10..=3600)
                            .speed(1.0),
                    )
                    .on_hover_text("How long to wait before sending keep-alive (10-3600 seconds)")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Character to send:");
                egui::ComboBox::from_id_salt("notifications_anti_idle_code")
                    .selected_text(match settings.config.notifications.anti_idle_code {
                        0 => "NUL (0x00)",
                        5 => "ENQ (0x05)",
                        27 => "ESC (0x1B)",
                        32 => "Space (0x20)",
                        _ => "Custom",
                    })
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_value(
                                &mut settings.config.notifications.anti_idle_code,
                                0,
                                "NUL (0x00) - Null character, most common",
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        if ui
                            .selectable_value(
                                &mut settings.config.notifications.anti_idle_code,
                                27,
                                "ESC (0x1B) - Escape, safe for most apps",
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        if ui
                            .selectable_value(
                                &mut settings.config.notifications.anti_idle_code,
                                5,
                                "ENQ (0x05) - Enquiry, may trigger answerback",
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        if ui
                            .selectable_value(
                                &mut settings.config.notifications.anti_idle_code,
                                32,
                                "Space (0x20) - Visible but harmless",
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });
            });

            ui.horizontal(|ui| {
                ui.label("Custom ASCII code:");
                if ui
                    .add(
                        egui::DragValue::new(&mut settings.config.notifications.anti_idle_code)
                            .range(0..=127)
                            .speed(1.0),
                    )
                    .on_hover_text("ASCII code (0-127) to send as keep-alive")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                ui.label(format!(
                    "(0x{:02X})",
                    settings.config.notifications.anti_idle_code
                ));
            });
        },
    );
}

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

            crate::dependent::dependent(
                ui,
                settings.config.notifications.anti_idle_enabled,
                "Send code when idle",
                |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Send after:");
                        if ui
                            .add(
                                egui::DragValue::new(
                                    &mut settings.config.notifications.anti_idle_seconds,
                                )
                                .range(10..=3600)
                                .speed(1.0)
                                .suffix(" s"),
                            )
                            .on_hover_text("Idle time before the keep-alive is sent (10-3600 s)")
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                            &mut c.notifications.anti_idle_seconds
                        });
                    });
                    show_anti_idle_code(ui, settings, changes_this_frame);
                },
            );
        },
    );
}

/// Named keep-alive characters offered in the picker.
const NAMED_CODES: &[(u8, &str, &str)] = &[
    (0, "NUL (0x00)", "Null character, most common"),
    (27, "ESC (0x1B)", "Escape, safe for most apps"),
    (5, "ENQ (0x05)", "Enquiry, may trigger answerback"),
    (32, "Space (0x20)", "Visible but harmless"),
];

/// One control for `anti_idle_code` (UX.md B50): a picker of named
/// characters plus "Custom", which shows the ASCII value field. Before, a
/// combo and an always-visible number both wrote the same field.
fn show_anti_idle_code(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    let code = &mut settings.config.notifications.anti_idle_code;
    let named = NAMED_CODES.iter().find(|(c, _, _)| *c == *code);
    let mut custom = named.is_none() || settings.advanced_tab.anti_idle_custom_code;
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Character to send:");
        egui::ComboBox::from_id_salt("notifications_anti_idle_code")
            .selected_text(match (custom, named) {
                (false, Some((_, name, _))) => (*name).to_string(),
                _ => format!("Custom (0x{code:02X})"),
            })
            .show_ui(ui, |ui| {
                for (value, name, hint) in NAMED_CODES {
                    if ui
                        .selectable_label(!custom && *code == *value, *name)
                        .on_hover_text(*hint)
                        .clicked()
                    {
                        *code = *value;
                        custom = false;
                        changed = true;
                    }
                }
                if ui.selectable_label(custom, "Custom…").clicked() {
                    custom = true;
                }
            });
        if custom
            && ui
                .add(egui::DragValue::new(code).range(0..=127).speed(1.0))
                .on_hover_text("ASCII code (0-127) to send as the keep-alive")
                .changed()
        {
            changed = true;
        }
    });
    settings.advanced_tab.anti_idle_custom_code = custom;
    if changed {
        settings.has_changes = true;
        *changes_this_frame = true;
    }
}

//! Activity, silence, and session notification settings.

use crate::SettingsUI;
use crate::section::{SLIDER_WIDTH, collapsing_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub(super) fn show_activity_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Activity",
        "notifications_activity",
        true,
        collapsed,
        |ui| {
            ui.label("Activity Notifications:");
            if ui
                .checkbox(
                    &mut settings.config.notifications.notification_activity_enabled,
                    "Notify on activity after inactivity",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            crate::dependent::dependent(
                ui,
                settings.config.notifications.notification_activity_enabled,
                "Notify on activity after inactivity",
                |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Activity threshold:");
                        if ui
                            .add_sized(
                                [SLIDER_WIDTH, SLIDER_HEIGHT],
                                egui::Slider::new(
                                    &mut settings
                                        .config
                                        .notifications
                                        .notification_activity_threshold,
                                    1..=300,
                                )
                                .suffix(" s"),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                            &mut c.notifications.notification_activity_threshold
                        });
                    });
                },
            );
            ui.separator();
            ui.label("Silence Notifications:");
            if ui
                .checkbox(
                    &mut settings.config.notifications.notification_silence_enabled,
                    "Notify after prolonged silence",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            crate::dependent::dependent(
                ui,
                settings.config.notifications.notification_silence_enabled,
                "Notify after prolonged silence",
                |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Silence threshold:");
                        if ui
                            .add_sized(
                                [SLIDER_WIDTH, SLIDER_HEIGHT],
                                egui::Slider::new(
                                    &mut settings
                                        .config
                                        .notifications
                                        .notification_silence_threshold,
                                    1..=600,
                                )
                                .suffix(" s"),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                            &mut c.notifications.notification_silence_threshold
                        });
                    });
                },
            );
            ui.separator();
            ui.label("Shell Exit Notifications:");
            if ui
                .checkbox(
                    &mut settings.config.notifications.notification_session_ended,
                    "Notify when a shell exits",
                )
                .on_hover_text("Send a desktop notification when the shell process exits")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        },
    );
}

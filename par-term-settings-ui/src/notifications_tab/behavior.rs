//! Notification behavior settings — suppression, buffer, and test notification.

use crate::SettingsUI;
use crate::section::{SLIDER_WIDTH, collapsing_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub(super) fn show_behavior_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Behavior",
        "notifications_behavior",
        false,
        collapsed,
        |ui| {
            if ui
                .checkbox(
                    &mut settings.config.suppress_notifications_when_focused,
                    "Suppress notifications when focused",
                )
                .on_hover_text("Skip desktop notifications when the terminal window is focused")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.horizontal(|ui| {
                ui.label("Max notification buffer:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(&mut settings.config.notification_max_buffer, 10..=1000),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .button("Test Notification")
                    .on_hover_text("Send a test notification to verify permissions are granted")
                    .clicked()
                {
                    settings.test_notification_requested = true;
                }
                #[cfg(target_os = "macos")]
                {
                    if ui
                        .button("Open System Preferences")
                        .on_hover_text("Open macOS notification settings")
                        .clicked()
                    {
                        let _ = std::process::Command::new("open")
                            .arg("x-apple.systempreferences:com.apple.preference.notifications")
                            .spawn();
                    }
                }
            });
        },
    );
}

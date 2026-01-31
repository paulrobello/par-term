use super::SettingsUI;

pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Bell & Notifications", |ui| {
        ui.label("Bell Settings:");
        if ui
            .checkbox(&mut settings.config.notification_bell_visual, "Visual bell")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.horizontal(|ui| {
            ui.label("Audio bell volume (0=off):");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.notification_bell_sound,
                    0..=100,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.notification_bell_desktop,
                "Desktop notifications for bell",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.separator();
        ui.label("Activity Notifications:");
        if ui
            .checkbox(
                &mut settings.config.notification_activity_enabled,
                "Notify on activity after inactivity",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.horizontal(|ui| {
            ui.label("Activity threshold (seconds):");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.notification_activity_threshold,
                    1..=300,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.separator();
        ui.label("Silence Notifications:");
        if ui
            .checkbox(
                &mut settings.config.notification_silence_enabled,
                "Notify after prolonged silence",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.horizontal(|ui| {
            ui.label("Silence threshold (seconds):");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.notification_silence_threshold,
                    1..=600,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.separator();
        ui.label("Session Notifications:");
        if ui
            .checkbox(
                &mut settings.config.notification_session_ended,
                "Notify when session/shell exits",
            )
            .on_hover_text("Send a desktop notification when the shell process exits")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.separator();
        ui.label("Notification Behavior:");
        if ui
            .checkbox(
                &mut settings.config.suppress_notifications_when_focused,
                "Suppress notifications when focused",
            )
            .on_hover_text(
                "Skip desktop notifications when the terminal window is focused (you're already looking at it)",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

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

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Max notification buffer:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.notification_max_buffer,
                    10..=1000,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}

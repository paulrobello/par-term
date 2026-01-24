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

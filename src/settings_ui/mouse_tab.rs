use super::SettingsUI;

pub fn show_selection(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Selection & Clipboard", |ui| {
        if ui
            .checkbox(
                &mut settings.config.auto_copy_selection,
                "Auto-copy selection",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.copy_trailing_newline,
                "Include trailing newline when copying",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(&mut settings.config.middle_click_paste, "Middle-click paste")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.horizontal(|ui| {
            ui.label("Max clipboard sync events:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.clipboard_max_sync_events,
                    8..=256,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Max clipboard event bytes:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.clipboard_max_event_bytes,
                    512..=16384,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}

pub fn show_mouse_behavior(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Mouse Behavior", |ui| {
        ui.horizontal(|ui| {
            ui.label("Scroll speed:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.mouse_scroll_speed,
                    0.1..=10.0,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Double-click threshold (ms):");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.mouse_double_click_threshold,
                    100..=1000,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Triple-click threshold (ms):");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.mouse_triple_click_threshold,
                    100..=1000,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}
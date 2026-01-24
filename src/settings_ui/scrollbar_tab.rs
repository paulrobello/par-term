use super::SettingsUI;

pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Scrollbar", |ui| {
        ui.horizontal(|ui| {
            ui.label("Width:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.scrollbar_width,
                    4.0..=50.0,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Autohide delay (ms, 0=never):");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.scrollbar_autohide_delay,
                    0..=5000,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Position:");
            ui.label("Right (only)");
        });

        ui.horizontal(|ui| {
            ui.label("Thumb color:");
            let mut thumb = egui::Color32::from_rgba_unmultiplied(
                (settings.config.scrollbar_thumb_color[0] * 255.0) as u8,
                (settings.config.scrollbar_thumb_color[1] * 255.0) as u8,
                (settings.config.scrollbar_thumb_color[2] * 255.0) as u8,
                (settings.config.scrollbar_thumb_color[3] * 255.0) as u8,
            );
            if egui::color_picker::color_edit_button_srgba(
                ui,
                &mut thumb,
                egui::color_picker::Alpha::Opaque,
            )
            .changed()
            {
                settings.config.scrollbar_thumb_color = [
                    thumb.r() as f32 / 255.0,
                    thumb.g() as f32 / 255.0,
                    thumb.b() as f32 / 255.0,
                    thumb.a() as f32 / 255.0,
                ];
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Track color:");
            let mut track = egui::Color32::from_rgba_unmultiplied(
                (settings.config.scrollbar_track_color[0] * 255.0) as u8,
                (settings.config.scrollbar_track_color[1] * 255.0) as u8,
                (settings.config.scrollbar_track_color[2] * 255.0) as u8,
                (settings.config.scrollbar_track_color[3] * 255.0) as u8,
            );
            if egui::color_picker::color_edit_button_srgba(
                ui,
                &mut track,
                egui::color_picker::Alpha::Opaque,
            )
            .changed()
            {
                settings.config.scrollbar_track_color = [
                    track.r() as f32 / 255.0,
                    track.g() as f32 / 255.0,
                    track.b() as f32 / 255.0,
                    track.a() as f32 / 255.0,
                ];
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}

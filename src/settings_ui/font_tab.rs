use super::SettingsUI;

pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Font", |ui| {
        ui.horizontal(|ui| {
            ui.label("Family (regular):");
            if ui
                .text_edit_singleline(&mut settings.temp_font_family)
                .changed()
            {
                settings.font_pending_changes = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Bold font (optional):");
            if ui
                .text_edit_singleline(&mut settings.temp_font_bold)
                .changed()
            {
                settings.font_pending_changes = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Italic font (optional):");
            if ui
                .text_edit_singleline(&mut settings.temp_font_italic)
                .changed()
            {
                settings.font_pending_changes = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Bold-Italic font (optional):");
            if ui
                .text_edit_singleline(&mut settings.temp_font_bold_italic)
                .changed()
            {
                settings.font_pending_changes = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Size:");
            if ui
                .add(egui::Slider::new(&mut settings.temp_font_size, 6.0..=48.0))
                .changed()
            {
                settings.font_pending_changes = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Line spacing:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.temp_line_spacing,
                    0.8..=2.0,
                ))
                .changed()
            {
                settings.font_pending_changes = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Char spacing:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.temp_char_spacing,
                    0.5..=1.0,
                ))
                .changed()
            {
                settings.font_pending_changes = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.temp_enable_text_shaping,
                "Enable text shaping",
            )
            .changed()
        {
            settings.font_pending_changes = true;
        }

        if ui
            .checkbox(&mut settings.temp_enable_ligatures, "Enable ligatures")
            .changed()
        {
            settings.font_pending_changes = true;
        }

        if ui
            .checkbox(&mut settings.temp_enable_kerning, "Enable kerning")
            .changed()
        {
            settings.font_pending_changes = true;
        }

        ui.horizontal(|ui| {
            if ui.button("Apply font changes").clicked() {
                settings.apply_font_changes();
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            if settings.font_pending_changes {
                ui.colored_label(egui::Color32::YELLOW, "(pending)");
            }
        });
    });
}

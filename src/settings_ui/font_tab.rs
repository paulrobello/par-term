use super::SettingsUI;
use crate::config::ThinStrokesMode;

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

        ui.separator();
        ui.label("Rendering Options");

        if ui
            .checkbox(
                &mut settings.config.font_antialias,
                "Anti-aliasing",
            )
            .on_hover_text("Enable smooth font edges. Disable for crisp, pixelated text.")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.font_hinting,
                "Hinting",
            )
            .on_hover_text("Align glyphs to pixel boundaries for sharper text at small sizes.")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.horizontal(|ui| {
            ui.label("Thin strokes:");
            let current_mode = settings.config.font_thin_strokes;
            let mode_label = match current_mode {
                ThinStrokesMode::Never => "Never",
                ThinStrokesMode::RetinaOnly => "Retina Only",
                ThinStrokesMode::DarkBackgroundsOnly => "Dark Backgrounds Only",
                ThinStrokesMode::RetinaDarkBackgroundsOnly => "Retina + Dark BG",
                ThinStrokesMode::Always => "Always",
            };

            egui::ComboBox::from_id_salt("thin_strokes_mode")
                .selected_text(mode_label)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(
                        current_mode == ThinStrokesMode::Never,
                        "Never",
                    ).clicked() {
                        settings.config.font_thin_strokes = ThinStrokesMode::Never;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                    if ui.selectable_label(
                        current_mode == ThinStrokesMode::RetinaOnly,
                        "Retina Only",
                    ).clicked() {
                        settings.config.font_thin_strokes = ThinStrokesMode::RetinaOnly;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                    if ui.selectable_label(
                        current_mode == ThinStrokesMode::DarkBackgroundsOnly,
                        "Dark Backgrounds Only",
                    ).clicked() {
                        settings.config.font_thin_strokes = ThinStrokesMode::DarkBackgroundsOnly;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                    if ui.selectable_label(
                        current_mode == ThinStrokesMode::RetinaDarkBackgroundsOnly,
                        "Retina + Dark BG",
                    ).clicked() {
                        settings.config.font_thin_strokes = ThinStrokesMode::RetinaDarkBackgroundsOnly;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                    if ui.selectable_label(
                        current_mode == ThinStrokesMode::Always,
                        "Always",
                    ).clicked() {
                        settings.config.font_thin_strokes = ThinStrokesMode::Always;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
        });
        ui.label("  Lighter font strokes for improved readability on HiDPI displays.")
            .on_hover_text("Similar to macOS font smoothing. Works best on Retina/HiDPI displays with dark backgrounds.");

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

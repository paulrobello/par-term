use super::SettingsUI;

pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Terminal", |ui| {
        ui.horizontal(|ui| {
            ui.label("Scrollback lines:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.scrollback_lines,
                    1000..=100000,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.exit_on_shell_exit,
                "Exit when shell exits",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.separator();
        ui.label(egui::RichText::new("Unicode Width").strong());
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Unicode version:");
            let version_text = match settings.config.unicode_version {
                par_term_emu_core_rust::UnicodeVersion::Unicode9 => "9.0",
                par_term_emu_core_rust::UnicodeVersion::Unicode10 => "10.0",
                par_term_emu_core_rust::UnicodeVersion::Unicode11 => "11.0",
                par_term_emu_core_rust::UnicodeVersion::Unicode12 => "12.0",
                par_term_emu_core_rust::UnicodeVersion::Unicode13 => "13.0",
                par_term_emu_core_rust::UnicodeVersion::Unicode14 => "14.0",
                par_term_emu_core_rust::UnicodeVersion::Unicode15 => "15.0",
                par_term_emu_core_rust::UnicodeVersion::Unicode15_1 => "15.1",
                par_term_emu_core_rust::UnicodeVersion::Unicode16 => "16.0",
                par_term_emu_core_rust::UnicodeVersion::Auto => "Auto (latest)",
            };
            egui::ComboBox::from_id_salt("unicode_version")
                .selected_text(version_text)
                .show_ui(ui, |ui| {
                    let versions = [
                        (
                            par_term_emu_core_rust::UnicodeVersion::Auto,
                            "Auto (latest)",
                        ),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode16, "16.0"),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode15_1, "15.1"),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode15, "15.0"),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode14, "14.0"),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode13, "13.0"),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode12, "12.0"),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode11, "11.0"),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode10, "10.0"),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode9, "9.0"),
                    ];
                    for (value, label) in versions {
                        if ui
                            .selectable_value(&mut settings.config.unicode_version, value, label)
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                })
                .response
                .on_hover_text(
                    "Unicode version for character width calculations.\n\
                     Different versions have different width tables for emoji.\n\
                     Use older versions for compatibility with legacy systems.",
                );
        });

        ui.horizontal(|ui| {
            ui.label("Ambiguous width:");
            let width_text = match settings.config.ambiguous_width {
                par_term_emu_core_rust::AmbiguousWidth::Narrow => "Narrow (1 cell)",
                par_term_emu_core_rust::AmbiguousWidth::Wide => "Wide (2 cells)",
            };
            egui::ComboBox::from_id_salt("ambiguous_width")
                .selected_text(width_text)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(
                            &mut settings.config.ambiguous_width,
                            par_term_emu_core_rust::AmbiguousWidth::Narrow,
                            "Narrow (1 cell)",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                    if ui
                        .selectable_value(
                            &mut settings.config.ambiguous_width,
                            par_term_emu_core_rust::AmbiguousWidth::Wide,
                            "Wide (2 cells)",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                })
                .response
                .on_hover_text(
                    "Treatment of East Asian Ambiguous width characters.\n\
                     - Narrow: 1 cell (Western default)\n\
                     - Wide: 2 cells (CJK default, use for Chinese/Japanese/Korean)",
                );
        });

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Answerback string:");
            if ui
                .text_edit_singleline(&mut settings.config.answerback_string)
                .on_hover_text(
                    "String sent in response to ENQ (0x05) control character.\n\
                     Used for legacy terminal identification.\n\
                     Leave empty (default) for security.\n\
                     Common values: \"par-term\", \"vt100\"",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "⚠ Security: Setting this may expose terminal identification to applications",
                )
                .small()
                .color(egui::Color32::YELLOW),
            );
        });
    });
}

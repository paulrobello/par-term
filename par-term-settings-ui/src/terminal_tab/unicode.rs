//! Unicode section for the terminal settings tab.
//!
//! Covers: unicode version, ambiguous width, normalization form, answerback string.

use crate::SettingsUI;
use crate::section::collapsing_section;
use std::collections::HashSet;

pub(super) fn show_unicode_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Unicode", "terminal_unicode", false, collapsed, |ui| {
        ui.horizontal(|ui| {
            ui.label("Unicode version:");
            let version_text = match settings.config.unicode_version {
                par_term_config::UnicodeVersion::Unicode9 => "9.0",
                par_term_config::UnicodeVersion::Unicode10 => "10.0",
                par_term_config::UnicodeVersion::Unicode11 => "11.0",
                par_term_config::UnicodeVersion::Unicode12 => "12.0",
                par_term_config::UnicodeVersion::Unicode13 => "13.0",
                par_term_config::UnicodeVersion::Unicode14 => "14.0",
                par_term_config::UnicodeVersion::Unicode15 => "15.0",
                par_term_config::UnicodeVersion::Unicode15_1 => "15.1",
                par_term_config::UnicodeVersion::Unicode16 => "16.0",
                par_term_config::UnicodeVersion::Auto => "Auto (latest)",
            };
            egui::ComboBox::from_id_salt("terminal_unicode_version")
                .selected_text(version_text)
                .show_ui(ui, |ui| {
                    let versions = [
                        (par_term_config::UnicodeVersion::Auto, "Auto (latest)"),
                        (par_term_config::UnicodeVersion::Unicode16, "16.0"),
                        (par_term_config::UnicodeVersion::Unicode15_1, "15.1"),
                        (par_term_config::UnicodeVersion::Unicode15, "15.0"),
                        (par_term_config::UnicodeVersion::Unicode14, "14.0"),
                        (par_term_config::UnicodeVersion::Unicode13, "13.0"),
                        (par_term_config::UnicodeVersion::Unicode12, "12.0"),
                        (par_term_config::UnicodeVersion::Unicode11, "11.0"),
                        (par_term_config::UnicodeVersion::Unicode10, "10.0"),
                        (par_term_config::UnicodeVersion::Unicode9, "9.0"),
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
                par_term_config::AmbiguousWidth::Narrow => "Narrow (1 cell)",
                par_term_config::AmbiguousWidth::Wide => "Wide (2 cells)",
            };
            egui::ComboBox::from_id_salt("terminal_ambiguous_width")
                .selected_text(width_text)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(
                            &mut settings.config.ambiguous_width,
                            par_term_config::AmbiguousWidth::Narrow,
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
                            par_term_config::AmbiguousWidth::Wide,
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

        ui.horizontal(|ui| {
            ui.label("Normalization:");
            let norm_text = match settings.config.normalization_form {
                par_term_config::NormalizationForm::None => "None",
                par_term_config::NormalizationForm::NFC => "NFC (default)",
                par_term_config::NormalizationForm::NFD => "NFD",
                par_term_config::NormalizationForm::NFKC => "NFKC",
                par_term_config::NormalizationForm::NFKD => "NFKD",
            };
            egui::ComboBox::from_id_salt("terminal_normalization_form")
                .selected_text(norm_text)
                .show_ui(ui, |ui| {
                    let forms = [
                        (par_term_config::NormalizationForm::NFC, "NFC (default)"),
                        (par_term_config::NormalizationForm::NFD, "NFD"),
                        (par_term_config::NormalizationForm::NFKC, "NFKC"),
                        (par_term_config::NormalizationForm::NFKD, "NFKD"),
                        (par_term_config::NormalizationForm::None, "None (disabled)"),
                    ];
                    for (value, label) in forms {
                        if ui
                            .selectable_value(&mut settings.config.normalization_form, value, label)
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                })
                .response
                .on_hover_text(
                    "Unicode normalization form for text processing.\n\
                     - NFC: Canonical composition (default, most compatible)\n\
                     - NFD: Canonical decomposition (macOS HFS+ style)\n\
                     - NFKC: Compatibility composition (resolves ligatures)\n\
                     - NFKD: Compatibility decomposition\n\
                     - None: No normalization (store text as-is)",
                );
        });

        ui.add_space(8.0);

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

        ui.label(
            egui::RichText::new(
                "Warning: Setting this may expose terminal identification to applications",
            )
            .small()
            .color(egui::Color32::YELLOW),
        );
    });
}

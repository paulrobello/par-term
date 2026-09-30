//! Font-related sections of the appearance settings tab.
//!
//! Covers: Theme, Auto Dark Mode, Fonts, Font Variants, and Font Rendering.
//!
//! Text Shaping (ligatures, kerning) has no section: the renderer does not
//! read those settings yet (UX bug B39), so a control for them would do
//! nothing. They stay YAML-configurable.

use crate::SettingsUI;
use crate::section::{INPUT_WIDTH, SLIDER_WIDTH, collapsing_section, section_matches};
use par_term_config::Theme;
use par_term_config::ThinStrokesMode;
use std::collections::HashSet;

pub(super) fn show_theme_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    if section_matches(
        &settings.search_query.trim().to_lowercase(),
        "Theme",
        &["color", "scheme", "dark", "light", "color scheme", "preset"],
    ) {
        collapsing_section(ui, "Theme", "appearance_theme", true, collapsed, |ui| {
            let available = Theme::available_themes();
            let mut selected = settings.config.theme_colors.theme.clone();

            ui.horizontal(|ui| {
                ui.label("Theme:");
                egui::ComboBox::from_id_salt("appearance_theme_select")
                    .width(INPUT_WIDTH)
                    .selected_text(selected.clone())
                    .show_ui(ui, |ui| {
                        for theme in &available {
                            ui.selectable_value(&mut selected, theme.to_string(), *theme);
                        }
                    });
            });

            if selected != settings.config.theme_colors.theme {
                settings.config.theme_colors.theme = selected;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    }
}

pub(super) fn show_auto_dark_mode_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    if section_matches(
        &settings.search_query.trim().to_lowercase(),
        "Auto Dark Mode",
        &[
            "auto",
            "dark mode",
            "light mode",
            "system",
            "appearance",
            "automatic",
            "system theme",
        ],
    ) {
        collapsing_section(
            ui,
            "Auto Dark Mode",
            "appearance_auto_dark_mode",
            false,
            collapsed,
            |ui| {
                if ui
                    .checkbox(
                        &mut settings.config.theme_colors.auto_dark_mode,
                        "Auto-switch theme with system appearance",
                    )
                    .on_hover_text(
                        "Automatically switch between light and dark themes when the OS appearance changes",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                ui.add_enabled_ui(settings.config.theme_colors.auto_dark_mode, |ui| {
                    let available = Theme::available_themes();

                    ui.horizontal(|ui| {
                        ui.label("Light theme:");
                        let mut selected = settings.config.theme_colors.light_theme.clone();
                        egui::ComboBox::from_id_salt("appearance_light_theme_select")
                            .width(INPUT_WIDTH)
                            .selected_text(selected.clone())
                            .show_ui(ui, |ui| {
                                for theme in &available {
                                    ui.selectable_value(&mut selected, theme.to_string(), *theme);
                                }
                            });
                        if selected != settings.config.theme_colors.light_theme {
                            settings.config.theme_colors.light_theme = selected;
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Dark theme:");
                        let mut selected = settings.config.theme_colors.dark_theme.clone();
                        egui::ComboBox::from_id_salt("appearance_dark_theme_select")
                            .width(INPUT_WIDTH)
                            .selected_text(selected.clone())
                            .show_ui(ui, |ui| {
                                for theme in &available {
                                    ui.selectable_value(&mut selected, theme.to_string(), *theme);
                                }
                            });
                        if selected != settings.config.theme_colors.dark_theme {
                            settings.config.theme_colors.dark_theme = selected;
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });
                });
            },
        );
    }
}

pub(super) fn show_fonts_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    if section_matches(
        &settings.search_query.trim().to_lowercase(),
        "Fonts",
        &[
            "font",
            "family",
            "size",
            "bold",
            "italic",
            "line spacing",
            "char spacing",
        ],
    ) {
        collapsing_section(ui, "Fonts", "appearance_fonts", true, collapsed, |ui| {
            ui.horizontal(|ui| {
                ui.label("Family (regular):");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut settings.temp_font_family)
                            .desired_width(INPUT_WIDTH),
                    )
                    .changed()
                {
                    settings.font_pending_changes = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Size:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, 18.0],
                        egui::Slider::new(&mut settings.temp_font_size, 6.0..=48.0).suffix(" pt"),
                    )
                    .changed()
                {
                    settings.font_pending_changes = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Line spacing:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, 18.0],
                        egui::Slider::new(&mut settings.temp_line_spacing, 0.8..=2.0).suffix("×"),
                    )
                    .changed()
                {
                    settings.font_pending_changes = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Char spacing:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, 18.0],
                        egui::Slider::new(&mut settings.temp_char_spacing, 0.5..=1.0).suffix("×"),
                    )
                    .changed()
                {
                    settings.font_pending_changes = true;
                }
            });

            staged_font_apply_row(ui, settings, changes_this_frame);
        });
    }
}

/// Font fields are staged (UX.md SS9): typing a family name would otherwise
/// rebuild the renderer on every keystroke. Every section that holds staged
/// font fields shows this row, so Apply is next to what it applies. Save
/// also applies staged fonts and Revert drops them.
fn staged_font_apply_row(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    #[cfg(test)]
    APPLY_ROWS.with(|n| n.set(n.get() + 1));
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                settings.font_pending_changes,
                egui::Button::new("Apply font changes"),
            )
            .on_hover_text("Font changes wait for Apply; Save also applies them")
            .clicked()
        {
            settings.apply_font_changes();
            settings.has_changes = true;
            *changes_this_frame = true;
        }
        if settings.font_pending_changes {
            ui.colored_label(egui::Color32::YELLOW, "(pending)");
        }
    });
}

#[cfg(test)]
thread_local! {
    static APPLY_ROWS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(super) fn show_font_variants_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    if section_matches(
        &settings.search_query.trim().to_lowercase(),
        "Font Variants",
        &["bold", "italic", "bold-italic", "font fallback", "variant"],
    ) {
        collapsing_section(
            ui,
            "Font Variants",
            "appearance_font_variants",
            false,
            collapsed,
            |ui| {
                ui.horizontal(|ui| {
                    ui.label("Bold font (optional):");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut settings.temp_font_bold)
                                .desired_width(INPUT_WIDTH),
                        )
                        .changed()
                    {
                        settings.font_pending_changes = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Italic font (optional):");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut settings.temp_font_italic)
                                .desired_width(INPUT_WIDTH),
                        )
                        .changed()
                    {
                        settings.font_pending_changes = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Bold-Italic font (optional):");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut settings.temp_font_bold_italic)
                                .desired_width(INPUT_WIDTH),
                        )
                        .changed()
                    {
                        settings.font_pending_changes = true;
                    }
                });

                staged_font_apply_row(ui, settings, changes_this_frame);
            },
        );
    }
}

pub(super) fn show_font_rendering_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    if section_matches(
        &settings.search_query.trim().to_lowercase(),
        "Font Rendering",
        &[
            "antialias",
            "hinting",
            "thin strokes",
            "smoothing",
            "minimum contrast",
            "readability",
            "brightness",
            "hidpi",
            "retina",
        ],
    ) {
        collapsing_section(
            ui,
            "Font Rendering",
            "appearance_font_rendering",
            false,
            collapsed,
            |ui| {
                if ui
                    .checkbox(
                        &mut settings.config.font_rendering.font_antialias,
                        "Anti-aliasing",
                    )
                    .on_hover_text("Enable smooth font edges. Disable for crisp, pixelated text.")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if ui
                    .checkbox(&mut settings.config.font_rendering.font_hinting, "Hinting")
                    .on_hover_text(
                        "Align glyphs to pixel boundaries for sharper text at small sizes.",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                ui.horizontal(|ui| {
                    ui.label("Thin strokes:");
                    let current_mode = settings.config.font_rendering.font_thin_strokes;
                    let mode_label = match current_mode {
                        ThinStrokesMode::Never => "Never",
                        ThinStrokesMode::RetinaOnly => "Retina Only",
                        ThinStrokesMode::DarkBackgroundsOnly => "Dark Backgrounds Only",
                        ThinStrokesMode::RetinaDarkBackgroundsOnly => "Retina + Dark BG",
                        ThinStrokesMode::Always => "Always",
                    };

                    egui::ComboBox::from_id_salt("appearance_thin_strokes_mode")
                        .selected_text(mode_label)
                        .show_ui(ui, |ui| {
                            for (mode, label) in [
                                (ThinStrokesMode::Never, "Never"),
                                (ThinStrokesMode::RetinaOnly, "Retina Only"),
                                (
                                    ThinStrokesMode::DarkBackgroundsOnly,
                                    "Dark Backgrounds Only",
                                ),
                                (
                                    ThinStrokesMode::RetinaDarkBackgroundsOnly,
                                    "Retina + Dark BG",
                                ),
                                (ThinStrokesMode::Always, "Always"),
                            ] {
                                if ui.selectable_label(current_mode == mode, label).clicked() {
                                    settings.config.font_rendering.font_thin_strokes = mode;
                                    settings.has_changes = true;
                                    *changes_this_frame = true;
                                }
                            }
                        });
                });
                ui.label("  Lighter font strokes for improved readability on HiDPI displays.")
                    .on_hover_text(
                        "Similar to macOS font smoothing. Works best on Retina/HiDPI \
                         displays with dark backgrounds.",
                    );

                // Minimum contrast setting
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Minimum contrast:");
                    let mut contrast = settings.config.font_rendering.minimum_contrast;
                    let slider =
                        crate::units::percent(egui::Slider::new(&mut contrast, 0.0..=0.99))
                            .text("")
                            .clamping(egui::SliderClamping::Always);
                    if ui.add(slider).changed() {
                        settings.config.font_rendering.minimum_contrast = contrast;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
                let contrast_label = if settings.config.font_rendering.minimum_contrast <= 0.0 {
                    "Disabled"
                } else if settings.config.font_rendering.minimum_contrast < 0.5 {
                    "Low"
                } else if settings.config.font_rendering.minimum_contrast < 0.97 {
                    "High"
                } else {
                    "Maximum (near B&W)"
                };
                ui.label(format!(
                    "  {contrast_label} - Boosts text contrast when color is close to background."
                ))
                .on_hover_text(
                    "Set to 0 to disable. Higher values push text color further from background.",
                );
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use par_term_config::Config;

    #[test]
    fn font_variants_section_has_its_own_apply_row() {
        let mut settings = SettingsUI::new_for_tests(Config::default());
        // Font Variants starts collapsed; mark it toggled so it renders open.
        settings
            .collapsed_sections
            .insert("appearance_font_variants".to_string());
        settings.temp_font_bold = "Some Bold Font".to_string();
        settings.font_pending_changes = true;
        APPLY_ROWS.with(|n| n.set(0));

        let ctx = egui::Context::default();
        let mut changes = false;
        let mut collapsed = std::mem::take(&mut settings.collapsed_sections);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show_font_variants_section(ui, &mut settings, &mut changes, &mut collapsed);
            });
        });
        output.textures_delta.clear();

        assert_eq!(APPLY_ROWS.with(|n| n.get()), 1);
        assert!(settings.font_pending_changes);
    }
}

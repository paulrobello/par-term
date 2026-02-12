//! Appearance settings tab.
//!
//! Consolidates: theme_tab, font_tab, cursor_tab
//!
//! Contains:
//! - Theme selection
//! - Font settings (family, size, spacing, variants)
//! - Text shaping (ligatures, kerning)
//! - Font rendering options
//! - Cursor appearance and behavior

use super::SettingsUI;
use super::section::{INPUT_WIDTH, SLIDER_WIDTH, collapsing_section, subsection_label};
use crate::config::{CursorStyle, ThinStrokesMode, UnfocusedCursorStyle};
use crate::themes::Theme;
use std::collections::HashSet;

/// Show the appearance tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // Theme section
    if section_matches(
        &query,
        "Theme",
        &["color", "scheme", "dark", "light", "color scheme", "preset"],
    ) {
        show_theme_section(ui, settings, changes_this_frame, collapsed);
    }

    // Fonts section
    if section_matches(
        &query,
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
        show_fonts_section(ui, settings, changes_this_frame, collapsed);
    }

    // Font Variants section (collapsed by default)
    if section_matches(
        &query,
        "Font Variants",
        &["bold", "italic", "bold-italic", "font fallback", "variant"],
    ) {
        show_font_variants_section(ui, settings, changes_this_frame, collapsed);
    }

    // Text Shaping section (collapsed by default)
    if section_matches(
        &query,
        "Text Shaping",
        &[
            "shaping",
            "ligatures",
            "kerning",
            "harfbuzz",
            "complex scripts",
            "opentype",
        ],
    ) {
        show_text_shaping_section(ui, settings, changes_this_frame, collapsed);
    }

    // Font Rendering section (collapsed by default)
    if section_matches(
        &query,
        "Font Rendering",
        &[
            "antialias",
            "hinting",
            "thin strokes",
            "smoothing",
            "minimum contrast",
            "wcag",
            "readability",
            "hidpi",
            "retina",
        ],
    ) {
        show_font_rendering_section(ui, settings, changes_this_frame, collapsed);
    }

    // Cursor section
    if section_matches(
        &query,
        "Cursor",
        &[
            "style",
            "block",
            "beam",
            "underline",
            "blink",
            "color",
            "text color",
            "cursor text color",
            "unfocused cursor",
            "hollow",
        ],
    ) {
        show_cursor_section(ui, settings, changes_this_frame, collapsed);
    }

    // Cursor Locks section (collapsed by default)
    if section_matches(
        &query,
        "Cursor Locks",
        &[
            "lock",
            "visibility",
            "style",
            "blink",
            "prevent applications",
        ],
    ) {
        show_cursor_locks_section(ui, settings, changes_this_frame, collapsed);
    }

    // Cursor Effects section (collapsed by default)
    if section_matches(
        &query,
        "Cursor Effects",
        &[
            "guide",
            "shadow",
            "boost",
            "glow",
            "horizontal line",
            "drop shadow",
            "shadow blur",
            "cursor row",
        ],
    ) {
        show_cursor_effects_section(ui, settings, changes_this_frame, collapsed);
    }
}

fn section_matches(query: &str, title: &str, keywords: &[&str]) -> bool {
    if query.is_empty() {
        return true;
    }
    if title.to_lowercase().contains(query) {
        return true;
    }
    keywords.iter().any(|k| k.to_lowercase().contains(query))
}

// ============================================================================
// Theme Section
// ============================================================================

fn show_theme_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Theme", "appearance_theme", true, collapsed, |ui| {
        let available = Theme::available_themes();
        let mut selected = settings.config.theme.clone();

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

        if selected != settings.config.theme {
            settings.config.theme = selected;
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

// ============================================================================
// Fonts Section
// ============================================================================

fn show_fonts_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
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
                    egui::Slider::new(&mut settings.temp_font_size, 6.0..=48.0),
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
                    egui::Slider::new(&mut settings.temp_line_spacing, 0.8..=2.0),
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
                    egui::Slider::new(&mut settings.temp_char_spacing, 0.5..=1.0),
                )
                .changed()
            {
                settings.font_pending_changes = true;
            }
        });

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

// ============================================================================
// Font Variants Section
// ============================================================================

fn show_font_variants_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    _changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
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
        },
    );
}

// ============================================================================
// Text Shaping Section
// ============================================================================

fn show_text_shaping_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    _changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Text Shaping",
        "appearance_text_shaping",
        false,
        collapsed,
        |ui| {
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
        },
    );
}

// ============================================================================
// Font Rendering Section
// ============================================================================

fn show_font_rendering_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Font Rendering",
        "appearance_font_rendering",
        false,
        collapsed,
        |ui| {
            if ui
                .checkbox(&mut settings.config.font_antialias, "Anti-aliasing")
                .on_hover_text("Enable smooth font edges. Disable for crisp, pixelated text.")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(&mut settings.config.font_hinting, "Hinting")
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
                                settings.config.font_thin_strokes = mode;
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });
            ui.label("  Lighter font strokes for improved readability on HiDPI displays.")
            .on_hover_text("Similar to macOS font smoothing. Works best on Retina/HiDPI displays with dark backgrounds.");

            // Minimum contrast setting
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Minimum contrast:");
                let mut contrast = settings.config.minimum_contrast;
                let slider = egui::Slider::new(&mut contrast, 1.0..=7.0)
                    .text("")
                    .clamping(egui::SliderClamping::Always);
                if ui.add(slider).changed() {
                    settings.config.minimum_contrast = contrast;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
            let wcag_label = if settings.config.minimum_contrast <= 1.0 {
                "Disabled"
            } else if settings.config.minimum_contrast < 4.5 {
                "Custom"
            } else if settings.config.minimum_contrast < 7.0 {
                "WCAG AA (4.5:1)"
            } else {
                "WCAG AAA (7:1)"
            };
            ui.label(format!(
                "  {wcag_label} - Adjusts text color to ensure readability against background."
            ))
            .on_hover_text(
                "Set to 1.0 to disable. WCAG AA (4.5:1) is recommended for accessibility.",
            );
        },
    );
}

// ============================================================================
// Cursor Section
// ============================================================================

fn show_cursor_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Cursor", "appearance_cursor", true, collapsed, |ui| {
        ui.horizontal(|ui| {
            ui.label("Style:");
            let current = match settings.config.cursor_style {
                CursorStyle::Block => 0,
                CursorStyle::Beam => 1,
                CursorStyle::Underline => 2,
            };
            let mut selected = current;
            egui::ComboBox::from_id_salt("appearance_cursor_style")
                .selected_text(match current {
                    0 => "Block",
                    1 => "Beam",
                    2 => "Underline",
                    _ => "Unknown",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected, 0, "Block");
                    ui.selectable_value(&mut selected, 1, "Beam");
                    ui.selectable_value(&mut selected, 2, "Underline");
                });
            if selected != current {
                settings.config.cursor_style = match selected {
                    0 => CursorStyle::Block,
                    1 => CursorStyle::Beam,
                    2 => CursorStyle::Underline,
                    _ => CursorStyle::Block,
                };
                settings.has_changes = true;
            }
        });

        if ui
            .checkbox(&mut settings.config.cursor_blink, "Cursor blink")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.horizontal(|ui| {
            ui.label("Blink interval (ms):");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.cursor_blink_interval,
                    100..=2000,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Color:");
            let mut color = settings.config.cursor_color;
            if ui.color_edit_button_srgb(&mut color).changed() {
                settings.config.cursor_color = color;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Text color (block cursor):");
            let mut use_custom_color = settings.config.cursor_text_color.is_some();
            if ui
                .checkbox(&mut use_custom_color, "")
                .on_hover_text("Enable custom text color under block cursor")
                .changed()
            {
                if use_custom_color {
                    settings.config.cursor_text_color = Some([0, 0, 0]);
                } else {
                    settings.config.cursor_text_color = None;
                }
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if let Some(ref mut text_color) = settings.config.cursor_text_color {
                let mut color = *text_color;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    *text_color = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            } else {
                ui.label("(auto)");
            }
        });

        subsection_label(ui, "When Unfocused");

        ui.horizontal(|ui| {
            ui.label("Style:");
            let current = match settings.config.unfocused_cursor_style {
                UnfocusedCursorStyle::Hollow => 0,
                UnfocusedCursorStyle::Same => 1,
                UnfocusedCursorStyle::Hidden => 2,
            };
            let mut selected = current;
            egui::ComboBox::from_id_salt("appearance_unfocused_cursor_style")
                .selected_text(match current {
                    0 => "Hollow (outline)",
                    1 => "Same",
                    2 => "Hidden",
                    _ => "Unknown",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected, 0, "Hollow (outline)");
                    ui.selectable_value(&mut selected, 1, "Same");
                    ui.selectable_value(&mut selected, 2, "Hidden");
                });
            if selected != current {
                settings.config.unfocused_cursor_style = match selected {
                    0 => UnfocusedCursorStyle::Hollow,
                    1 => UnfocusedCursorStyle::Same,
                    2 => UnfocusedCursorStyle::Hidden,
                    _ => UnfocusedCursorStyle::Hollow,
                };
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}

// ============================================================================
// Cursor Locks Section
// ============================================================================

fn show_cursor_locks_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Cursor Locks",
        "appearance_cursor_locks",
        false,
        collapsed,
        |ui| {
            ui.label("Prevent applications from changing cursor settings:");
            ui.add_space(4.0);

            if ui
                .checkbox(
                    &mut settings.config.lock_cursor_visibility,
                    "Lock cursor visibility",
                )
                .on_hover_text("Prevent applications from hiding the cursor")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(&mut settings.config.lock_cursor_style, "Lock cursor style")
                .on_hover_text("Prevent applications from changing cursor style")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_enabled_ui(!settings.config.lock_cursor_style, |ui| {
                if ui
                    .checkbox(&mut settings.config.lock_cursor_blink, "Lock cursor blink")
                    .on_hover_text(if settings.config.lock_cursor_style {
                        "Disabled: Lock cursor style already controls blink"
                    } else {
                        "Prevent applications from enabling cursor blink"
                    })
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        },
    );
}

// ============================================================================
// Cursor Effects Section
// ============================================================================

fn show_cursor_effects_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Cursor Effects",
        "appearance_cursor_effects",
        false,
        collapsed,
        |ui| {
            // Cursor Guide
            if ui
                .checkbox(
                    &mut settings.config.cursor_guide_enabled,
                    "Cursor guide (horizontal line)",
                )
                .on_hover_text("Show a subtle horizontal line at the cursor row")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if settings.config.cursor_guide_enabled {
                ui.horizontal(|ui| {
                    ui.label("Guide color:");
                    let mut color = settings.config.cursor_guide_color;
                    if ui
                        .color_edit_button_srgba_unmultiplied(&mut color)
                        .changed()
                    {
                        settings.config.cursor_guide_color = color;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            }

            ui.add_space(4.0);

            // Cursor Shadow
            if ui
                .checkbox(&mut settings.config.cursor_shadow_enabled, "Cursor shadow")
                .on_hover_text("Add a drop shadow behind the cursor")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if settings.config.cursor_shadow_enabled {
                ui.horizontal(|ui| {
                    ui.label("Shadow color:");
                    let mut color = settings.config.cursor_shadow_color;
                    if ui
                        .color_edit_button_srgba_unmultiplied(&mut color)
                        .changed()
                    {
                        settings.config.cursor_shadow_color = color;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Shadow offset X:");
                    if ui
                        .add(egui::Slider::new(
                            &mut settings.config.cursor_shadow_offset[0],
                            0.0..=10.0,
                        ))
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Shadow offset Y:");
                    if ui
                        .add(egui::Slider::new(
                            &mut settings.config.cursor_shadow_offset[1],
                            0.0..=10.0,
                        ))
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Shadow blur:");
                    if ui
                        .add(
                            egui::Slider::new(&mut settings.config.cursor_shadow_blur, 0.0..=20.0)
                                .suffix(" px"),
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            }

            ui.add_space(4.0);

            // Cursor Boost (Glow)
            ui.horizontal(|ui| {
                ui.label("Cursor boost (glow):");
                if ui
                    .add(egui::Slider::new(
                        &mut settings.config.cursor_boost,
                        0.0..=1.0,
                    ))
                    .on_hover_text("Add a glow effect around the cursor for visibility")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            if settings.config.cursor_boost > 0.0 {
                ui.horizontal(|ui| {
                    ui.label("Boost color:");
                    let mut color = settings.config.cursor_boost_color;
                    if ui.color_edit_button_srgb(&mut color).changed() {
                        settings.config.cursor_boost_color = color;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            }
        },
    );
}

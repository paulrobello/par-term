//! General › Common: the most frequently changed settings on one page
//! (UX.md SD3, formerly the quick-settings strip above every tab).
//!
//! Every control here is a deliberate summary (UX.md SC7): each one also
//! lives on its owning page, which carries the reset button and YAML-key
//! search tag. "Go to" links lead there.

use super::SettingsUI;
use crate::section::keyword_section;
use par_term_config::Theme;
use par_term_config::{BackgroundMode, CursorStyle, TabBarMode};
use std::collections::HashSet;

/// Render the General › Common page.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "Common",
        "general_common",
        &["quick settings"],
        true,
        collapsed,
        |ui| {
            ui.label(
                egui::RichText::new(
                    "The settings changed most often. Each also lives on its own page.",
                )
                .small()
                .weak(),
            );
            ui.add_space(4.0);
            show_controls(ui, settings, changes_this_frame);
            ui.add_space(8.0);
            show_links(ui, settings);
        },
    );
}

/// Links to the pages that own the controls above.
fn show_links(ui: &mut egui::Ui, settings: &mut SettingsUI) {
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("Go to:").small());
        for (label, section) in [
            ("Text & Fonts", "appearance_fonts"),
            ("Theme", "appearance_theme"),
            ("Cursor", "appearance_cursor"),
            ("Window", "window_transparency"),
            ("Tab Bar", "window_tab_bar"),
            ("Background & Shader", "background_effects"),
        ] {
            if ui.link(label).clicked() {
                settings.open_section(section);
            }
        }
    });
}

fn show_controls(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;

        // Font Family dropdown
        ui.horizontal(|ui| {
            ui.label("Font:");
            let response = ui.add(
                egui::TextEdit::singleline(&mut settings.temp_font_family)
                    .desired_width(120.0)
                    .hint_text("JetBrains Mono"),
            );
            if response.changed() {
                settings.font_pending_changes = true;
            }
        });

        ui.separator();

        // Font Size slider
        ui.horizontal(|ui| {
            ui.label("Size:");
            if ui
                .add(
                    egui::Slider::new(&mut settings.temp_font_size, 6.0..=48.0)
                        .suffix(" pt")
                        .show_value(true),
                )
                .changed()
            {
                settings.font_pending_changes = true;
            }
        });

        ui.separator();

        // Theme dropdown
        ui.horizontal(|ui| {
            ui.label("Theme:");
            let available = Theme::available_themes();
            let mut selected = settings.config.theme_colors.theme.clone();
            egui::ComboBox::from_id_salt("quick_theme_select")
                .width(120.0)
                .selected_text(selected.clone())
                .show_ui(ui, |ui| {
                    for theme in &available {
                        ui.selectable_value(&mut selected, theme.to_string(), *theme);
                    }
                });
            if selected != settings.config.theme_colors.theme {
                settings.config.theme_colors.theme = selected;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;

        // Window Opacity slider
        ui.horizontal(|ui| {
            ui.label("Opacity:");
            if ui
                .add(crate::units::percent(egui::Slider::new(
                    &mut settings.config.window.window_opacity,
                    0.1..=1.0,
                )))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.separator();

        // Cursor Style segmented control
        ui.horizontal(|ui| {
            ui.label("Cursor:");
            let current = settings.config.cursor.cursor_style;
            if ui
                .selectable_label(current == CursorStyle::Block, "Block")
                .clicked()
            {
                settings.config.cursor.cursor_style = CursorStyle::Block;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            if ui
                .selectable_label(current == CursorStyle::Beam, "Beam")
                .clicked()
            {
                settings.config.cursor.cursor_style = CursorStyle::Beam;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            if ui
                .selectable_label(current == CursorStyle::Underline, "Line")
                .clicked()
            {
                settings.config.cursor.cursor_style = CursorStyle::Underline;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.separator();

        // Cursor Blink checkbox
        if ui
            .checkbox(&mut settings.config.cursor.cursor_blink, "Blink")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;

        // Tab Bar visibility dropdown
        ui.horizontal(|ui| {
            ui.label("Tab bar:");
            let current = match settings.config.tabs.tab_bar_mode {
                TabBarMode::Always => 0,
                TabBarMode::WhenMultiple => 1,
                TabBarMode::Never => 2,
            };
            let mut selected = current;
            egui::ComboBox::from_id_salt("quick_tab_bar_mode")
                .width(100.0)
                .selected_text(match current {
                    0 => "Always",
                    1 => "Multiple",
                    2 => "Never",
                    _ => "Unknown",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected, 0, "Always");
                    ui.selectable_value(&mut selected, 1, "When multiple");
                    ui.selectable_value(&mut selected, 2, "Never");
                });
            if selected != current {
                settings.config.tabs.tab_bar_mode = match selected {
                    0 => TabBarMode::Always,
                    1 => TabBarMode::WhenMultiple,
                    2 => TabBarMode::Never,
                    _ => TabBarMode::WhenMultiple,
                };
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.separator();

        // Background mode dropdown
        ui.horizontal(|ui| {
            ui.label("Background:");
            let current = match settings.config.image.background_mode {
                BackgroundMode::Default => 0,
                BackgroundMode::Color => 1,
                BackgroundMode::Image => 2,
            };
            let mut selected = current;
            egui::ComboBox::from_id_salt("quick_bg_mode")
                .width(80.0)
                .selected_text(match current {
                    0 => "Default",
                    1 => "Color",
                    2 => "Image",
                    _ => "Unknown",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected, 0, "Default");
                    ui.selectable_value(&mut selected, 1, "Color");
                    ui.selectable_value(&mut selected, 2, "Image");
                });
            if selected != current {
                settings.config.image.background_mode = match selected {
                    0 => BackgroundMode::Default,
                    1 => BackgroundMode::Color,
                    2 => BackgroundMode::Image,
                    _ => BackgroundMode::Default,
                };
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.separator();

        // Background shader toggle
        if ui
            .checkbox(
                &mut settings.config.shader.custom_shader_enabled,
                "BG Shader",
            )
            .on_hover_text("Enable background shader effect")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.separator();

        // Cursor shader toggle
        if ui
            .checkbox(
                &mut settings.config.shader.cursor_shader_enabled,
                "Cursor Shader",
            )
            .on_hover_text("Enable cursor shader effect")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        // Apply Font Changes button (only show if pending)
        if settings.font_pending_changes {
            ui.separator();
            if ui.button("Apply Font").clicked() {
                settings.apply_font_changes();
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            ui.colored_label(egui::Color32::YELLOW, "(pending)");
        }
    });
}

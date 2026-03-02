//! Quick settings strip - horizontal bar with commonly adjusted settings.
//!
//! This component provides fast access to the most frequently changed settings
//! without navigating through the full settings UI.

use super::SettingsUI;
use par_term_config::Theme;
use par_term_config::{BackgroundMode, CursorStyle, TabBarMode};

/// Render the quick settings strip at the top of the settings UI.
///
/// Returns true if any setting was changed this frame.
pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
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
                .add(egui::Slider::new(&mut settings.temp_font_size, 6.0..=48.0).show_value(true))
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
            let mut selected = settings.config.theme.clone();
            egui::ComboBox::from_id_salt("quick_theme_select")
                .width(120.0)
                .selected_text(selected.clone())
                .show_ui(ui, |ui| {
                    for theme in &available {
                        ui.selectable_value(&mut selected, theme.to_string(), *theme);
                    }
                });
            if selected != settings.config.theme {
                settings.config.theme = selected;
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
                .add(egui::Slider::new(
                    &mut settings.config.window_opacity,
                    0.1..=1.0,
                ))
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
            let current = settings.config.cursor_style;
            if ui
                .selectable_label(current == CursorStyle::Block, "Block")
                .clicked()
            {
                settings.config.cursor_style = CursorStyle::Block;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            if ui
                .selectable_label(current == CursorStyle::Beam, "Beam")
                .clicked()
            {
                settings.config.cursor_style = CursorStyle::Beam;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            if ui
                .selectable_label(current == CursorStyle::Underline, "Line")
                .clicked()
            {
                settings.config.cursor_style = CursorStyle::Underline;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.separator();

        // Cursor Blink checkbox
        if ui
            .checkbox(&mut settings.config.cursor_blink, "Blink")
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
            let current = match settings.config.tab_bar_mode {
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
                settings.config.tab_bar_mode = match selected {
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
            let current = match settings.config.background_mode {
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
                settings.config.background_mode = match selected {
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

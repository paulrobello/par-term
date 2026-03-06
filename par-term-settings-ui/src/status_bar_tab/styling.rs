//! Status bar styling section (colors, font size, separator).

use crate::SettingsUI;
use crate::section::{SLIDER_WIDTH, collapsing_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub fn show_styling_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Styling", "status_bar_styling", true, collapsed, |ui| {
        // Background color
        ui.horizontal(|ui| {
            ui.label("Background color:");
            let mut color = egui::Color32::from_rgb(
                settings.config.status_bar.status_bar_bg_color[0],
                settings.config.status_bar.status_bar_bg_color[1],
                settings.config.status_bar.status_bar_bg_color[2],
            );
            if ui.color_edit_button_srgba(&mut color).changed() {
                settings.config.status_bar.status_bar_bg_color = [color.r(), color.g(), color.b()];
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // Background opacity
        ui.horizontal(|ui| {
            ui.label("Background opacity:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(
                        &mut settings.config.status_bar.status_bar_bg_alpha,
                        0.0..=1.0,
                    )
                    .show_value(true),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(4.0);

        // Foreground color
        ui.horizontal(|ui| {
            ui.label("Text color:");
            let mut color = egui::Color32::from_rgb(
                settings.config.status_bar.status_bar_fg_color[0],
                settings.config.status_bar.status_bar_fg_color[1],
                settings.config.status_bar.status_bar_fg_color[2],
            );
            if ui.color_edit_button_srgba(&mut color).changed() {
                settings.config.status_bar.status_bar_fg_color = [color.r(), color.g(), color.b()];
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);

        // Font size
        ui.horizontal(|ui| {
            ui.label("Font size:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(
                        &mut settings.config.status_bar.status_bar_font_size,
                        8.0..=24.0,
                    )
                    .suffix(" pt")
                    .show_value(true),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);

        // Separator
        ui.horizontal(|ui| {
            ui.label("Separator:");
            if ui
                .add(
                    egui::TextEdit::singleline(
                        &mut settings.config.status_bar.status_bar_separator,
                    )
                    .font(egui::TextStyle::Monospace)
                    .hint_text(" | ")
                    .desired_width(80.0),
                )
                .on_hover_text("Text displayed between widgets in the same section")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}

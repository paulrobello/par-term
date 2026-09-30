//! Status bar styling section (colors, font size, separator).

use crate::SettingsUI;
use crate::search::SearchTag;
use crate::section::{SLIDER_WIDTH, keyword_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub fn show_styling_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "Styling",
        "status_bar_styling",
        &["foreground"],
        true,
        collapsed,
        |ui| {
            crate::dependent::dependent(
                ui,
                settings.config.status_bar.status_bar_enabled,
                "Enable status bar",
                |ui| {
                    // Background color
                    ui.horizontal(|ui| {
                        ui.label("Background color:");
                        if crate::color_helpers::rgb_color_button(
                            ui,
                            &mut settings.config.status_bar.status_bar_bg_color,
                        )
                        .changed()
                        {
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
                                crate::units::percent(egui::Slider::new(
                                    &mut settings.config.status_bar.status_bar_bg_alpha,
                                    0.0..=1.0,
                                ))
                                .show_value(true),
                            )
                            .search_tag(&["status_bar_bg_alpha"])
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                            &mut c.status_bar.status_bar_bg_alpha
                        });
                    });

                    ui.add_space(4.0);

                    // Foreground color
                    ui.horizontal(|ui| {
                        ui.label("Text color:");
                        if crate::color_helpers::rgb_color_button(
                            ui,
                            &mut settings.config.status_bar.status_bar_fg_color,
                        )
                        .changed()
                        {
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
                            .search_tag(&["status_bar_font_size"])
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                            &mut c.status_bar.status_bar_font_size
                        });
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
                },
            );
        },
    );
}

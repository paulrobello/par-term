//! Transparency section of the window settings tab.

use crate::SettingsUI;
use crate::section::{SLIDER_WIDTH, collapsing_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub(super) fn show_transparency_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Transparency",
        "window_transparency",
        true,
        collapsed,
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Opacity:");
                let response = ui.add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.window_opacity, 0.1..=1.0),
                );
                if response.changed() {
                    log::info!(
                        "Opacity slider changed to: {}",
                        settings.config.window_opacity
                    );
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            if ui
                .checkbox(
                    &mut settings.config.transparency_affects_only_default_background,
                    "Transparency affects only default background",
                )
                .on_hover_text(
                    "When enabled, colored backgrounds (syntax highlighting, status bars) remain opaque for better readability",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(&mut settings.config.keep_text_opaque, "Keep text opaque")
                .on_hover_text(
                    "When enabled, text is always rendered at full opacity regardless of window transparency",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            // Blur settings (macOS only)
            #[cfg(target_os = "macos")]
            {
                ui.add_space(8.0);

                if ui
                    .checkbox(&mut settings.config.blur_enabled, "Enable window blur")
                    .on_hover_text(
                        "Blur content behind the transparent window for better readability (requires transparency)",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if settings.config.blur_enabled {
                    ui.horizontal(|ui| {
                        ui.label("Blur radius:");
                        // Convert u32 to i32 for slider, clamp to valid range
                        let mut radius_i32 = settings.config.blur_radius.min(64) as i32;
                        if ui
                            .add(egui::Slider::new(&mut radius_i32, 1..=64))
                            .on_hover_text("Blur intensity (higher = more blur)")
                            .changed()
                        {
                            settings.config.blur_radius = radius_i32 as u32;
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });
                }
            }
        },
    );
}

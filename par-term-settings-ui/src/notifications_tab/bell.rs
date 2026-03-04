//! Bell notification settings — visual, audio, and desktop bell.

use crate::SettingsUI;
use crate::section::{SLIDER_WIDTH, collapsing_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub(super) fn show_bell_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Bell", "notifications_bell", true, collapsed, |ui| {
        if ui
            .checkbox(&mut settings.config.notification_bell_visual, "Visual bell")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        // Visual bell color picker (only shown when visual bell is enabled)
        if settings.config.notification_bell_visual {
            ui.horizontal(|ui| {
                ui.label("Flash color:");
                let mut color = egui::Color32::from_rgb(
                    settings.config.notification_visual_bell_color[0],
                    settings.config.notification_visual_bell_color[1],
                    settings.config.notification_visual_bell_color[2],
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    settings.config.notification_visual_bell_color =
                        [color.r(), color.g(), color.b()];
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }

        ui.horizontal(|ui| {
            ui.label("Audio bell volume (0=off):");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.notification_bell_sound, 0..=100),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.notification_bell_desktop,
                "Desktop notifications for bell",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

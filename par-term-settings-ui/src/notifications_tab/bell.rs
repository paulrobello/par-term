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
            .checkbox(
                &mut settings.config.notifications.notification_bell_visual,
                "Visual bell",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        // Visual bell color picker (only shown when visual bell is enabled)
        if settings.config.notifications.notification_bell_visual {
            ui.horizontal(|ui| {
                ui.label("Flash color:");
                if crate::color_helpers::rgb_color_button(
                    ui,
                    &mut settings.config.notifications.notification_visual_bell_color,
                )
                .changed()
                {
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
                    egui::Slider::new(
                        &mut settings.config.notifications.notification_bell_sound,
                        0..=100,
                    ),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.notifications.notification_bell_desktop,
                "Desktop notifications for bell",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

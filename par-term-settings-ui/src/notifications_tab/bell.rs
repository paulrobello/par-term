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

        crate::dependent::dependent(
            ui,
            settings.config.notifications.notification_bell_visual,
            "Visual bell",
            |ui| {
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
            },
        );

        // One bell model (UX.md B40): an enabled Alert Sounds › Bell entry
        // owns the bell's sound, so the plain volume is disabled rather than
        // silently ignored.
        let alert_owns_bell = settings
            .config
            .notifications
            .alert_sounds
            .get(&par_term_config::AlertEvent::Bell)
            .is_some_and(|alert| alert.enabled);
        crate::dependent::dependent(ui, !alert_owns_bell, "Alert Sounds › Bell: off", |ui| {
            ui.horizontal(|ui| {
                ui.label("Audio bell volume:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.notifications.notification_bell_sound,
                            0..=100,
                        )
                        .suffix(" %"),
                    )
                    .on_hover_text("0 % turns the audio bell off")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                    &mut c.notifications.notification_bell_sound
                });
            });
        });
        if alert_owns_bell {
            ui.label(
                egui::RichText::new(
                    "The Bell entry under Alert Sounds sets the bell's sound and volume.",
                )
                .small()
                .color(egui::Color32::GRAY),
            );
        }

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

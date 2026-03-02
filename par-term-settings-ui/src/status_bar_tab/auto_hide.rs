//! Status bar auto-hide settings section (fullscreen, mouse inactivity timeout).

use crate::SettingsUI;
use crate::section::{SLIDER_WIDTH, collapsing_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub fn show_auto_hide_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Auto-Hide",
        "status_bar_auto_hide",
        false,
        collapsed,
        |ui| {
            if ui
                .checkbox(
                    &mut settings.config.status_bar.status_bar_auto_hide_fullscreen,
                    "Hide in fullscreen",
                )
                .on_hover_text("Automatically hide the status bar when the window is fullscreen")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings
                        .config
                        .status_bar
                        .status_bar_auto_hide_mouse_inactive,
                    "Hide on mouse inactivity",
                )
                .on_hover_text("Automatically hide the status bar when the mouse has been inactive")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            // Timeout slider (only shown when mouse inactivity hide is enabled)
            if settings
                .config
                .status_bar
                .status_bar_auto_hide_mouse_inactive
            {
                ui.horizontal(|ui| {
                    ui.label("Timeout:");
                    if ui
                        .add_sized(
                            [SLIDER_WIDTH, SLIDER_HEIGHT],
                            egui::Slider::new(
                                &mut settings.config.status_bar.status_bar_mouse_inactive_timeout,
                                1.0..=30.0,
                            )
                            .suffix(" sec")
                            .show_value(true),
                        )
                        .on_hover_text(
                            "Seconds of mouse inactivity before the status bar is hidden",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            }
        },
    );
}

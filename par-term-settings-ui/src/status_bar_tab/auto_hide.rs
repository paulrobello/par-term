//! Status bar auto-hide settings section (fullscreen, mouse inactivity timeout).

use crate::SettingsUI;
use crate::search::SearchTag;
use crate::section::{SLIDER_WIDTH, keyword_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub fn show_auto_hide_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "Auto-Hide",
        "status_bar_auto_hide",
        &["inactivity timeout"],
        false,
        collapsed,
        |ui| {
            crate::dependent::dependent(
                ui,
                settings.config.status_bar.status_bar_enabled,
                "Enable status bar",
                |ui| {
                    if ui
                        .checkbox(
                            &mut settings.config.status_bar.status_bar_auto_hide_fullscreen,
                            "Hide in fullscreen",
                        )
                        .search_tag(&["status_bar_auto_hide_fullscreen"])
                        .on_hover_text(
                            "Automatically hide the status bar when the window is fullscreen",
                        )
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
                        .search_tag(&["status_bar_auto_hide_mouse_inactive"])
                        .on_hover_text(
                            "Automatically hide the status bar when the mouse has been inactive",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }

                    crate::dependent::dependent(
                        ui,
                        settings
                            .config
                            .status_bar
                            .status_bar_auto_hide_mouse_inactive,
                        "Hide on mouse inactivity",
                        |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Timeout:");
                                if ui
                        .add_sized(
                            [SLIDER_WIDTH, SLIDER_HEIGHT],
                            egui::Slider::new(
                                &mut settings.config.status_bar.status_bar_mouse_inactive_timeout,
                                1.0..=30.0,
                            )
                            .suffix(" s")
                            .show_value(true),
                        ).search_tag(&["status_bar_mouse_inactive_timeout"])
                        .on_hover_text(
                            "Seconds of mouse inactivity before the status bar is hidden",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                                crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                                    &mut c.status_bar.status_bar_mouse_inactive_timeout
                                });
                            });
                        },
                    );
                },
            );
        },
    );
}

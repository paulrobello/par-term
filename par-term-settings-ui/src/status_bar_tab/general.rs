//! Status bar general settings section (enable, position, height).

use crate::SettingsUI;
use crate::section::{SLIDER_WIDTH, collapsing_section};
use par_term_config::StatusBarPosition;
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub fn show_general_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "General", "status_bar_general", true, collapsed, |ui| {
        if ui
            .checkbox(
                &mut settings.config.status_bar.status_bar_enabled,
                "Enable status bar",
            )
            .on_hover_text("Show a configurable status bar with widgets")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);

        // Position dropdown
        ui.horizontal(|ui| {
            ui.label("Position:");
            egui::ComboBox::from_id_salt("status_bar_position")
                .selected_text(match settings.config.status_bar.status_bar_position {
                    StatusBarPosition::Top => "Top",
                    StatusBarPosition::Bottom => "Bottom",
                })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(
                            &mut settings.config.status_bar.status_bar_position,
                            StatusBarPosition::Top,
                            "Top",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                    if ui
                        .selectable_value(
                            &mut settings.config.status_bar.status_bar_position,
                            StatusBarPosition::Bottom,
                            "Bottom",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
        });

        // Height slider
        ui.horizontal(|ui| {
            ui.label("Height:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(
                        &mut settings.config.status_bar.status_bar_height,
                        16.0..=40.0,
                    )
                    .suffix(" px")
                    .show_value(true),
                )
                .on_hover_text("Height of the status bar in logical pixels")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}

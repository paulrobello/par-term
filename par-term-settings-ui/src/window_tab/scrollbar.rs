//! Scrollbar section for the window settings tab.

use crate::SettingsUI;
use crate::search::SearchTag;
use crate::section::{SLIDER_WIDTH, collapsing_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub(crate) fn show_scrollbar_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Scrollbar", "window_scrollbar", true, collapsed, |ui| {
        if ui
            .checkbox(
                &mut settings.config.scrollbar.scrollbar_command_marks,
                "Show command markers (requires shell integration)",
            )
            .search_tag(&["scrollbar_command_marks"])
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        // Indent the tooltip option under command markers
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.add_enabled_ui(settings.config.scrollbar.scrollbar_command_marks, |ui| {
                if ui
                    .checkbox(
                        &mut settings.config.scrollbar.scrollbar_mark_tooltips,
                        "Show tooltips on hover",
                    )
                    .search_tag(&["scrollbar_mark_tooltips"])
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        });

        ui.horizontal(|ui| {
            ui.label("Side:");
            let mut position = settings.config.scrollbar.scrollbar_position.clone();
            egui::ComboBox::from_id_salt("scrollbar_position")
                .selected_text(if position == "left" { "Left" } else { "Right" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut position, "left".to_string(), "Left");
                    ui.selectable_value(&mut position, "right".to_string(), "Right");
                })
                .response
                .search_tag(&["scrollbar_position"])
                .on_hover_text("Which edge of the terminal the scrollbar is drawn on");
            if position != settings.config.scrollbar.scrollbar_position {
                settings.config.scrollbar.scrollbar_position = position;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Width:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.scrollbar.scrollbar_width, 4.0..=50.0)
                        .suffix(" px"),
                )
                .search_tag(&["scrollbar_width"])
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                &mut c.scrollbar.scrollbar_width
            });
        });

        ui.horizontal(|ui| {
            ui.label("Autohide delay (0 = never):");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(
                        &mut settings.config.scrollbar.scrollbar_autohide_delay,
                        0..=5000,
                    )
                    .suffix(" ms"),
                )
                .search_tag(&["scrollbar_autohide_delay"])
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                &mut c.scrollbar.scrollbar_autohide_delay
            });
        });

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Colors").strong());

        ui.horizontal(|ui| {
            ui.label("Thumb color:");
            if crate::color_helpers::rgba_f32_color_button(
                ui,
                &mut settings.config.scrollbar.scrollbar_thumb_color,
            )
            .search_tag(&["scrollbar_thumb_color"])
            .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Track color:");
            if crate::color_helpers::rgba_f32_color_button(
                ui,
                &mut settings.config.scrollbar.scrollbar_track_color,
            )
            .search_tag(&["scrollbar_track_color"])
            .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}

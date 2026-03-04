//! Scrollbar section for the window settings tab.

use crate::SettingsUI;
use crate::section::{SLIDER_WIDTH, collapsing_section};
use par_term_config::color_u8x4_to_f32;
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub(super) fn show_scrollbar_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Scrollbar", "window_scrollbar", true, collapsed, |ui| {
        if ui
            .checkbox(
                &mut settings.config.scrollbar_command_marks,
                "Show command markers (requires shell integration)",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        // Indent the tooltip option under command markers
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.add_enabled_ui(settings.config.scrollbar_command_marks, |ui| {
                if ui
                    .checkbox(
                        &mut settings.config.scrollbar_mark_tooltips,
                        "Show tooltips on hover",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        });

        ui.horizontal(|ui| {
            ui.label("Width:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.scrollbar_width, 4.0..=50.0),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Autohide delay (ms, 0=never):");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.scrollbar_autohide_delay, 0..=5000),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Colors").strong());

        ui.horizontal(|ui| {
            ui.label("Thumb color:");
            let mut thumb = egui::Color32::from_rgba_unmultiplied(
                (settings.config.scrollbar_thumb_color[0] * 255.0) as u8,
                (settings.config.scrollbar_thumb_color[1] * 255.0) as u8,
                (settings.config.scrollbar_thumb_color[2] * 255.0) as u8,
                (settings.config.scrollbar_thumb_color[3] * 255.0) as u8,
            );
            if egui::color_picker::color_edit_button_srgba(
                ui,
                &mut thumb,
                egui::color_picker::Alpha::Opaque,
            )
            .changed()
            {
                settings.config.scrollbar_thumb_color =
                    color_u8x4_to_f32([thumb.r(), thumb.g(), thumb.b(), thumb.a()]);
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Track color:");
            let mut track = egui::Color32::from_rgba_unmultiplied(
                (settings.config.scrollbar_track_color[0] * 255.0) as u8,
                (settings.config.scrollbar_track_color[1] * 255.0) as u8,
                (settings.config.scrollbar_track_color[2] * 255.0) as u8,
                (settings.config.scrollbar_track_color[3] * 255.0) as u8,
            );
            if egui::color_picker::color_edit_button_srgba(
                ui,
                &mut track,
                egui::color_picker::Alpha::Opaque,
            )
            .changed()
            {
                settings.config.scrollbar_track_color =
                    color_u8x4_to_f32([track.r(), track.g(), track.b(), track.a()]);
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}

//! Display section of the window settings tab.

use crate::SettingsUI;
use crate::section::{SLIDER_WIDTH, collapsing_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub(super) fn show_display_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Display", "window_display", true, collapsed, |ui| {
        ui.horizontal(|ui| {
            ui.label("Title:");
            if ui
                .text_edit_singleline(&mut settings.config.window_title)
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.allow_title_change,
                "Allow apps to change window title",
            )
            .on_hover_text(
                "When enabled, terminal applications can change the window title via OSC escape sequences",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Columns:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.cols, 40..=300),
                )
                .on_hover_text("Number of columns in the terminal grid (determines window width)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Rows:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.rows, 10..=100),
                )
                .on_hover_text("Number of rows in the terminal grid (determines window height)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // Show current size and button to use it
        ui.horizontal(|ui| {
            let current_size = format!(
                "Current: {}x{}",
                settings.current_cols, settings.current_rows
            );
            ui.label(&current_size);

            // Show button (disabled if sizes already match)
            let differs = settings.current_cols != settings.config.cols
                || settings.current_rows != settings.config.rows;
            if ui
                .add_enabled(differs, egui::Button::new("Use Current Size"))
                .on_hover_text(if differs {
                    "Set the configured columns and rows to match the current window size"
                } else {
                    "Config already matches current window size"
                })
                .clicked()
            {
                settings.config.cols = settings.current_cols;
                settings.config.rows = settings.current_rows;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Padding:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.window_padding, 0.0..=50.0),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.hide_window_padding_on_split,
                "Hide padding on split",
            )
            .on_hover_text(
                "Automatically remove window padding when panes are split (panes have their own padding)",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

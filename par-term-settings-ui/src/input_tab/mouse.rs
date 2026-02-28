//! Mouse behavior settings section.

use crate::SettingsUI;
use crate::section::{SLIDER_WIDTH, collapsing_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

// ============================================================================
// Mouse Section
// ============================================================================

pub(super) fn show_mouse_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Mouse", "input_mouse", true, collapsed, |ui| {
        ui.horizontal(|ui| {
            ui.label("Scroll speed:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.mouse_scroll_speed, 0.1..=10.0),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Double-click threshold (ms):");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(
                        &mut settings.config.mouse_double_click_threshold,
                        100..=1000,
                    ),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Triple-click threshold (ms):");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(
                        &mut settings.config.mouse_triple_click_threshold,
                        100..=1000,
                    ),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.separator();
        ui.label("Advanced Mouse Features");

        #[cfg(target_os = "macos")]
        let option_click_label = "Option+Click moves cursor";
        #[cfg(not(target_os = "macos"))]
        let option_click_label = "Alt+Click moves cursor";

        if ui
            .checkbox(
                &mut settings.config.option_click_moves_cursor,
                option_click_label,
            )
            .on_hover_text("Position the text cursor at the clicked location")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.focus_follows_mouse,
                "Focus follows mouse",
            )
            .on_hover_text("Automatically focus the terminal window when the mouse enters it")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.report_horizontal_scroll,
                "Report horizontal scroll events",
            )
            .on_hover_text(
                "Report horizontal scroll to applications via mouse button codes 6 and 7",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

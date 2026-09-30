//! Mouse behavior settings section.

use crate::SettingsUI;
use crate::search::SearchTag;
use crate::section::{SLIDER_WIDTH, keyword_section};
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
    keyword_section(
        ui,
        "Mouse",
        "input_mouse",
        &["alt+click"],
        true,
        collapsed,
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Scroll speed:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.mouse.mouse_scroll_speed,
                            0.1..=10.0,
                        )
                        .suffix("×"),
                    )
                    .search_tag(&["mouse_scroll_speed"])
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                    &mut c.mouse.mouse_scroll_speed
                });
            });

            ui.horizontal(|ui| {
                ui.label("Double-click threshold:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.mouse.mouse_double_click_threshold,
                            100..=1000,
                        )
                        .suffix(" ms"),
                    )
                    .search_tag(&["mouse_double_click_threshold"])
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                    &mut c.mouse.mouse_double_click_threshold
                });
            });

            ui.horizontal(|ui| {
                ui.label("Triple-click threshold:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.mouse.mouse_triple_click_threshold,
                            100..=1000,
                        )
                        .suffix(" ms"),
                    )
                    .search_tag(&["mouse_triple_click_threshold"])
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                    &mut c.mouse.mouse_triple_click_threshold
                });
            });

            ui.separator();
            ui.label("Advanced Mouse Features");

            #[cfg(target_os = "macos")]
            let option_click_label = "Option+Click moves the cursor";
            #[cfg(not(target_os = "macos"))]
            let option_click_label = "Alt+Click moves the cursor";

            if ui
                .checkbox(
                    &mut settings.config.mouse.option_click_moves_cursor,
                    option_click_label,
                )
                .search_tag(&["option_click_moves_cursor"])
                .on_hover_text("Position the text cursor at the clicked location")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.mouse.focus_follows_mouse,
                    "Focus follows mouse",
                )
                .search_tag(&["focus_follows_mouse"])
                .on_hover_text("Automatically focus the terminal window when the mouse enters it")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.mouse.pane_focus_follows_mouse,
                    "Pane focus follows mouse",
                )
                .search_tag(&["pane_focus_follows_mouse"])
                .on_hover_text(
                    "Focus the split pane under the pointer as it moves, without a click. \
                 Not while a mouse button is held or a pane is zoomed.",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.mouse.report_horizontal_scroll,
                    "Report horizontal scroll events",
                )
                .search_tag(&["report_horizontal_scroll"])
                .on_hover_text(
                    "Report horizontal scroll to applications via mouse button codes 6 and 7",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        },
    );
}

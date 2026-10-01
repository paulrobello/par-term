//! Window behavior section of the window settings tab.

use crate::SettingsUI;
use crate::search::SearchTag;
use crate::section::keyword_section;
use par_term_config::WindowType;
use std::collections::HashSet;

pub(crate) fn show_behavior_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "Window Behavior",
        "window_behavior",
        &["edge-anchored", "primary monitor"],
        false,
        collapsed,
        |ui| {
            if ui
                .checkbox(
                    &mut settings.config.window.window_decorations,
                    "Window decorations",
                )
                .search_tag(&["window_decorations"])
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.window.window_always_on_top,
                    "Always on top",
                )
                .search_tag(&["window_always_on_top"])
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.placement.lock_window_size,
                    "Lock window size",
                )
                .search_tag(&["lock_window_size"])
                .on_hover_text("Prevent window from being resized by the user")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.placement.show_window_number,
                    "Show window number in title",
                )
                .search_tag(&["show_window_number"])
                .on_hover_text(
                    "Display window index number in the title bar (useful for multiple windows)",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(8.0);

            // Window type dropdown
            ui.horizontal(|ui| {
                ui.label("Window type:");
                let current_type = settings.config.placement.window_type;
                egui::ComboBox::from_id_salt("window_window_type")
                    .selected_text(current_type.display_name())
                    .show_ui(ui, |ui| {
                        for window_type in WindowType::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.placement.window_type,
                                    *window_type,
                                    window_type.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    })
                    .response
                    .search_tag(&["window_type"]);
            });

            // Target monitor setting
            ui.horizontal(|ui| {
                ui.label("Target monitor:");
                let mut monitor_index =
                    settings.config.placement.target_monitor.unwrap_or(0) as i32;
                let mut use_default = settings.config.placement.target_monitor.is_none();

                if ui
                    .checkbox(&mut use_default, "Auto")
                    .search_tag(&["target_monitor"])
                    .on_hover_text("Let the OS decide which monitor to open on")
                    .changed()
                {
                    if use_default {
                        settings.config.placement.target_monitor = None;
                    } else {
                        settings.config.placement.target_monitor = Some(0);
                    }
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if !use_default
                    && ui
                        .add(egui::Slider::new(&mut monitor_index, 0..=7))
                        .on_hover_text("Monitor index (0 = primary)")
                        .changed()
                {
                    settings.config.placement.target_monitor = Some(monitor_index as usize);
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            crate::deferred_badge(ui, crate::Deferred::NewWindows);

            // Target macOS Space setting (only visible on macOS)
            if cfg!(target_os = "macos") {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Target Space:").search_tag(&[
                        "mission control",
                        "macos space",
                        "target_space",
                    ]);
                    let mut space_number =
                        settings.config.placement.target_space.unwrap_or(1) as i32;
                    let mut use_default = settings.config.placement.target_space.is_none();

                    if ui
                        .checkbox(&mut use_default, "Auto")
                        .on_hover_text("Let the OS decide which Space (virtual desktop) to open on")
                        .changed()
                    {
                        if use_default {
                            settings.config.placement.target_space = None;
                        } else {
                            settings.config.placement.target_space = Some(1);
                        }
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }

                    if !use_default
                        && ui
                            .add(egui::Slider::new(&mut space_number, 1..=16))
                            .on_hover_text("Space number in Mission Control (1 = first Space)")
                            .changed()
                    {
                        settings.config.placement.target_space = Some(space_number as u32);
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
                crate::deferred_badge(ui, crate::Deferred::NewWindows);
            }
        },
    );
}

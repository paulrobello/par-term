//! Window behavior section of the window settings tab.

use crate::SettingsUI;
use crate::section::collapsing_section;
use par_term_config::WindowType;
use std::collections::HashSet;

pub(super) fn show_behavior_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Window Behavior",
        "window_behavior",
        false,
        collapsed,
        |ui| {
            if ui
                .checkbox(
                    &mut settings.config.window_decorations,
                    "Window decorations",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(&mut settings.config.window_always_on_top, "Always on top")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(&mut settings.config.lock_window_size, "Lock window size")
                .on_hover_text("Prevent window from being resized by the user")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.show_window_number,
                    "Show window number in title",
                )
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
                let current_type = settings.config.window_type;
                egui::ComboBox::from_id_salt("window_window_type")
                    .selected_text(current_type.display_name())
                    .show_ui(ui, |ui| {
                        for window_type in WindowType::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.window_type,
                                    *window_type,
                                    window_type.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            // Target monitor setting
            ui.horizontal(|ui| {
                ui.label("Target monitor:");
                let mut monitor_index = settings.config.target_monitor.unwrap_or(0) as i32;
                let mut use_default = settings.config.target_monitor.is_none();

                if ui
                    .checkbox(&mut use_default, "Auto")
                    .on_hover_text("Let the OS decide which monitor to open on")
                    .changed()
                {
                    if use_default {
                        settings.config.target_monitor = None;
                    } else {
                        settings.config.target_monitor = Some(0);
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
                    settings.config.target_monitor = Some(monitor_index as usize);
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            if settings.config.window_type.is_edge() {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "Note: Edge-anchored windows take effect on next window creation",
                );
            }

            // Target macOS Space setting (only visible on macOS)
            if cfg!(target_os = "macos") {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Target Space:");
                    let mut space_number = settings.config.target_space.unwrap_or(1) as i32;
                    let mut use_default = settings.config.target_space.is_none();

                    if ui
                        .checkbox(&mut use_default, "Auto")
                        .on_hover_text("Let the OS decide which Space (virtual desktop) to open on")
                        .changed()
                    {
                        if use_default {
                            settings.config.target_space = None;
                        } else {
                            settings.config.target_space = Some(1);
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
                        settings.config.target_space = Some(space_number as u32);
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "Note: Target Space takes effect on next window creation",
                );
            }
        },
    );
}

//! Performance section of the window settings tab.

use crate::SettingsUI;
use crate::section::{SLIDER_WIDTH, collapsing_section};
use par_term_config::{PowerPreference, VsyncMode};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub(super) fn show_performance_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Performance",
        "window_performance",
        false,
        collapsed,
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Max FPS:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(&mut settings.config.max_fps, 1..=240),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("VSync Mode:");
                let current = match settings.config.vsync_mode {
                    VsyncMode::Immediate => 0,
                    VsyncMode::Mailbox => 1,
                    VsyncMode::Fifo => 2,
                };
                let mut selected = current;

                // Helper to format mode name with support indicator
                let format_mode = |mode: VsyncMode, name: &str| -> String {
                    if settings.is_vsync_mode_supported(mode) {
                        name.to_string()
                    } else {
                        format!("{} (not supported)", name)
                    }
                };

                egui::ComboBox::from_id_salt("window_vsync_mode")
                    .selected_text(match current {
                        0 => format_mode(VsyncMode::Immediate, "Immediate (No VSync)"),
                        1 => format_mode(VsyncMode::Mailbox, "Mailbox (Balanced)"),
                        2 => format_mode(VsyncMode::Fifo, "FIFO (VSync)"),
                        _ => "Unknown".to_string(),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut selected,
                            0,
                            format_mode(VsyncMode::Immediate, "Immediate (No VSync)"),
                        );
                        ui.selectable_value(
                            &mut selected,
                            1,
                            format_mode(VsyncMode::Mailbox, "Mailbox (Balanced)"),
                        );
                        ui.selectable_value(
                            &mut selected,
                            2,
                            format_mode(VsyncMode::Fifo, "FIFO (VSync)"),
                        );
                    });
                if selected != current {
                    let new_mode = match selected {
                        0 => VsyncMode::Immediate,
                        1 => VsyncMode::Mailbox,
                        2 => VsyncMode::Fifo,
                        _ => VsyncMode::Immediate,
                    };

                    // Check if the mode is supported
                    if settings.is_vsync_mode_supported(new_mode) {
                        settings.config.vsync_mode = new_mode;
                        settings.vsync_warning = None;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    } else {
                        // Set warning and revert to Fifo (always supported)
                        settings.vsync_warning = Some(format!(
                            "{:?} is not supported on this display. Using FIFO instead.",
                            new_mode
                        ));
                        settings.config.vsync_mode = VsyncMode::Fifo;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                }
            });

            // Show vsync warning if present
            if let Some(ref warning) = settings.vsync_warning {
                ui.colored_label(egui::Color32::YELLOW, warning);
            }

            ui.horizontal(|ui| {
                ui.label("GPU Power Preference:");
                let current_pref = settings.config.power_preference;
                egui::ComboBox::from_id_salt("gpu_power_preference")
                    .selected_text(current_pref.display_name())
                    .show_ui(ui, |ui| {
                        for pref in PowerPreference::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.power_preference,
                                    *pref,
                                    pref.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });
            ui.colored_label(
                egui::Color32::GRAY,
                "Note: Requires app restart to take effect",
            );

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Power Saving").strong());

            if ui
                .checkbox(
                    &mut settings.config.pause_shaders_on_blur,
                    "Pause shader animations when unfocused",
                )
                .on_hover_text(
                    "Reduces GPU usage by pausing animated shaders when the window is not in focus",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.pause_refresh_on_blur,
                    "Reduce refresh rate when unfocused",
                )
                .on_hover_text(
                    "Reduces CPU/GPU usage by lowering the frame rate when the window is not in focus",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.horizontal(|ui| {
                ui.label("Unfocused FPS:");
                if ui
                    .add_enabled(
                        settings.config.pause_refresh_on_blur,
                        egui::Slider::new(&mut settings.config.unfocused_fps, 1..=30),
                    )
                    .on_hover_text(
                        "Target frame rate when window is unfocused (lower = more power savings)",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Inactive Tab FPS:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(&mut settings.config.inactive_tab_fps, 1..=30),
                    )
                    .on_hover_text(
                        "Refresh rate for non-visible tabs. Lower values reduce CPU usage\n\
                         from mutex polling when many tabs are open.\n\
                         Only needs to be high enough to detect activity, bells, and shell exit.",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Flicker Reduction").strong());

            if ui
                .checkbox(
                    &mut settings.config.reduce_flicker,
                    "Reduce flicker during fast updates",
                )
                .on_hover_text(
                    "Delays screen redraws while the cursor is hidden (DECTCEM off).\n\
                 Many terminal programs hide the cursor during bulk updates.\n\
                 This batches updates to reduce visual flicker.",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.horizontal(|ui| {
                ui.label("Maximum delay:");
                if ui
                    .add_enabled(
                        settings.config.reduce_flicker,
                        egui::Slider::new(&mut settings.config.reduce_flicker_delay_ms, 1..=100)
                            .suffix("ms"),
                    )
                    .on_hover_text(
                        "Maximum time to wait for cursor to become visible.\n\
                     Lower = more responsive, Higher = smoother for slow programs.\n\
                     Default: 16ms (~1 frame at 60fps)",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.add_space(10.0);
            ui.label(egui::RichText::new("Throughput Mode").strong());

            if ui
                .checkbox(&mut settings.config.maximize_throughput, {
                    #[cfg(target_os = "macos")]
                    {
                        "Maximize throughput (Cmd+Shift+T)"
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        "Maximize throughput (Ctrl+Shift+M)"
                    }
                })
                .on_hover_text(
                    "Batches screen updates during bulk terminal output.\n\
                 Reduces CPU overhead when processing large outputs.\n\
                 Trade-off: display updates are delayed by the interval below.",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.horizontal(|ui| {
                ui.label("Render interval:");
                if ui
                    .add_enabled(
                        settings.config.maximize_throughput,
                        egui::Slider::new(
                            &mut settings.config.throughput_render_interval_ms,
                            50..=500,
                        )
                        .suffix("ms"),
                    )
                    .on_hover_text(
                        "How often to update the display in throughput mode.\n\
                     Lower = more responsive, Higher = better throughput.\n\
                     Default: 100ms (~10 updates/sec)",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        },
    );
}

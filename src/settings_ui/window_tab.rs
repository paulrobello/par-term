use super::SettingsUI;
use crate::config::VsyncMode;

pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Window & Display", |ui| {
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

        ui.horizontal(|ui| {
            ui.label("Columns:");
            if ui
                .add(egui::Slider::new(&mut settings.config.cols, 40..=300))
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
                .add(egui::Slider::new(&mut settings.config.rows, 10..=100))
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
                "Current: {}×{}",
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

        ui.horizontal(|ui| {
            ui.label("Padding:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.window_padding,
                    0.0..=50.0,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Opacity:");
            let response = ui.add(egui::Slider::new(
                &mut settings.config.window_opacity,
                0.1..=1.0,
            ));
            if response.changed() {
                log::info!(
                    "Opacity slider changed to: {}",
                    settings.config.window_opacity
                );
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.transparency_affects_only_default_background,
                "Transparency affects only default background",
            )
            .on_hover_text(
                "When enabled, colored backgrounds (syntax highlighting, status bars) remain opaque for better readability",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(&mut settings.config.keep_text_opaque, "Keep text opaque")
            .on_hover_text(
                "When enabled, text is always rendered at full opacity regardless of window transparency",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        // Blur settings (macOS only)
        #[cfg(target_os = "macos")]
        {
            ui.add_space(4.0);

            if ui
                .checkbox(&mut settings.config.blur_enabled, "Enable window blur")
                .on_hover_text(
                    "Blur content behind the transparent window for better readability (requires transparency)",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if settings.config.blur_enabled {
                ui.horizontal(|ui| {
                    ui.label("Blur radius:");
                    // Convert u32 to i32 for slider, clamp to valid range
                    let mut radius_i32 = settings.config.blur_radius.min(64) as i32;
                    if ui
                        .add(egui::Slider::new(&mut radius_i32, 1..=64))
                        .on_hover_text("Blur intensity (higher = more blur)")
                        .changed()
                    {
                        settings.config.blur_radius = radius_i32 as u32;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            }
        }

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

        ui.horizontal(|ui| {
            ui.label("Max FPS:");
            if ui
                .add(egui::Slider::new(&mut settings.config.max_fps, 1..=240))
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

            egui::ComboBox::from_id_salt("vsync_mode")
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

        ui.separator();
        ui.label("Power Saving (when window loses focus):");

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
    });
}

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
            ui.label("Width:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.window_width,
                    400..=3840,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Height:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.window_height,
                    300..=2160,
                ))
                .changed()
            {
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
            .checkbox(&mut settings.config.window_decorations, "Window decorations")
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
            egui::ComboBox::from_id_salt("vsync_mode")
                .selected_text(match current {
                    0 => "Immediate (No VSync)",
                    1 => "Mailbox (Balanced)",
                    2 => "FIFO (VSync)",
                    _ => "Unknown",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut selected,
                        0,
                        "Immediate (No VSync)",
                    );
                    ui.selectable_value(&mut selected, 1, "Mailbox (Balanced)");
                    ui.selectable_value(&mut selected, 2, "FIFO (VSync)");
                });
            if selected != current {
                settings.config.vsync_mode = match selected {
                    0 => VsyncMode::Immediate,
                    1 => VsyncMode::Mailbox,
                    2 => VsyncMode::Fifo,
                    _ => VsyncMode::Immediate,
                };
                settings.has_changes = true;
            }
        });
    });
}

//! Tab bar settings UI.

use super::SettingsUI;

pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Tab Bar", |ui| {
        ui.label("Background Colors");
        ui.indent("tab_bg_colors", |ui| {
            ui.horizontal(|ui| {
                ui.label("Tab bar background:");
                let mut color = settings.config.tab_bar_background;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_bar_background = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Active tab:");
                let mut color = settings.config.tab_active_background;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_active_background = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Inactive tab:");
                let mut color = settings.config.tab_inactive_background;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_inactive_background = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Hovered tab:");
                let mut color = settings.config.tab_hover_background;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_hover_background = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        });

        ui.add_space(8.0);
        ui.label("Text Colors");
        ui.indent("tab_text_colors", |ui| {
            ui.horizontal(|ui| {
                ui.label("Active tab text:");
                let mut color = settings.config.tab_active_text;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_active_text = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Inactive tab text:");
                let mut color = settings.config.tab_inactive_text;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_inactive_text = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        });

        ui.add_space(8.0);
        ui.label("Indicator Colors");
        ui.indent("tab_indicator_colors", |ui| {
            ui.horizontal(|ui| {
                ui.label("Active indicator:");
                let mut color = settings.config.tab_active_indicator;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_active_indicator = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Activity indicator:");
                let mut color = settings.config.tab_activity_indicator;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_activity_indicator = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Bell indicator:");
                let mut color = settings.config.tab_bell_indicator;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_bell_indicator = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        });

        ui.add_space(8.0);
        ui.label("Close Button Colors");
        ui.indent("tab_close_colors", |ui| {
            ui.horizontal(|ui| {
                ui.label("Close button:");
                let mut color = settings.config.tab_close_button;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_close_button = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Close button hover:");
                let mut color = settings.config.tab_close_button_hover;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_close_button_hover = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        });
    });
}

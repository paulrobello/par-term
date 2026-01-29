use super::SettingsUI;
use crate::config::CursorStyle;

pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Cursor", |ui| {
        ui.horizontal(|ui| {
            ui.label("Style:");
            let current = match settings.config.cursor_style {
                CursorStyle::Block => 0,
                CursorStyle::Beam => 1,
                CursorStyle::Underline => 2,
            };
            let mut selected = current;
            egui::ComboBox::from_id_salt("cursor_style")
                .selected_text(match current {
                    0 => "Block",
                    1 => "Beam",
                    2 => "Underline",
                    _ => "Unknown",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected, 0, "Block");
                    ui.selectable_value(&mut selected, 1, "Beam");
                    ui.selectable_value(&mut selected, 2, "Underline");
                });
            if selected != current {
                settings.config.cursor_style = match selected {
                    0 => CursorStyle::Block,
                    1 => CursorStyle::Beam,
                    2 => CursorStyle::Underline,
                    _ => CursorStyle::Block,
                };
                settings.has_changes = true;
            }
        });

        if ui
            .checkbox(&mut settings.config.cursor_blink, "Cursor blink")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.horizontal(|ui| {
            ui.label("Blink interval (ms):");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.cursor_blink_interval,
                    100..=2000,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Color:");
            let mut color = [
                settings.config.cursor_color[0],
                settings.config.cursor_color[1],
                settings.config.cursor_color[2],
            ];
            if ui.color_edit_button_srgb(&mut color).changed() {
                settings.config.cursor_color = color;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);
        ui.label("Application Control Locks:");

        if ui
            .checkbox(
                &mut settings.config.lock_cursor_visibility,
                "Lock cursor visibility",
            )
            .on_hover_text("Prevent applications from hiding the cursor")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(&mut settings.config.lock_cursor_style, "Lock cursor style")
            .on_hover_text("Prevent applications from changing cursor style")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        // Disable lock_cursor_blink when lock_cursor_style is enabled (style lock already controls blink)
        ui.add_enabled_ui(!settings.config.lock_cursor_style, |ui| {
            if ui
                .checkbox(&mut settings.config.lock_cursor_blink, "Lock cursor blink")
                .on_hover_text(if settings.config.lock_cursor_style {
                    "Disabled: Lock cursor style already controls blink"
                } else {
                    "Prevent applications from enabling cursor blink"
                })
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}

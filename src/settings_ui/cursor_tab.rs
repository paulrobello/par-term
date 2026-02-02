use super::SettingsUI;
use crate::config::{CursorStyle, UnfocusedCursorStyle};

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

        // Cursor text color (only affects block cursor)
        ui.horizontal(|ui| {
            ui.label("Text color (block cursor):");

            // Checkbox to enable/disable custom cursor text color
            let mut use_custom_color = settings.config.cursor_text_color.is_some();
            if ui
                .checkbox(&mut use_custom_color, "")
                .on_hover_text(
                    "Enable custom text color under block cursor. \
                     When disabled, uses automatic contrast color.",
                )
                .changed()
            {
                if use_custom_color {
                    // Enable: use default black text color
                    settings.config.cursor_text_color = Some([0, 0, 0]);
                } else {
                    // Disable: use auto-contrast
                    settings.config.cursor_text_color = None;
                }
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            // Color picker (only shown when enabled)
            if let Some(ref mut text_color) = settings.config.cursor_text_color {
                let mut color = *text_color;
                if ui
                    .color_edit_button_srgb(&mut color)
                    .on_hover_text("Color of text displayed under the block cursor")
                    .changed()
                {
                    *text_color = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            } else {
                ui.label("(auto)")
                    .on_hover_text("Using automatic contrast color based on cursor brightness");
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

        // Cursor Enhancements section
        ui.add_space(8.0);
        ui.separator();
        ui.label("Cursor Enhancements:");

        // Unfocused cursor style
        ui.horizontal(|ui| {
            ui.label("When unfocused:");
            let current = match settings.config.unfocused_cursor_style {
                UnfocusedCursorStyle::Hollow => 0,
                UnfocusedCursorStyle::Same => 1,
                UnfocusedCursorStyle::Hidden => 2,
            };
            let mut selected = current;
            egui::ComboBox::from_id_salt("unfocused_cursor_style")
                .selected_text(match current {
                    0 => "Hollow (outline)",
                    1 => "Same",
                    2 => "Hidden",
                    _ => "Unknown",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected, 0, "Hollow (outline)");
                    ui.selectable_value(&mut selected, 1, "Same");
                    ui.selectable_value(&mut selected, 2, "Hidden");
                });
            if selected != current {
                settings.config.unfocused_cursor_style = match selected {
                    0 => UnfocusedCursorStyle::Hollow,
                    1 => UnfocusedCursorStyle::Same,
                    2 => UnfocusedCursorStyle::Hidden,
                    _ => UnfocusedCursorStyle::Hollow,
                };
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // Cursor Guide
        ui.add_space(4.0);
        if ui
            .checkbox(
                &mut settings.config.cursor_guide_enabled,
                "Cursor guide (horizontal line)",
            )
            .on_hover_text("Show a subtle horizontal line at the cursor row")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if settings.config.cursor_guide_enabled {
            ui.horizontal(|ui| {
                ui.label("Guide color:");
                let mut color = [
                    settings.config.cursor_guide_color[0],
                    settings.config.cursor_guide_color[1],
                    settings.config.cursor_guide_color[2],
                    settings.config.cursor_guide_color[3],
                ];
                if ui
                    .color_edit_button_srgba_unmultiplied(&mut color)
                    .changed()
                {
                    settings.config.cursor_guide_color = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }

        // Cursor Shadow
        ui.add_space(4.0);
        if ui
            .checkbox(&mut settings.config.cursor_shadow_enabled, "Cursor shadow")
            .on_hover_text("Add a drop shadow behind the cursor")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if settings.config.cursor_shadow_enabled {
            ui.horizontal(|ui| {
                ui.label("Shadow color:");
                let mut color = [
                    settings.config.cursor_shadow_color[0],
                    settings.config.cursor_shadow_color[1],
                    settings.config.cursor_shadow_color[2],
                    settings.config.cursor_shadow_color[3],
                ];
                if ui
                    .color_edit_button_srgba_unmultiplied(&mut color)
                    .changed()
                {
                    settings.config.cursor_shadow_color = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Shadow offset X:");
                if ui
                    .add(egui::Slider::new(
                        &mut settings.config.cursor_shadow_offset[0],
                        0.0..=10.0,
                    ))
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Shadow offset Y:");
                if ui
                    .add(egui::Slider::new(
                        &mut settings.config.cursor_shadow_offset[1],
                        0.0..=10.0,
                    ))
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Shadow blur:");
                if ui
                    .add(
                        egui::Slider::new(&mut settings.config.cursor_shadow_blur, 0.0..=20.0)
                            .suffix(" px"),
                    )
                    .on_hover_text("Blur radius for the cursor shadow (0 = sharp edge)")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }

        // Cursor Boost (Glow)
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Cursor boost (glow):");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.cursor_boost,
                    0.0..=1.0,
                ))
                .on_hover_text("Add a glow effect around the cursor for visibility")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if settings.config.cursor_boost > 0.0 {
            ui.horizontal(|ui| {
                ui.label("Boost color:");
                let mut color = [
                    settings.config.cursor_boost_color[0],
                    settings.config.cursor_boost_color[1],
                    settings.config.cursor_boost_color[2],
                ];
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.cursor_boost_color = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }
    });
}

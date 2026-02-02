//! Settings UI tab for split pane configuration
//!
//! This tab provides controls for:
//! - Divider appearance (width, color, hover color)
//! - Inactive pane dimming
//! - Pane title bars
//! - Focus indicator settings

use super::SettingsUI;

pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Split Panes", |ui| {
        ui.label("Configure split pane behavior and appearance");
        ui.add_space(8.0);

        // Divider Settings
        ui.heading("Dividers");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Divider Width:");
            let mut width = settings.config.pane_divider_width.unwrap_or(2.0);
            if ui
                .add(egui::Slider::new(&mut width, 1.0..=10.0).suffix(" px"))
                .on_hover_text("Visual width of dividers between panes")
                .changed()
            {
                settings.config.pane_divider_width = Some(width);
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Drag Hit Width:");
            if ui
                .add(
                    egui::Slider::new(&mut settings.config.pane_divider_hit_width, 4.0..=20.0)
                        .suffix(" px"),
                )
                .on_hover_text("Width of the drag area for resizing (larger = easier to grab)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Pane Padding:");
            if ui
                .add(
                    egui::Slider::new(&mut settings.config.pane_padding, 0.0..=20.0)
                        .suffix(" px"),
                )
                .on_hover_text("Padding inside panes (space between content and border/divider)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Divider Color:");
            let mut color = settings.config.pane_divider_color;
            let egui_color = egui::Color32::from_rgb(color[0], color[1], color[2]);
            let mut edit_color = egui_color;
            if ui.color_edit_button_srgba(&mut edit_color).changed() {
                color = [edit_color.r(), edit_color.g(), edit_color.b()];
                settings.config.pane_divider_color = color;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Hover Color:");
            let mut color = settings.config.pane_divider_hover_color;
            let egui_color = egui::Color32::from_rgb(color[0], color[1], color[2]);
            let mut edit_color = egui_color;
            if ui
                .color_edit_button_srgba(&mut edit_color)
                .on_hover_text("Color when hovering over a divider for resize")
                .changed()
            {
                color = [edit_color.r(), edit_color.g(), edit_color.b()];
                settings.config.pane_divider_hover_color = color;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(12.0);

        // Focus Indicator Settings
        ui.heading("Focus Indicator");
        ui.add_space(4.0);

        if ui
            .checkbox(
                &mut settings.config.pane_focus_indicator,
                "Show focus indicator",
            )
            .on_hover_text("Draw a border around the focused pane")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if settings.config.pane_focus_indicator {
            ui.horizontal(|ui| {
                ui.label("Focus Color:");
                let mut color = settings.config.pane_focus_color;
                let egui_color = egui::Color32::from_rgb(color[0], color[1], color[2]);
                let mut edit_color = egui_color;
                if ui.color_edit_button_srgba(&mut edit_color).changed() {
                    color = [edit_color.r(), edit_color.g(), edit_color.b()];
                    settings.config.pane_focus_color = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Focus Width:");
                if ui
                    .add(
                        egui::Slider::new(&mut settings.config.pane_focus_width, 1.0..=5.0)
                            .suffix(" px"),
                    )
                    .on_hover_text("Width of the focus indicator border")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }

        ui.add_space(12.0);

        // Inactive Pane Dimming
        ui.heading("Inactive Panes");
        ui.add_space(4.0);

        if ui
            .checkbox(
                &mut settings.config.dim_inactive_panes,
                "Dim inactive panes",
            )
            .on_hover_text("Reduce opacity of panes that don't have focus")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if settings.config.dim_inactive_panes {
            ui.horizontal(|ui| {
                ui.label("Inactive Opacity:");
                if ui
                    .add(egui::Slider::new(
                        &mut settings.config.inactive_pane_opacity,
                        0.3..=1.0,
                    ))
                    .on_hover_text("Opacity level for unfocused panes (1.0 = fully visible)")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }

        ui.add_space(12.0);

        // Pane Titles
        ui.heading("Pane Titles");
        ui.add_space(4.0);

        if ui
            .checkbox(&mut settings.config.show_pane_titles, "Show pane titles")
            .on_hover_text("Display a title bar at the top of each pane")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if settings.config.show_pane_titles {
            ui.horizontal(|ui| {
                ui.label("Title Height:");
                if ui
                    .add(
                        egui::Slider::new(&mut settings.config.pane_title_height, 14.0..=30.0)
                            .suffix(" px"),
                    )
                    .on_hover_text("Height of pane title bars")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }

        ui.add_space(12.0);

        // Limits
        ui.heading("Limits");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Max Panes:");
            if ui
                .add(egui::Slider::new(&mut settings.config.max_panes, 0..=32))
                .on_hover_text("Maximum number of panes per tab (0 = unlimited)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Min Pane Size:");
            if ui
                .add(
                    egui::Slider::new(&mut settings.config.pane_min_size, 5..=40)
                        .suffix(" cells"),
                )
                .on_hover_text("Minimum pane size in cells (prevents tiny unusable panes)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(12.0);

        // Background Integration
        ui.heading("Background Integration");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Pane Opacity:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.pane_background_opacity,
                    0.5..=1.0,
                ))
                .on_hover_text(
                    "Pane background opacity (lower values let background image/shader show through)",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(12.0);

        // Keyboard Shortcuts Info
        ui.heading("Keyboard Shortcuts");
        ui.add_space(4.0);

        ui.label("Split Panes:");
        ui.horizontal(|ui| {
            ui.label("• Horizontal:");
            ui.label(egui::RichText::new("Cmd+D").monospace());
        });
        ui.horizontal(|ui| {
            ui.label("• Vertical:");
            ui.label(egui::RichText::new("Cmd+Shift+D").monospace());
        });
        ui.horizontal(|ui| {
            ui.label("• Close Pane:");
            ui.label(egui::RichText::new("Cmd+Shift+W").monospace());
        });

        ui.add_space(8.0);
        ui.label("Navigate Panes:");
        ui.horizontal(|ui| {
            ui.label("• Arrow keys:");
            ui.label(egui::RichText::new("Cmd+Option+Arrow").monospace());
        });

        ui.add_space(8.0);
        ui.label("Resize Panes:");
        ui.horizontal(|ui| {
            ui.label("• Arrow keys:");
            ui.label(egui::RichText::new("Cmd+Option+Shift+Arrow").monospace());
        });
    });
}

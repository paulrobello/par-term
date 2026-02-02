//! Tab bar settings UI.

use super::SettingsUI;
use crate::config::TabBarMode;

/// Show tab bar settings section
pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Tab Bar", |ui| {
        // Tab Bar Visibility
        ui.label("Visibility");
        ui.indent("tab_visibility", |ui| {
            ui.horizontal(|ui| {
                ui.label("Show tab bar:");
                let current = match settings.config.tab_bar_mode {
                    TabBarMode::Always => 0,
                    TabBarMode::WhenMultiple => 1,
                    TabBarMode::Never => 2,
                };
                let mut selected = current;
                egui::ComboBox::from_id_salt("tab_bar_mode")
                    .selected_text(match current {
                        0 => "Always",
                        1 => "When multiple tabs",
                        2 => "Never",
                        _ => "Unknown",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut selected, 0, "Always");
                        ui.selectable_value(&mut selected, 1, "When multiple tabs");
                        ui.selectable_value(&mut selected, 2, "Never");
                    });
                if selected != current {
                    settings.config.tab_bar_mode = match selected {
                        0 => TabBarMode::Always,
                        1 => TabBarMode::WhenMultiple,
                        2 => TabBarMode::Never,
                        _ => TabBarMode::WhenMultiple,
                    };
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Tab bar height:");
                if ui
                    .add(
                        egui::Slider::new(&mut settings.config.tab_bar_height, 20.0..=50.0)
                            .step_by(1.0)
                            .suffix("px"),
                    )
                    .on_hover_text("Height of the tab bar in pixels")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            if ui
                .checkbox(
                    &mut settings.config.tab_show_index,
                    "Show tab index numbers",
                )
                .on_hover_text(
                    "Display tab numbers (1, 2, 3...) in tab titles for keyboard navigation",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);

        // Tab Behavior section
        ui.label("Tab Behavior");
        ui.indent("tab_behavior", |ui| {
            if ui
                .checkbox(
                    &mut settings.config.tab_inherit_cwd,
                    "New tabs inherit current directory",
                )
                .on_hover_text(
                    "When opening a new tab, start in the same directory as the current tab",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.horizontal(|ui| {
                ui.label("Maximum tabs:");
                // Convert usize to u32 for slider
                let mut max_tabs = settings.config.max_tabs as u32;
                if ui
                    .add(egui::Slider::new(&mut max_tabs, 0..=50))
                    .on_hover_text("Maximum number of tabs allowed (0 = unlimited)")
                    .changed()
                {
                    settings.config.max_tabs = max_tabs as usize;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                if settings.config.max_tabs == 0 {
                    ui.label("(unlimited)");
                }
            });
        });

        ui.add_space(8.0);

        // Tab Layout section
        ui.label("Tab Layout");
        ui.indent("tab_layout", |ui| {
            ui.horizontal(|ui| {
                ui.label("Minimum tab width:");
                if ui
                    .add(
                        egui::Slider::new(&mut settings.config.tab_min_width, 120.0..=512.0)
                            .step_by(1.0)
                            .suffix("px"),
                    )
                    .on_hover_text("Minimum width for tabs before horizontal scrolling is enabled")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
            ui.label(
                egui::RichText::new(
                    "Tabs spread equally; scroll buttons appear when space is limited",
                )
                .small()
                .weak(),
            );
        });

        ui.add_space(8.0);

        // Tab Border section
        ui.label("Tab Border");
        ui.indent("tab_border", |ui| {
            ui.horizontal(|ui| {
                ui.label("Border width:");
                if ui
                    .add(
                        egui::Slider::new(&mut settings.config.tab_border_width, 0.0..=3.0)
                            .step_by(0.5)
                            .suffix("px"),
                    )
                    .on_hover_text("Width of the border around each tab (0 = no border)")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Border color:");
                let mut color = settings.config.tab_border_color;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.tab_border_color = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        });

        ui.add_space(8.0);

        // Inactive Tab Dimming section
        ui.label("Inactive Tab Dimming");
        ui.indent("tab_dimming", |ui| {
            if ui
                .checkbox(&mut settings.config.dim_inactive_tabs, "Dim inactive tabs")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if settings.config.dim_inactive_tabs {
                ui.horizontal(|ui| {
                    ui.label("Opacity:");
                    if ui
                        .add(
                            egui::Slider::new(&mut settings.config.inactive_tab_opacity, 0.2..=1.0)
                                .step_by(0.05)
                                .suffix(""),
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
                ui.label(
                    egui::RichText::new("Hovered tabs temporarily restore full opacity")
                        .small()
                        .weak(),
                );
            }
        });

        ui.add_space(8.0);
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
                ui.label("Active tab border:");
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
        ui.label("Close Button");
        ui.indent("tab_close_button", |ui| {
            if ui
                .checkbox(
                    &mut settings.config.tab_show_close_button,
                    "Show close button on tabs",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
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

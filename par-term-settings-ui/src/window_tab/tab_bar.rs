//! Tab bar sections of the window settings tab.
//!
//! Contains:
//! - Tab Bar section (behavior and configuration)
//! - Tab Bar Appearance section (colors and visual styling)

use crate::SettingsUI;
use crate::section::collapsing_section;
use par_term_config::{TabBarMode, TabBarPosition, TabStyle, TabTitleMode};
use std::collections::HashSet;

pub(super) fn show_tab_bar_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Tab Bar", "window_tab_bar", true, collapsed, |ui| {
        // Tab style preset dropdown
        ui.horizontal(|ui| {
            ui.label("Tab style:");
            let current_style = settings.config.tab_style;
            egui::ComboBox::from_id_salt("window_tab_style")
                .selected_text(current_style.display_name())
                .show_ui(ui, |ui| {
                    for style in TabStyle::all() {
                        if ui
                            .selectable_value(
                                &mut settings.config.tab_style,
                                *style,
                                style.display_name(),
                            )
                            .changed()
                        {
                            settings.config.apply_tab_style();
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        // Show light/dark sub-style dropdowns when Automatic is selected
        if settings.config.tab_style == TabStyle::Automatic {
            ui.indent("auto_tab_style_indent", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Light tab style:");
                    let current = settings.config.light_tab_style;
                    egui::ComboBox::from_id_salt("window_light_tab_style")
                        .selected_text(current.display_name())
                        .show_ui(ui, |ui| {
                            for style in TabStyle::all_concrete() {
                                if ui
                                    .selectable_value(
                                        &mut settings.config.light_tab_style,
                                        *style,
                                        style.display_name(),
                                    )
                                    .changed()
                                {
                                    settings.has_changes = true;
                                    *changes_this_frame = true;
                                }
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("Dark tab style:");
                    let current = settings.config.dark_tab_style;
                    egui::ComboBox::from_id_salt("window_dark_tab_style")
                        .selected_text(current.display_name())
                        .show_ui(ui, |ui| {
                            for style in TabStyle::all_concrete() {
                                if ui
                                    .selectable_value(
                                        &mut settings.config.dark_tab_style,
                                        *style,
                                        style.display_name(),
                                    )
                                    .changed()
                                {
                                    settings.has_changes = true;
                                    *changes_this_frame = true;
                                }
                            }
                        });
                });
            });
        }

        ui.horizontal(|ui| {
            ui.label("Show tab bar:");
            let current = match settings.config.tab_bar_mode {
                TabBarMode::Always => 0,
                TabBarMode::WhenMultiple => 1,
                TabBarMode::Never => 2,
            };
            let mut selected = current;
            egui::ComboBox::from_id_salt("window_tab_bar_mode")
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
            ui.label("Tab title mode:");
            let current = match settings.config.tab_title_mode {
                TabTitleMode::Auto => 0,
                TabTitleMode::OscOnly => 1,
            };
            let mut selected = current;
            egui::ComboBox::from_id_salt("window_tab_title_mode")
                .selected_text(match current {
                    0 => "Auto (OSC + CWD)",
                    1 => "OSC only",
                    _ => "Unknown",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected, 0, "Auto (OSC + CWD)")
                        .on_hover_text("Use OSC title, fall back to working directory");
                    ui.selectable_value(&mut selected, 1, "OSC only")
                        .on_hover_text("Only use titles set by OSC escape sequences");
                });
            if selected != current {
                settings.config.tab_title_mode = match selected {
                    0 => TabTitleMode::Auto,
                    1 => TabTitleMode::OscOnly,
                    _ => TabTitleMode::Auto,
                };
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Position:");
            let current_position = settings.config.tab_bar_position;
            egui::ComboBox::from_id_salt("window_tab_bar_position")
                .selected_text(current_position.display_name())
                .show_ui(ui, |ui| {
                    for &pos in TabBarPosition::all() {
                        if ui
                            .selectable_value(
                                &mut settings.config.tab_bar_position,
                                pos,
                                pos.display_name(),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        // Show tab bar width slider only for Left position
        if settings.config.tab_bar_position == TabBarPosition::Left {
            ui.horizontal(|ui| {
                ui.label("Tab bar width:");
                if ui
                    .add(
                        egui::Slider::new(&mut settings.config.tab_bar_width, 100.0..=300.0)
                            .step_by(1.0)
                            .suffix("px"),
                    )
                    .on_hover_text("Width of the left tab bar panel")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }

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
            .on_hover_text("Display tab numbers (1, 2, 3...) in tab titles for keyboard navigation")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

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

        if ui
            .checkbox(
                &mut settings.config.tab_stretch_to_fill,
                "Stretch tabs to fill bar",
            )
            .on_hover_text("Make tabs share available width evenly when they fit without scrolling")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(&mut settings.config.tab_html_titles, "HTML tab titles")
            .on_hover_text(
                "Render limited HTML in tab titles: <b>, <i>, <u>, <span style=\"color:...\">",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);

        if ui
            .checkbox(
                &mut settings.config.tab_inherit_cwd,
                "New tabs inherit current directory",
            )
            .on_hover_text("When opening a new tab, start in the same directory as the current tab")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.show_profile_drawer_button,
                "Show profile drawer button",
            )
            .on_hover_text(
                "Show the profile drawer toggle button on the right edge of the terminal window",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.new_tab_shortcut_shows_profiles,
                "New tab shortcut shows profile picker",
            )
            .on_hover_text(
                "When enabled, the new tab keyboard shortcut (Cmd+T / Ctrl+Shift+T) shows a profile selection dropdown instead of immediately creating a default tab",
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
}

pub(super) fn show_tab_bar_appearance_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Tab Bar Appearance",
        "window_tab_bar_appearance",
        false,
        collapsed,
        |ui| {
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

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Border").strong());

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

            if ui
                .checkbox(
                    &mut settings.config.tab_inactive_outline_only,
                    "Inactive tabs outline only",
                )
                .on_hover_text("Render inactive tabs with just an outline border and no fill")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Inactive Tab Dimming").strong());

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
                                .step_by(0.05),
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            }

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Background Colors").strong());

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

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Text Colors").strong());

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

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Indicator Colors").strong());

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

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Close Button Colors").strong());

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
        },
    );
}

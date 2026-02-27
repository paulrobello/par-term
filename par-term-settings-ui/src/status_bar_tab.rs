//! Status bar settings tab.
//!
//! Contains:
//! - General: enable/disable, position, height
//! - Styling: colors, font, separator
//! - Auto-Hide: fullscreen, mouse inactivity
//! - Poll Intervals: system monitor, git branch
//! - Widgets: three-column layout with toggle/reorder/move controls

use super::SettingsUI;
use super::section::{SLIDER_WIDTH, collapsing_section, section_matches};
use par_term_config::StatusBarPosition;
use par_term_config::{StatusBarSection, StatusBarWidgetConfig, WidgetId};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

/// Show the status bar tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // General section
    if section_matches(
        &query,
        "General",
        &["enable", "status bar", "position", "height"],
    ) {
        show_general_section(ui, settings, changes_this_frame, collapsed);
    }

    // Styling section
    if section_matches(
        &query,
        "Styling",
        &[
            "color",
            "background",
            "foreground",
            "font",
            "separator",
            "opacity",
        ],
    ) {
        show_styling_section(ui, settings, changes_this_frame, collapsed);
    }

    // Auto-Hide section
    if section_matches(
        &query,
        "Auto-Hide",
        &["auto hide", "fullscreen", "mouse", "inactivity", "timeout"],
    ) {
        show_auto_hide_section(ui, settings, changes_this_frame, collapsed);
    }

    // Widget Options section
    if section_matches(
        &query,
        "Widget Options",
        &[
            "time", "format", "clock", "git", "ahead", "behind", "dirty", "status",
        ],
    ) {
        show_widget_options_section(ui, settings, changes_this_frame, collapsed);
    }

    // Poll Intervals section
    if section_matches(
        &query,
        "Poll Intervals",
        &["poll", "interval", "system", "git", "refresh"],
    ) {
        show_poll_intervals_section(ui, settings, changes_this_frame, collapsed);
    }

    // Widgets section
    if section_matches(
        &query,
        "Widgets",
        &[
            "widget",
            "clock",
            "cpu",
            "memory",
            "network",
            "git",
            "bell",
            "command",
            "directory",
            "hostname",
            "left",
            "center",
            "right",
            "custom",
            "update",
        ],
    ) {
        show_widgets_section(ui, settings, changes_this_frame, collapsed);
    }
}


// ============================================================================
// General Section
// ============================================================================

fn show_general_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "General", "status_bar_general", true, collapsed, |ui| {
        if ui
            .checkbox(&mut settings.config.status_bar_enabled, "Enable status bar")
            .on_hover_text("Show a configurable status bar with widgets")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);

        // Position dropdown
        ui.horizontal(|ui| {
            ui.label("Position:");
            egui::ComboBox::from_id_salt("status_bar_position")
                .selected_text(match settings.config.status_bar_position {
                    StatusBarPosition::Top => "Top",
                    StatusBarPosition::Bottom => "Bottom",
                })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(
                            &mut settings.config.status_bar_position,
                            StatusBarPosition::Top,
                            "Top",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                    if ui
                        .selectable_value(
                            &mut settings.config.status_bar_position,
                            StatusBarPosition::Bottom,
                            "Bottom",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
        });

        // Height slider
        ui.horizontal(|ui| {
            ui.label("Height:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.status_bar_height, 16.0..=40.0)
                        .suffix(" px")
                        .show_value(true),
                )
                .on_hover_text("Height of the status bar in logical pixels")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}

// ============================================================================
// Styling Section
// ============================================================================

fn show_styling_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Styling", "status_bar_styling", true, collapsed, |ui| {
        // Background color
        ui.horizontal(|ui| {
            ui.label("Background color:");
            let mut color = egui::Color32::from_rgb(
                settings.config.status_bar_bg_color[0],
                settings.config.status_bar_bg_color[1],
                settings.config.status_bar_bg_color[2],
            );
            if ui.color_edit_button_srgba(&mut color).changed() {
                settings.config.status_bar_bg_color = [color.r(), color.g(), color.b()];
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // Background opacity
        ui.horizontal(|ui| {
            ui.label("Background opacity:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.status_bar_bg_alpha, 0.0..=1.0)
                        .show_value(true),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(4.0);

        // Foreground color
        ui.horizontal(|ui| {
            ui.label("Text color:");
            let mut color = egui::Color32::from_rgb(
                settings.config.status_bar_fg_color[0],
                settings.config.status_bar_fg_color[1],
                settings.config.status_bar_fg_color[2],
            );
            if ui.color_edit_button_srgba(&mut color).changed() {
                settings.config.status_bar_fg_color = [color.r(), color.g(), color.b()];
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);

        // Font size
        ui.horizontal(|ui| {
            ui.label("Font size:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.status_bar_font_size, 8.0..=24.0)
                        .suffix(" pt")
                        .show_value(true),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);

        // Separator
        ui.horizontal(|ui| {
            ui.label("Separator:");
            if ui
                .add(
                    egui::TextEdit::singleline(&mut settings.config.status_bar_separator)
                        .hint_text(" | ")
                        .desired_width(80.0),
                )
                .on_hover_text("Text displayed between widgets in the same section")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}

// ============================================================================
// Auto-Hide Section
// ============================================================================

fn show_auto_hide_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Auto-Hide",
        "status_bar_auto_hide",
        false,
        collapsed,
        |ui| {
            if ui
                .checkbox(
                    &mut settings.config.status_bar_auto_hide_fullscreen,
                    "Hide in fullscreen",
                )
                .on_hover_text("Automatically hide the status bar when the window is fullscreen")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.status_bar_auto_hide_mouse_inactive,
                    "Hide on mouse inactivity",
                )
                .on_hover_text("Automatically hide the status bar when the mouse has been inactive")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            // Timeout slider (only shown when mouse inactivity hide is enabled)
            if settings.config.status_bar_auto_hide_mouse_inactive {
                ui.horizontal(|ui| {
                    ui.label("Timeout:");
                    if ui
                        .add_sized(
                            [SLIDER_WIDTH, SLIDER_HEIGHT],
                            egui::Slider::new(
                                &mut settings.config.status_bar_mouse_inactive_timeout,
                                1.0..=30.0,
                            )
                            .suffix(" sec")
                            .show_value(true),
                        )
                        .on_hover_text(
                            "Seconds of mouse inactivity before the status bar is hidden",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            }
        },
    );
}

// ============================================================================
// Widget Options Section
// ============================================================================

fn show_widget_options_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Widget Options",
        "status_bar_widget_options",
        true,
        collapsed,
        |ui| {
            // Time format
            ui.horizontal(|ui| {
                ui.label("Time format:");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut settings.config.status_bar_time_format)
                            .hint_text("%H:%M:%S")
                            .desired_width(120.0),
                    )
                    .on_hover_text("strftime format string for the Clock widget (e.g. %H:%M:%S, %I:%M %p, %H:%M)")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Expandable format reference
            ui.collapsing("Format codes reference", |ui| {
                let dim = egui::Color32::from_rgb(140, 140, 140);
                let bright = egui::Color32::from_rgb(210, 210, 210);
                egui::Grid::new("time_format_help")
                    .num_columns(2)
                    .spacing([16.0, 2.0])
                    .show(ui, |ui| {
                        let rows: &[(&str, &str)] = &[
                            ("%H", "Hour 00–23"),
                            ("%I", "Hour 01–12"),
                            ("%M", "Minute 00–59"),
                            ("%S", "Second 00–59"),
                            ("%p", "AM / PM"),
                            ("%P", "am / pm"),
                            ("%Y", "Year (2026)"),
                            ("%m", "Month 01–12"),
                            ("%d", "Day 01–31"),
                            ("%a", "Weekday (Mon)"),
                            ("%A", "Weekday (Monday)"),
                            ("%b", "Month (Jan)"),
                            ("%B", "Month (January)"),
                            ("%Z", "Timezone (UTC)"),
                            ("%%", "Literal %"),
                        ];
                        for (code, desc) in rows {
                            ui.label(egui::RichText::new(*code).color(bright).monospace());
                            ui.label(egui::RichText::new(*desc).color(dim).small());
                            ui.end_row();
                        }
                    });
            });

            ui.add_space(8.0);

            // Git show status
            if ui
                .checkbox(
                    &mut settings.config.status_bar_git_show_status,
                    "Show git ahead/behind and dirty status",
                )
                .on_hover_text(
                    "Display commit counts ahead/behind upstream and a dirty indicator on the Git Branch widget",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        },
    );
}

// ============================================================================
// Poll Intervals Section
// ============================================================================

fn show_poll_intervals_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Poll Intervals",
        "status_bar_poll_intervals",
        false,
        collapsed,
        |ui| {
            ui.horizontal(|ui| {
                ui.label("System monitor:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.status_bar_system_poll_interval,
                            0.5..=30.0,
                        )
                        .suffix(" sec")
                        .show_value(true),
                    )
                    .on_hover_text("How often to poll CPU, memory, and network usage")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Git branch:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.status_bar_git_poll_interval,
                            1.0..=60.0,
                        )
                        .suffix(" sec")
                        .show_value(true),
                    )
                    .on_hover_text("How often to poll the current git branch name")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        },
    );
}

// ============================================================================
// Widgets Section
// ============================================================================

fn show_widgets_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Widgets", "status_bar_widgets", true, collapsed, |ui| {
        ui.label(
            egui::RichText::new("Click a widget to toggle it. Right-click for more options.")
                .small()
                .color(egui::Color32::GRAY),
        );
        ui.add_space(8.0);

        // Collect pending mutations to apply after iteration
        let mut toggle_index: Option<usize> = None;
        let mut move_to_section: Option<(usize, StatusBarSection)> = None;
        let mut swap_pair: Option<(usize, usize)> = None;
        let mut delete_index: Option<usize> = None;

        // Show three columns: Left, Center, Right
        let sections = [
            ("Left", StatusBarSection::Left),
            ("Center", StatusBarSection::Center),
            ("Right", StatusBarSection::Right),
        ];

        for (section_label, section) in &sections {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!("{section_label} Section"))
                    .strong()
                    .color(egui::Color32::from_rgb(180, 180, 220)),
            );
            ui.separator();

            // Gather indices of widgets in this section, sorted by order
            let mut section_indices: Vec<usize> = settings
                .config
                .status_bar_widgets
                .iter()
                .enumerate()
                .filter(|(_, w)| w.section == *section)
                .map(|(i, _)| i)
                .collect();
            section_indices.sort_by_key(|&i| settings.config.status_bar_widgets[i].order);

            if section_indices.is_empty() {
                ui.label(
                    egui::RichText::new("  (empty)")
                        .small()
                        .color(egui::Color32::GRAY),
                );
            }

            for (pos, &widget_idx) in section_indices.iter().enumerate() {
                let w = &settings.config.status_bar_widgets[widget_idx];
                let icon = w.id.icon();
                let label = w.id.label();
                let enabled = w.enabled;
                let is_custom = matches!(w.id, WidgetId::Custom(_));

                let text_color = if enabled {
                    egui::Color32::from_rgb(220, 220, 220)
                } else {
                    egui::Color32::from_rgb(100, 100, 100)
                };

                let status_indicator = if enabled { "[ON]" } else { "[OFF]" };

                let button = egui::Button::new(
                    egui::RichText::new(format!("{icon}  {label}  {status_indicator}"))
                        .color(text_color)
                        .size(13.0),
                )
                .fill(if enabled {
                    egui::Color32::from_rgb(40, 40, 55)
                } else {
                    egui::Color32::from_rgb(30, 30, 35)
                })
                .min_size(egui::vec2(ui.available_width() - 20.0, 28.0));

                let response = ui.add(button);

                // Left-click toggles
                if response.clicked() {
                    toggle_index = Some(widget_idx);
                }

                // Right-click context menu
                response.context_menu(|ui| {
                    // Move to other sections
                    let other_sections: Vec<(&str, StatusBarSection)> = sections
                        .iter()
                        .filter(|(_, s)| s != section)
                        .map(|(l, s)| (*l, *s))
                        .collect();

                    for (target_label, target_section) in &other_sections {
                        if ui.button(format!("Move to {target_label}")).clicked() {
                            move_to_section = Some((widget_idx, *target_section));
                            ui.close();
                        }
                    }

                    ui.separator();

                    // Move up/down within section
                    if pos > 0 && ui.button("Move Up").clicked() {
                        swap_pair = Some((widget_idx, section_indices[pos - 1]));
                        ui.close();
                    }
                    if pos + 1 < section_indices.len() && ui.button("Move Down").clicked() {
                        swap_pair = Some((widget_idx, section_indices[pos + 1]));
                        ui.close();
                    }

                    // Delete custom widgets
                    if is_custom {
                        ui.separator();
                        if ui
                            .button(
                                egui::RichText::new("Delete")
                                    .color(egui::Color32::from_rgb(220, 80, 80)),
                            )
                            .clicked()
                        {
                            delete_index = Some(widget_idx);
                            ui.close();
                        }
                    }
                });

                // Show format editor for custom widgets inline
                if is_custom
                    && enabled
                    && let Some(ref mut fmt) = settings.config.status_bar_widgets[widget_idx].format
                {
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.label(
                            egui::RichText::new("Format:")
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                        if ui
                            .add(
                                egui::TextEdit::singleline(fmt)
                                    .hint_text("custom text")
                                    .desired_width(200.0),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });
                }
            }
        }

        // Apply mutations
        if let Some(idx) = toggle_index {
            settings.config.status_bar_widgets[idx].enabled =
                !settings.config.status_bar_widgets[idx].enabled;
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if let Some((idx, new_section)) = move_to_section {
            // Find max order in new section
            let max_order = settings
                .config
                .status_bar_widgets
                .iter()
                .filter(|w| w.section == new_section)
                .map(|w| w.order)
                .max()
                .unwrap_or(-1);
            settings.config.status_bar_widgets[idx].section = new_section;
            settings.config.status_bar_widgets[idx].order = max_order + 1;
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if let Some((a, b)) = swap_pair {
            let order_a = settings.config.status_bar_widgets[a].order;
            let order_b = settings.config.status_bar_widgets[b].order;
            settings.config.status_bar_widgets[a].order = order_b;
            settings.config.status_bar_widgets[b].order = order_a;
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if let Some(idx) = delete_index {
            settings.config.status_bar_widgets.remove(idx);
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        // Add custom widget button
        ui.add_space(12.0);
        if ui
            .button("+ Add Custom Text Widget")
            .on_hover_text("Add a custom text widget with a format string")
            .clicked()
        {
            let custom_name = format!("Custom {}", settings.config.status_bar_widgets.len());
            let max_order = settings
                .config
                .status_bar_widgets
                .iter()
                .filter(|w| w.section == StatusBarSection::Left)
                .map(|w| w.order)
                .max()
                .unwrap_or(-1);
            settings
                .config
                .status_bar_widgets
                .push(StatusBarWidgetConfig {
                    id: WidgetId::Custom(custom_name),
                    enabled: true,
                    section: StatusBarSection::Left,
                    order: max_order + 1,
                    format: Some("custom text".to_string()),
                });
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

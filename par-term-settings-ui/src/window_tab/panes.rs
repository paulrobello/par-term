//! Split pane sections of the window settings tab.
//!
//! Contains:
//! - Split Panes section (behavior and sizing)
//! - Pane Appearance section (colors and visual styling)

use crate::SettingsUI;
use crate::section::collapsing_section;
use par_term_config::{DividerStyle, PaneTitlePosition};
use std::collections::HashSet;

pub(super) fn show_panes_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Split Panes", "window_panes", true, collapsed, |ui| {
        ui.label("Configure split pane behavior and appearance");
        ui.add_space(8.0);

        ui.label(egui::RichText::new("Dividers").strong());

        ui.horizontal(|ui| {
            ui.label("Divider Width:");
            let mut width = settings.config.panes.pane_divider_width.unwrap_or(2.0);
            if ui
                .add(egui::Slider::new(&mut width, 1.0..=10.0).suffix(" px"))
                .on_hover_text("Visual width of dividers between panes")
                .changed()
            {
                settings.config.panes.pane_divider_width = Some(width);
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Drag Hit Width:");
            if ui
                .add(
                    egui::Slider::new(
                        &mut settings.config.panes.pane_divider_hit_width,
                        4.0..=20.0,
                    )
                    .suffix(" px"),
                )
                .on_hover_text("Width of the drag area for resizing (larger = easier to grab)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                &mut c.panes.pane_divider_hit_width
            });
        });

        ui.horizontal(|ui| {
            ui.label("Pane Padding:");
            if ui
                .add(
                    egui::Slider::new(&mut settings.config.panes.pane_padding, 0.0..=20.0)
                        .suffix(" px"),
                )
                .on_hover_text("Padding inside panes (space between content and border/divider)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                &mut c.panes.pane_padding
            });
        });

        ui.horizontal(|ui| {
            ui.label("Divider Style:");
            let current_style = settings.config.panes.pane_divider_style;
            egui::ComboBox::from_id_salt("pane_divider_style")
                .selected_text(current_style.display_name())
                .show_ui(ui, |ui| {
                    for style in DividerStyle::ALL {
                        if ui
                            .selectable_value(
                                &mut settings.config.panes.pane_divider_style,
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

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Focus Indicator").strong());

        if ui
            .checkbox(
                &mut settings.config.panes.pane_focus_indicator,
                "Show focus indicator",
            )
            .on_hover_text("Draw a border around the focused pane")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        crate::dependent::dependent(
            ui,
            settings.config.panes.pane_focus_indicator,
            "Show focus indicator",
            |ui| {
                ui.horizontal(|ui| {
                    ui.label("Focus Color:");
                    if crate::color_helpers::rgb_color_button(
                        ui,
                        &mut settings.config.panes.pane_focus_color,
                    )
                    .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Focus Width:");
                    if ui
                        .add(
                            egui::Slider::new(
                                &mut settings.config.panes.pane_focus_width,
                                1.0..=5.0,
                            )
                            .suffix(" px"),
                        )
                        .on_hover_text("Width of the focus indicator border")
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                    crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                        &mut c.panes.pane_focus_width
                    });
                });
            },
        );

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Limits").strong());

        ui.horizontal(|ui| {
            ui.label("Max Panes:");
            if ui
                .add(
                    egui::Slider::new(&mut settings.config.panes.max_panes, 0..=32)
                        .suffix(" panes"),
                )
                .on_hover_text("Maximum number of panes per tab (0 = unlimited)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                &mut c.panes.max_panes
            });
        });

        ui.horizontal(|ui| {
            ui.label("Min Pane Size:");
            if ui
                .add(
                    egui::Slider::new(&mut settings.config.panes.pane_min_size, 5..=40)
                        .suffix(" cells"),
                )
                .on_hover_text("Minimum pane size in cells (prevents tiny unusable panes)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                &mut c.panes.pane_min_size
            });
        });

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Keyboard Shortcuts").weak().small());
        // DOC13: the live bindings, so a rebind shows here and an unbound
        // action is left out rather than advertised.
        for line in pane_shortcut_lines(&settings.config.keybindings) {
            ui.label(egui::RichText::new(format!("  {line}")).weak().small());
        }
    });
}

/// Pane actions listed under the Panes section's shortcut hint, in order.
const PANE_HINT_ACTIONS: &[(&str, &str)] = &[
    ("split_right", "Split right"),
    ("split_down", "Split down"),
    ("close_pane", "Close pane"),
    ("navigate_pane_left", "Focus left (arrows likewise)"),
    ("resize_pane_left", "Resize left (arrows likewise)"),
    ("swap_pane_left", "Swap left (arrows likewise)"),
    ("select_pane_hint", "Select pane by letter"),
    ("toggle_broadcast_input", "Broadcast input"),
];

/// `"<chord>: <label>"` for each pane action `keybindings` binds. An
/// action's first binding is its primary chord.
fn pane_shortcut_lines(keybindings: &[par_term_config::KeyBinding]) -> Vec<String> {
    PANE_HINT_ACTIONS
        .iter()
        .filter_map(|(action, label)| {
            keybindings
                .iter()
                .find(|kb| kb.action == *action)
                .map(|kb| format!("{}: {label}", crate::input_tab::display_key_combo(&kb.key)))
        })
        .collect()
}

pub(super) fn show_pane_appearance_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Pane Appearance",
        "window_pane_appearance",
        false,
        collapsed,
        |ui| {
            ui.label(egui::RichText::new("Divider Colors").strong());

            ui.horizontal(|ui| {
                ui.label("Divider Color:");
                if crate::color_helpers::rgb_color_button(
                    ui,
                    &mut settings.config.panes.pane_divider_color,
                )
                .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Hover Color:");
                if crate::color_helpers::rgb_color_button(
                    ui,
                    &mut settings.config.panes.pane_divider_hover_color,
                )
                .on_hover_text("Color when hovering over a divider for resize")
                .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Inactive Panes").strong());

            if ui
                .checkbox(
                    &mut settings.config.panes.dim_inactive_panes,
                    "Dim inactive panes",
                )
                .on_hover_text("Reduce opacity of panes that don't have focus")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            crate::dependent::dependent(
                ui,
                settings.config.panes.dim_inactive_panes,
                "Dim inactive panes",
                |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Inactive Opacity:");
                        if ui
                            .add(crate::units::percent(egui::Slider::new(
                                &mut settings.config.panes.inactive_pane_opacity,
                                0.3..=1.0,
                            )))
                            .on_hover_text(
                                "Opacity level for unfocused panes (1.0 = fully visible)",
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                            &mut c.panes.inactive_pane_opacity
                        });
                    });
                },
            );

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Pane Titles").strong());

            if ui
                .checkbox(
                    &mut settings.config.panes.show_pane_titles,
                    "Show pane titles",
                )
                .on_hover_text("Display a title bar at the top of each pane")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            crate::dependent::dependent(
                ui,
                settings.config.panes.show_pane_titles,
                "Show pane titles",
                |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Title Height:");
                        if ui
                            .add(
                                egui::Slider::new(
                                    &mut settings.config.panes.pane_title_height,
                                    14.0..=30.0,
                                )
                                .suffix(" px"),
                            )
                            .on_hover_text("Height of pane title bars")
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                            &mut c.panes.pane_title_height
                        });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Title Position:");
                        let current_pos = settings.config.panes.pane_title_position;
                        egui::ComboBox::from_id_salt("pane_title_position")
                            .selected_text(current_pos.display_name())
                            .show_ui(ui, |ui| {
                                for pos in PaneTitlePosition::ALL {
                                    if ui
                                        .selectable_value(
                                            &mut settings.config.panes.pane_title_position,
                                            *pos,
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

                    ui.horizontal(|ui| {
                        ui.label("Title text color:");
                        let mut color = settings.config.panes.pane_title_color;
                        if ui.color_edit_button_srgb(&mut color).changed() {
                            settings.config.panes.pane_title_color = color;
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Title background:");
                        let mut color = settings.config.panes.pane_title_bg_color;
                        if ui.color_edit_button_srgb(&mut color).changed() {
                            settings.config.panes.pane_title_bg_color = color;
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });
                },
            );

            ui.add_space(8.0);
            ui.label(egui::RichText::new("Background Integration").strong());

            ui.horizontal(|ui| {
                ui.label("Pane Opacity:");
                if ui
                    .add(crate::units::percent(egui::Slider::new(
                        &mut settings.config.panes.pane_background_opacity,
                        0.5..=1.0,
                    )))
                    .on_hover_text(
                        "Pane background opacity (lower values let background image/shader show through)",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                crate::reset::reset_button(ui, settings, changes_this_frame, |c| &mut c.panes.pane_background_opacity);
            });
        },
    );
}

#[cfg(test)]
mod tests {
    use super::pane_shortcut_lines;
    use par_term_config::KeyBinding;

    /// DOC13: the hint shows the live chord and omits unbound actions.
    #[test]
    fn pane_hint_reads_the_live_bindings() {
        let lines = pane_shortcut_lines(&[KeyBinding {
            key: "F9".to_string(),
            action: "split_right".to_string(),
        }]);
        assert_eq!(lines, ["F9: Split right"]);
    }
}

//! Status bar widgets section — three-column layout with toggle, reorder, and move controls.

use crate::SettingsUI;
use crate::section::collapsing_section;
use par_term_config::{StatusBarSection, StatusBarWidgetConfig, WidgetId};
use std::collections::HashSet;

pub fn show_widgets_section(
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
                .status_bar
                .status_bar_widgets
                .iter()
                .enumerate()
                .filter(|(_, w)| w.section == *section)
                .map(|(i, _)| i)
                .collect();
            section_indices
                .sort_by_key(|&i| settings.config.status_bar.status_bar_widgets[i].order);

            if section_indices.is_empty() {
                ui.label(
                    egui::RichText::new("  (empty)")
                        .small()
                        .color(egui::Color32::GRAY),
                );
            }

            for (pos, &widget_idx) in section_indices.iter().enumerate() {
                let w = &settings.config.status_bar.status_bar_widgets[widget_idx];
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
                    && let Some(ref mut fmt) =
                        settings.config.status_bar.status_bar_widgets[widget_idx].format
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
            settings.config.status_bar.status_bar_widgets[idx].enabled =
                !settings.config.status_bar.status_bar_widgets[idx].enabled;
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if let Some((idx, new_section)) = move_to_section {
            // Find max order in new section
            let max_order = settings
                .config
                .status_bar
                .status_bar_widgets
                .iter()
                .filter(|w| w.section == new_section)
                .map(|w| w.order)
                .max()
                .unwrap_or(-1);
            settings.config.status_bar.status_bar_widgets[idx].section = new_section;
            settings.config.status_bar.status_bar_widgets[idx].order = max_order + 1;
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if let Some((a, b)) = swap_pair {
            let order_a = settings.config.status_bar.status_bar_widgets[a].order;
            let order_b = settings.config.status_bar.status_bar_widgets[b].order;
            settings.config.status_bar.status_bar_widgets[a].order = order_b;
            settings.config.status_bar.status_bar_widgets[b].order = order_a;
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if let Some(idx) = delete_index {
            settings.config.status_bar.status_bar_widgets.remove(idx);
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
            let custom_name = format!(
                "Custom {}",
                settings.config.status_bar.status_bar_widgets.len()
            );
            let max_order = settings
                .config
                .status_bar
                .status_bar_widgets
                .iter()
                .filter(|w| w.section == StatusBarSection::Left)
                .map(|w| w.order)
                .max()
                .unwrap_or(-1);
            settings
                .config
                .status_bar
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

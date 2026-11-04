//! Status bar widgets section — three draggable columns: click toggles,
//! drag reorders within a section and moves across sections.

use crate::SettingsUI;
use crate::section::keyword_section;
use par_term_config::{StatusBarSection, StatusBarWidgetConfig, WidgetId};
use std::collections::HashSet;

/// The sections in display order.
const SECTIONS: [(&str, StatusBarSection); 3] = [
    ("Left", StatusBarSection::Left),
    ("Center", StatusBarSection::Center),
    ("Right", StatusBarSection::Right),
];

/// The insertion-line accent.
const INSERT_LINE_COLOR: egui::Color32 = egui::Color32::from_rgb(120, 170, 255);

/// Move the widget at `from` to position `to_pos` within `target_section`'s
/// ordered list — `to_pos` counts positions that exclude the mover (dropping
/// "above row at pos" stays at `pos`), clamped to append. Every section is
/// renumbered densely from 0, so `order` values stay tidy forever.
/// Positional only: every other field travels with the widget.
fn reorder_widget(
    widgets: &mut [StatusBarWidgetConfig],
    from: usize,
    target_section: StatusBarSection,
    to_pos: usize,
) {
    debug_assert!(from < widgets.len());
    widgets[from].section = target_section;
    for (_, section) in SECTIONS {
        let mut members: Vec<usize> = (0..widgets.len())
            .filter(|&i| widgets[i].section == section)
            .collect();
        members.sort_by_key(|&i| widgets[i].order);
        if section == target_section {
            let here = members
                .iter()
                .position(|&i| i == from)
                .expect("mover in its new section");
            members.remove(here);
            let to_pos = to_pos.min(members.len());
            members.insert(to_pos, from);
        }
        for (order, &i) in members.iter().enumerate() {
            widgets[i].order = order as i32;
        }
    }
}

/// One live drop candidate: where a release would land this frame.
struct DropTarget {
    section: StatusBarSection,
    /// Insert position excluding the mover (clamped inside).
    pos: usize,
    line_y: f32,
    line_x: (f32, f32),
}

/// The Widgets section: three columns (Left / Center / Right), one row per
/// widget. A click (no drag) toggles; a drag reorders — drop above or below
/// any row in any column, or on a column's empty area to append. The
/// right-click menu keeps Move Up / Move Down / Move to <section> / Delete
/// as a keyboard-free fallback.
pub fn show_widgets_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "Widgets",
        "status_bar_widgets",
        &[
            "hostname",
            "storage",
            "par-mux",
            "mux",
            "status_bar_widgets",
        ],
        true,
        collapsed,
        |ui| {
            crate::dependent::dependent(
                ui,
                settings.config.status_bar.status_bar_enabled,
                "Enable status bar",
                |ui| {
                    ui.label(
                        egui::RichText::new(
                            "Click a widget to toggle it. Drag it to reorder within its section or move it to another section. Right-click for more options.",
                        )
                        .small()
                        .color(egui::Color32::GRAY),
                    );
                    ui.add_space(8.0);

                    let ctx = ui.ctx().clone();
                    let mut toggle_index: Option<usize> = None;
                    let mut menu_move: Option<(usize, StatusBarSection, usize)> = None;
                    let mut delete_index: Option<usize> = None;
                    let mut hover_drop: Option<DropTarget> = None;
                    let dragging: Option<std::sync::Arc<usize>> =
                        egui::DragAndDrop::payload::<usize>(&ctx);

                    ui.columns(3, |columns| {
                        for (col, (section_label, section)) in SECTIONS.iter().enumerate() {
                            let column = &mut columns[col];
                            column.add_space(2.0);
                            column.label(
                                egui::RichText::new(format!("{section_label} Section"))
                                    .strong()
                                    .color(egui::Color32::from_rgb(180, 180, 220)),
                            );
                            column.separator();

                            // Ordered indices of this section, sorted by order.
                            let mut section_indices: Vec<usize> = settings
                                .config
                                .status_bar
                                .status_bar_widgets
                                .iter()
                                .enumerate()
                                .filter(|(_, w)| w.section == *section)
                                .map(|(i, _)| i)
                                .collect();
                            section_indices.sort_by_key(|&i| {
                                settings.config.status_bar.status_bar_widgets[i].order
                            });

                            if section_indices.is_empty() {
                                column.label(
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
                                let widget_key = format!("{widget_idx}:{label}");
                                let is_dragged =
                                    dragging.as_ref().is_some_and(|src| **src == widget_idx);

                                let text_color = if enabled {
                                    egui::Color32::from_rgb(220, 220, 220)
                                } else {
                                    egui::Color32::from_rgb(100, 100, 100)
                                };

                                let status_indicator = if enabled { "[ON]" } else { "[OFF]" };

                                let button = egui::Button::new(
                                    egui::RichText::new(format!(
                                        "{icon}  {label}  {status_indicator}"
                                    ))
                                    .color(text_color)
                                    .size(13.0),
                                )
                                .fill(if is_dragged {
                                    egui::Color32::from_rgb(55, 55, 75)
                                } else if enabled {
                                    egui::Color32::from_rgb(40, 40, 55)
                                } else {
                                    egui::Color32::from_rgb(30, 30, 35)
                                })
                                .min_size(egui::vec2(column.available_width() - 8.0, 28.0))
                                .sense(egui::Sense::click_and_drag());

                                let response = column.add(button);
                                if response.drag_started() {
                                    egui::DragAndDrop::set_payload(&ctx, widget_idx);
                                }

                                // Live drop candidate while a drag is in flight.
                                if let Some(src) = dragging.as_ref() {
                                    let src_idx = **src;
                                    if src_idx != widget_idx
                                        && column.rect_contains_pointer(response.rect)
                                    {
                                        let pointer_pos = ctx
                                            .input(|i| i.pointer.latest_pos())
                                            .unwrap_or(response.rect.center());
                                        let top_half = pointer_pos.y < response.rect.center().y;
                                        let mut insert = if top_half { pos } else { pos + 1 };
                                        let src_section =
                                            settings.config.status_bar.status_bar_widgets[src_idx]
                                                .section;
                                        if src_section == *section
                                            && let Some(src_pos) =
                                                section_indices.iter().position(|&i| i == src_idx)
                                            && insert > src_pos
                                        {
                                            insert -= 1;
                                        }
                                        let y = if top_half {
                                            response.rect.top()
                                        } else {
                                            response.rect.bottom()
                                        };
                                        let span = column.clip_rect();
                                        hover_drop = Some(DropTarget {
                                            section: *section,
                                            pos: insert,
                                            line_y: y,
                                            line_x: (span.left(), span.right()),
                                        });
                                    }
                                }
                                // Left-click toggles (egui suppresses click
                                // when the pointer moved — a drag never fires it).
                                if response.clicked() {
                                    toggle_index = Some(widget_idx);
                                }

                                // Right-click context menu (fallback paths).
                                response.context_menu(|menu| {
                                    let other_sections: Vec<(&str, StatusBarSection)> = SECTIONS
                                        .iter()
                                        .filter(|(_, s)| *s != *section)
                                        .map(|(label, s)| (*label, *s))
                                        .collect();
                                    for &(target_label, target_sec) in other_sections.iter() {
                                        if menu.button(format!("Move to {target_label}")).clicked()
                                        {
                                            menu_move = Some((widget_idx, target_sec, usize::MAX));
                                            menu.close();
                                        }
                                    }

                                    menu.separator();

                                    if pos > 0 && menu.button("Move Up").clicked() {
                                        menu_move = Some((widget_idx, *section, pos - 1));
                                        menu.close();
                                    }
                                    if pos + 1 < section_indices.len()
                                        && menu.button("Move Down").clicked()
                                    {
                                        menu_move = Some((widget_idx, *section, pos + 1));
                                        menu.close();
                                    }
                                });

                                // Order is set by drag and the menu, so only Delete.
                                if is_custom {
                                    let len = settings.config.status_bar.status_bar_widgets.len();
                                    column.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if let Some(crate::list_editor::RowAction::Delete(i)) =
                                                crate::list_editor::row_actions(
                                                    ui,
                                                    &mut settings.pending_list_delete,
                                                    crate::list_editor::Row {
                                                        index: widget_idx,
                                                        len,
                                                        list: "status_widget",
                                                        key: &widget_key,
                                                        delete_label: "Delete",
                                                    },
                                                    crate::list_editor::RowButtons {
                                                        reorder: false,
                                                        duplicate: false,
                                                        edit: false,
                                                        delete: true,
                                                    },
                                                )
                                            {
                                                delete_index = Some(i);
                                            }
                                        },
                                    );
                                }

                                // Show format editor for custom widgets inline
                                if is_custom
                                    && enabled
                                    && let Some(ref mut fmt) =
                                        settings.config.status_bar.status_bar_widgets[widget_idx]
                                            .format
                                {
                                    column.horizontal(|ui| {
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

                            // Empty column area appends: pointer over this
                            // column, no row captured this frame.
                            if dragging.is_some()
                                && hover_drop.is_none()
                                && column.rect_contains_pointer(column.clip_rect())
                            {
                                let span = column.clip_rect();
                                hover_drop = Some(DropTarget {
                                    section: *section,
                                    pos: section_indices.len(),
                                    line_y: span.bottom(),
                                    line_x: (span.left(), span.right()),
                                });
                            }
                        }
                    });

                    // Insertion line for the live candidate.
                    if let Some(target) = &hover_drop {
                        ui.painter().line_segment(
                            [
                                egui::pos2(target.line_x.0, target.line_y),
                                egui::pos2(target.line_x.1, target.line_y),
                            ],
                            egui::Stroke::new(2.0, INSERT_LINE_COLOR),
                        );
                    }

                    // Release applies the last candidate.
                    let released = ctx.input(|i| i.pointer.any_released());
                    if released && let Some(src) = dragging {
                        if let Some(target) = hover_drop {
                            reorder_widget(
                                &mut settings.config.status_bar.status_bar_widgets,
                                *src,
                                target.section,
                                target.pos,
                            );
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        egui::DragAndDrop::clear_payload(&ctx);
                    }

                    // Apply mutations
                    if let Some(idx) = toggle_index {
                        settings.config.status_bar.status_bar_widgets[idx].enabled =
                            !settings.config.status_bar.status_bar_widgets[idx].enabled;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }

                    if let Some((idx, section, pos)) = menu_move {
                        reorder_widget(
                            &mut settings.config.status_bar.status_bar_widgets,
                            idx,
                            section,
                            pos,
                        );
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
                },
            );
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use StatusBarSection::{Center, Left, Right};

    fn widget(section: StatusBarSection, order: i32) -> StatusBarWidgetConfig {
        StatusBarWidgetConfig {
            id: WidgetId::CurrentCommand,
            enabled: true,
            section,
            order,
            format: None,
        }
    }

    /// All orders dense 0..n within each section.
    fn assert_dense(widgets: &[StatusBarWidgetConfig]) {
        for (_, section) in SECTIONS {
            let mut orders: Vec<i32> = widgets
                .iter()
                .filter(|w| w.section == section)
                .map(|w| w.order)
                .collect();
            orders.sort();
            let expected: Vec<i32> = (0..orders.len() as i32).collect();
            assert_eq!(orders, expected, "{section:?} not dense");
        }
    }

    #[test]
    fn move_up_and_down_within_a_section() {
        let mut widgets = vec![widget(Left, 0), widget(Left, 1), widget(Left, 2)];
        reorder_widget(&mut widgets, 2, Left, 0);
        assert_dense(&widgets);
        let seq: Vec<i32> = widgets.iter().map(|w| w.order).collect();
        // Widget 2 now leads (order 0); w0 and w1 keep their relative order.
        assert_eq!(seq, [1, 2, 0]);

        reorder_widget(&mut widgets, 2, Left, 2); // back below widget 1
        let seq: Vec<i32> = widgets.iter().map(|w| w.order).collect();
        assert_eq!(seq, [0, 1, 2]);
    }

    #[test]
    fn move_across_sections_and_renumber_both() {
        let mut widgets = vec![
            widget(Left, 0),
            widget(Left, 1),
            widget(Right, 0),
            widget(Right, 1),
        ];
        // Move the first Left widget to the head of Right.
        reorder_widget(&mut widgets, 0, Right, 0);
        assert_dense(&widgets);
        assert_eq!(widgets[0].section, Right);
        assert_eq!(widgets[0].order, 0);
        assert_eq!(widgets[2].order, 1, "old Right head shifted down");
        assert_eq!(widgets[3].order, 2);
        assert_eq!(widgets[1].order, 0, "Left renumbered after the move out");
    }

    #[test]
    fn cross_section_move_to_pos_beyond_len_appends() {
        let mut widgets = vec![widget(Left, 0), widget(Right, 0)];
        reorder_widget(&mut widgets, 0, Right, 9);
        assert_dense(&widgets);
        assert_eq!(widgets[0].order, 1, "appended after the single Right row");
    }

    #[test]
    fn stray_sparse_orders_normalize_on_any_move() {
        let mut widgets = vec![widget(Center, 17), widget(Center, 4), widget(Right, 200)];
        reorder_widget(&mut widgets, 2, Right, 0); // no-op move, still renumbers
        assert_dense(&widgets);
        assert_eq!(widgets[1].order, 0, "order 4 sorts before 17");
        assert_eq!(widgets[0].order, 1);
    }

    #[test]
    fn custom_format_travels_with_the_widget() {
        let mut w = widget(Left, 0);
        w.format = Some("custom".to_string());
        let mut widgets = vec![w, widget(Right, 0)];
        reorder_widget(&mut widgets, 0, Center, 0);
        assert!(widgets[0].format.is_some(), "format must not be dropped");
        assert_eq!(widgets[0].section, StatusBarSection::Center);
    }

    /// The section renders its columns against a live egui context — the
    /// drag wiring (payload, insertion line, release) runs for real with no
    /// drag in flight.
    #[test]
    fn widgets_section_renders_without_panicking() {
        let ctx = egui::Context::default();
        let mut settings = crate::SettingsUI::new_for_tests(par_term_config::Config::default());
        settings.config.status_bar.status_bar_enabled = true;
        let mut collapsed = HashSet::new();
        let mut changes = false;
        for time in [0.0f64, 1.0] {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 700.0),
                )),
                time: Some(time),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                show_widgets_section(ui, &mut settings, &mut changes, &mut collapsed);
            });
            output.textures_delta.clear();
        }
        assert!(!changes, "a passive render must not mutate config");
    }
}

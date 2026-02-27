//! Drag-and-drop state and rendering for the tab bar.
//!
//! Contains [`TabBarUI`] methods for rendering drag feedback indicators,
//! the floating ghost tab that follows the cursor, and drop-target detection
//! for both horizontal and vertical tab bar layouts.

use crate::config::Config;
use crate::ui_constants::{TAB_CONTENT_PAD_X, TAB_DRAW_SHRINK_Y, TAB_DROP_DIAMOND_SIZE};

use super::TabBarAction;
use super::TabBarUI;
use super::title_utils::sanitize_egui_title_text;

impl TabBarUI {
    /// Render drag feedback indicator and handle drop/cancel for horizontal layout.
    pub(super) fn render_drag_feedback(
        &mut self,
        ui: &mut egui::Ui,
        config: &Config,
    ) -> TabBarAction {
        let mut action = TabBarAction::None;

        let dragging_id = match self.dragging_tab {
            Some(id) => id,
            None => {
                self.drag_in_progress = false;
                return action;
            }
        };

        // Cancel on Escape
        if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            self.drag_in_progress = false;
            self.dragging_tab = None;
            self.drop_target_index = None;
            return action;
        }

        // Set grabbing cursor
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);

        // Find the current index of the dragged tab
        let drag_source_index = self.tab_rects.iter().position(|(id, _)| *id == dragging_id);

        // Calculate insertion index from pointer position
        if let Some(pointer_pos) = ui.ctx().input(|i| i.pointer.hover_pos()) {
            let mut insert_index = self.tab_rects.len(); // default: after last tab
            for (i, (_id, rect)) in self.tab_rects.iter().enumerate() {
                if pointer_pos.x < rect.center().x {
                    insert_index = i;
                    break;
                }
            }

            // Determine if this would be a no-op (dropping in same position)
            let is_noop =
                drag_source_index.is_some_and(|src| insert_index == src || insert_index == src + 1);

            if is_noop {
                self.drop_target_index = None;
            } else {
                self.drop_target_index = Some(insert_index);

                // Draw vertical indicator line with glow at insertion point
                let indicator_x = if insert_index < self.tab_rects.len() {
                    self.tab_rects[insert_index].1.left() - 2.0
                } else if let Some(last) = self.tab_rects.last() {
                    last.1.right() + 2.0
                } else {
                    0.0
                };

                let indicator_color = egui::Color32::from_rgb(80, 160, 255);
                let glow_color = egui::Color32::from_rgba_unmultiplied(80, 160, 255, 50);
                let top = config.tab_bar_height * 0.1;
                let bottom = config.tab_bar_height * 0.9;

                // Glow behind the indicator (wider, semi-transparent)
                ui.painter().rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(indicator_x - 4.0, top),
                        egui::pos2(indicator_x + 4.0, bottom),
                    ),
                    2.0,
                    glow_color,
                );

                // Main indicator line
                ui.painter().line_segment(
                    [
                        egui::pos2(indicator_x, top),
                        egui::pos2(indicator_x, bottom),
                    ],
                    egui::Stroke::new(3.0, indicator_color),
                );

                // Small diamond/arrow at top and bottom of indicator
                let diamond_size = TAB_DROP_DIAMOND_SIZE;
                for y in [top, bottom] {
                    ui.painter().circle_filled(
                        egui::pos2(indicator_x, y),
                        diamond_size,
                        indicator_color,
                    );
                }
            }
        }

        // Handle drop (pointer released)
        if ui.ctx().input(|i| i.pointer.any_released()) {
            if let Some(insert_idx) = self.drop_target_index {
                // Convert insertion index to target index accounting for removal
                let effective_target = if let Some(src) = drag_source_index {
                    if insert_idx > src {
                        insert_idx - 1
                    } else {
                        insert_idx
                    }
                } else {
                    insert_idx
                };
                action = TabBarAction::Reorder(dragging_id, effective_target);
            }
            self.drag_in_progress = false;
            self.dragging_tab = None;
            self.drop_target_index = None;
        }

        action
    }

    /// Render drag feedback for vertical tab bar layout.
    pub(super) fn render_vertical_drag_feedback(
        &mut self,
        ui: &mut egui::Ui,
        config: &Config,
    ) -> TabBarAction {
        let mut action = TabBarAction::None;

        let dragging_id = match self.dragging_tab {
            Some(id) => id,
            None => {
                self.drag_in_progress = false;
                return action;
            }
        };

        if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            self.drag_in_progress = false;
            self.dragging_tab = None;
            self.drop_target_index = None;
            return action;
        }

        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);

        let drag_source_index = self.tab_rects.iter().position(|(id, _)| *id == dragging_id);

        if let Some(pointer_pos) = ui.ctx().input(|i| i.pointer.hover_pos()) {
            let mut insert_index = self.tab_rects.len();
            for (i, (_id, rect)) in self.tab_rects.iter().enumerate() {
                if pointer_pos.y < rect.center().y {
                    insert_index = i;
                    break;
                }
            }

            let is_noop =
                drag_source_index.is_some_and(|src| insert_index == src || insert_index == src + 1);

            if is_noop {
                self.drop_target_index = None;
            } else {
                self.drop_target_index = Some(insert_index);

                // Horizontal indicator line for vertical layout
                let indicator_y = if insert_index < self.tab_rects.len() {
                    self.tab_rects[insert_index].1.top() - 2.0
                } else if let Some(last) = self.tab_rects.last() {
                    last.1.bottom() + 2.0
                } else {
                    0.0
                };

                let indicator_color = egui::Color32::from_rgb(80, 160, 255);
                let left = config.tab_bar_width * 0.05;
                let right = config.tab_bar_width * 0.95;

                ui.painter().line_segment(
                    [
                        egui::pos2(left, indicator_y),
                        egui::pos2(right, indicator_y),
                    ],
                    egui::Stroke::new(3.0, indicator_color),
                );
            }
        }

        if ui.ctx().input(|i| i.pointer.any_released()) {
            if let Some(insert_idx) = self.drop_target_index {
                let effective_target = if let Some(src) = drag_source_index {
                    if insert_idx > src {
                        insert_idx - 1
                    } else {
                        insert_idx
                    }
                } else {
                    insert_idx
                };
                action = TabBarAction::Reorder(dragging_id, effective_target);
            }
            self.drag_in_progress = false;
            self.dragging_tab = None;
            self.drop_target_index = None;
        }

        action
    }

    /// Render a floating ghost tab that follows the cursor during drag.
    pub(super) fn render_ghost_tab(&self, ctx: &egui::Context, config: &Config) {
        let Some(pointer_pos) = ctx.input(|i| i.pointer.hover_pos()) else {
            return;
        };

        let ghost_width = self.dragging_tab_width;
        let ghost_height = config.tab_bar_height - TAB_DRAW_SHRINK_Y * 2.0;

        // Center ghost on pointer horizontally, offset slightly below vertically
        let ghost_pos = egui::pos2(
            pointer_pos.x - ghost_width / 2.0,
            pointer_pos.y - ghost_height / 2.0,
        );

        // Determine ghost background color
        let bg_color = if let Some(custom) = self.dragging_color {
            egui::Color32::from_rgba_unmultiplied(custom[0], custom[1], custom[2], 200)
        } else {
            let c = config.tab_active_background;
            egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 200)
        };

        let border_color = egui::Color32::from_rgba_unmultiplied(120, 180, 255, 200);

        egui::Area::new(egui::Id::new("tab_drag_ghost"))
            .fixed_pos(ghost_pos)
            .order(egui::Order::Tooltip)
            .interactable(false)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ghost_width, ghost_height),
                    egui::Sense::hover(),
                );

                let rounding = ghost_height / 2.0;

                // Shadow
                let shadow_rect = rect.translate(egui::vec2(2.0, 2.0));
                ui.painter().rect_filled(
                    shadow_rect,
                    rounding,
                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 80),
                );

                // Background
                ui.painter().rect_filled(rect, rounding, bg_color);

                // Border
                ui.painter().rect_stroke(
                    rect,
                    rounding,
                    egui::Stroke::new(1.5, border_color),
                    egui::StrokeKind::Middle,
                );

                // Title text (truncated to fit)
                let text_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220);
                let font_id = egui::FontId::proportional(13.0);
                let max_text_width = ghost_width - TAB_CONTENT_PAD_X * 2.0;
                let safe_drag_title = sanitize_egui_title_text(&self.dragging_title);
                let galley = ui.painter().layout(
                    safe_drag_title.into_owned(),
                    font_id,
                    text_color,
                    max_text_width,
                );
                let text_pos = egui::pos2(
                    rect.left() + TAB_CONTENT_PAD_X,
                    rect.center().y - galley.size().y / 2.0,
                );
                ui.painter().galley(text_pos, galley, text_color);
            });
    }
}

//! Vertical tab rendering and shared tab painting utilities.
//!
//! Contains:
//! - [`TabRenderParams`] — parameters struct shared by both layout renderers
//! - [`TabBarUI::compute_tab_bg_color`] — shared background-color computation
//! - [`TabBarUI::render_vertical_tab`] — per-tab drawing for the vertical (left-side) layout
//!
//! Horizontal per-tab drawing ([`TabBarUI::render_tab_with_width`]) lives in the
//! sibling `tab_painter` module.

use crate::config::Config;
use crate::tab::TabId;
use crate::ui_constants::{
    TAB_ACTIVE_INDICATOR_WIDTH, TAB_CLOSE_BTN_MARGIN, TAB_CLOSE_BTN_SIZE_H, TAB_CONTENT_PAD_X,
    TAB_CONTENT_PAD_Y, TAB_DRAW_SHRINK_X, TAB_DRAW_SHRINK_Y, TAB_ROUNDING,
};

use super::TabBarAction;
use super::TabBarUI;
use super::title_utils::{estimate_max_chars, sanitize_egui_title_text, truncate_plain};

/// Parameters shared by both `render_vertical_tab` and `render_tab_with_width`.
///
/// The `tab_size` field carries the tab height (vertical bar) or tab width (horizontal bar).
pub(super) struct TabRenderParams<'a> {
    pub id: TabId,
    pub index: usize,
    pub title: &'a str,
    pub profile_icon: Option<&'a str>,
    pub custom_icon: Option<&'a str>,
    pub is_active: bool,
    pub has_activity: bool,
    pub is_bell_active: bool,
    pub custom_color: Option<[u8; 3]>,
    pub config: &'a Config,
    /// Tab height (vertical layout) or tab width (horizontal layout).
    pub tab_size: f32,
    pub tab_count: usize,
}

impl TabBarUI {
    /// Compute the tab background color based on active/hover/drag state and custom color.
    ///
    /// Shared by both `render_tab_with_width` (horizontal) and `render_vertical_tab` (vertical)
    /// to eliminate duplicated ~35-line bg-color blocks.
    pub(super) fn compute_tab_bg_color(
        &self,
        id: TabId,
        is_active: bool,
        custom_color: Option<[u8; 3]>,
        config: &Config,
    ) -> (egui::Color32, u8) {
        let is_hovered = self.hovered_tab == Some(id);
        let is_being_dragged = self.dragging_tab == Some(id) && self.drag_in_progress;
        let should_dim =
            is_being_dragged || (config.dim_inactive_tabs && !is_active && !is_hovered);
        let opacity: u8 = if is_being_dragged {
            100
        } else if should_dim {
            (config.inactive_tab_opacity * 255.0) as u8
        } else {
            255
        };

        // Whether this inactive tab should render as outline-only (no fill)
        let outline_only = config.tab_inactive_outline_only && !is_active;

        let bg_color = if outline_only {
            egui::Color32::TRANSPARENT
        } else if let Some(custom) = custom_color {
            if is_active {
                egui::Color32::from_rgba_unmultiplied(custom[0], custom[1], custom[2], 255)
            } else if is_hovered {
                let lighten = |c: u8| c.saturating_add(20);
                egui::Color32::from_rgba_unmultiplied(
                    lighten(custom[0]),
                    lighten(custom[1]),
                    lighten(custom[2]),
                    255,
                )
            } else {
                let darken = |c: u8| c.saturating_sub(30);
                egui::Color32::from_rgba_unmultiplied(
                    darken(custom[0]),
                    darken(custom[1]),
                    darken(custom[2]),
                    opacity,
                )
            }
        } else if is_active {
            let c = config.tab_active_background;
            egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 255)
        } else if is_hovered {
            let c = config.tab_hover_background;
            egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 255)
        } else {
            let c = config.tab_inactive_background;
            egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], opacity)
        };

        (bg_color, opacity)
    }

    /// Render a single tab as a full-width row in the vertical tab bar.
    pub(super) fn render_vertical_tab(
        &mut self,
        ui: &mut egui::Ui,
        p: TabRenderParams<'_>,
    ) -> (TabBarAction, egui::Rect) {
        let TabRenderParams {
            id,
            index: _index,
            title,
            profile_icon,
            custom_icon,
            is_active,
            has_activity,
            is_bell_active,
            custom_color,
            config,
            tab_size: tab_height,
            tab_count,
        } = p;
        let mut action = TabBarAction::None;

        let is_hovered = self.hovered_tab == Some(id);

        let (bg_color, opacity) = self.compute_tab_bg_color(id, is_active, custom_color, config);

        // Whether this inactive tab should render as outline-only (no fill)
        let outline_only = config.tab_inactive_outline_only && !is_active;

        let full_width = ui.available_width();
        let (tab_rect, _) =
            ui.allocate_exact_size(egui::vec2(full_width, tab_height), egui::Sense::hover());

        let tab_draw_rect = tab_rect.shrink2(egui::vec2(TAB_DRAW_SHRINK_X, TAB_DRAW_SHRINK_Y));
        let tab_rounding = TAB_ROUNDING;
        if ui.is_rect_visible(tab_rect) {
            ui.painter()
                .rect_filled(tab_draw_rect, tab_rounding, bg_color);

            // Draw outline border for outline-only inactive tabs (brighten on hover)
            if outline_only {
                let base = if let Some(custom) = custom_color {
                    custom
                } else {
                    config.tab_border_color
                };
                let c = if is_hovered {
                    let brighten = |v: u8| v.saturating_add(60);
                    [brighten(base[0]), brighten(base[1]), brighten(base[2])]
                } else {
                    base
                };
                let border_width = config.tab_border_width.max(1.0);
                ui.painter().rect_stroke(
                    tab_draw_rect,
                    tab_rounding,
                    egui::Stroke::new(border_width, egui::Color32::from_rgb(c[0], c[1], c[2])),
                    egui::StrokeKind::Inside,
                );
            }

            // Active indicator: left edge bar instead of bottom underline
            if is_active {
                let c = if let Some(custom) = custom_color {
                    let lighten = |v: u8| v.saturating_add(50);
                    [lighten(custom[0]), lighten(custom[1]), lighten(custom[2])]
                } else {
                    config.tab_active_indicator
                };
                let indicator_rect = egui::Rect::from_min_size(
                    tab_draw_rect.left_top(),
                    egui::vec2(TAB_ACTIVE_INDICATOR_WIDTH, tab_draw_rect.height()),
                );
                ui.painter().rect_filled(
                    indicator_rect,
                    egui::CornerRadius {
                        nw: tab_rounding as u8,
                        sw: tab_rounding as u8,
                        ne: 0,
                        se: 0,
                    },
                    egui::Color32::from_rgb(c[0], c[1], c[2]),
                );
            }

            // Content
            let content_rect = tab_rect.shrink2(egui::vec2(TAB_CONTENT_PAD_X, TAB_CONTENT_PAD_Y));
            let mut content_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(content_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );

            content_ui.horizontal(|ui| {
                if is_bell_active {
                    let c = config.tab_bell_indicator;
                    ui.colored_label(egui::Color32::from_rgb(c[0], c[1], c[2]), "🔔");
                    ui.add_space(2.0);
                } else if has_activity && !is_active {
                    let c = config.tab_activity_indicator;
                    ui.colored_label(egui::Color32::from_rgb(c[0], c[1], c[2]), "•");
                    ui.add_space(2.0);
                }

                // Profile icon
                let icon_width = if let Some(icon) = profile_icon {
                    let icon = sanitize_egui_title_text(icon);
                    ui.label(icon.as_ref());
                    ui.add_space(2.0);
                    18.0
                } else {
                    0.0
                };

                let text_color = if is_active {
                    let c = config.tab_active_text;
                    egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 255)
                } else {
                    let c = config.tab_inactive_text;
                    egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], opacity)
                };

                // Truncate title to fit available width
                let close_width = if config.tab_show_close_button {
                    TAB_CLOSE_BTN_SIZE_H + TAB_CLOSE_BTN_MARGIN
                } else {
                    0.0
                };
                let available = (full_width - TAB_CONTENT_PAD_X * 2.0 - icon_width - close_width)
                    .max(TAB_CONTENT_PAD_X * 2.0 + TAB_CLOSE_BTN_MARGIN);
                let base_font_id = ui.style().text_styles[&egui::TextStyle::Button].clone();
                let max_chars = estimate_max_chars(ui, &base_font_id, available);
                let safe_title = sanitize_egui_title_text(title);
                let display_title = truncate_plain(safe_title.as_ref(), max_chars);
                ui.label(egui::RichText::new(display_title).color(text_color));
            });

            // Close button at right edge
            if config.tab_show_close_button {
                let close_size = TAB_CLOSE_BTN_SIZE_H;
                let close_rect = egui::Rect::from_min_size(
                    egui::pos2(
                        tab_rect.right() - close_size - TAB_CLOSE_BTN_MARGIN,
                        tab_rect.center().y - close_size / 2.0,
                    ),
                    egui::vec2(close_size, close_size),
                );
                let pointer_pos = ui.ctx().input(|i| i.pointer.hover_pos());
                let close_hovered = pointer_pos.is_some_and(|pos| close_rect.contains(pos));

                if close_hovered {
                    self.close_hovered = Some(id);
                } else if self.close_hovered == Some(id) {
                    self.close_hovered = None;
                }

                let close_color = if self.close_hovered == Some(id) {
                    let c = config.tab_close_button_hover;
                    egui::Color32::from_rgb(c[0], c[1], c[2])
                } else {
                    let c = config.tab_close_button;
                    egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], opacity)
                };

                ui.painter().text(
                    close_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "×",
                    egui::FontId::proportional(12.0),
                    close_color,
                );
            }
        }

        // Handle click and drag
        let tab_response = ui.interact(
            tab_rect,
            egui::Id::new(("tab_click", id)),
            egui::Sense::click_and_drag(),
        );

        let pointer_in_tab = tab_response.hovered();
        let clicked = tab_response.clicked_by(egui::PointerButton::Primary);

        if tab_count > 1
            && !self.drag_in_progress
            && self.close_hovered != Some(id)
            && tab_response.drag_started_by(egui::PointerButton::Primary)
        {
            self.drag_in_progress = true;
            self.dragging_tab = Some(id);
            self.dragging_title = title.to_string();
            self.dragging_color = custom_color;
            self.dragging_tab_width = full_width;
        }

        let is_dragging_this = self.dragging_tab == Some(id) && self.drag_in_progress;

        if clicked
            && !is_dragging_this
            && action == TabBarAction::None
            && self.close_hovered != Some(id)
        {
            action = TabBarAction::SwitchTo(id);
        }

        if clicked && self.close_hovered == Some(id) {
            action = TabBarAction::Close(id);
        }

        if tab_response.secondary_clicked() {
            self.editing_color = custom_color.unwrap_or([100, 100, 100]);
            self.context_menu_tab = Some(id);
            self.context_menu_title = title.to_string();
            self.context_menu_icon = custom_icon.map(|s| s.to_string());
            self.icon_buffer = custom_icon.unwrap_or("").to_string();
            self.picking_icon = false;
            if let Some(pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                self.context_menu_pos = pos;
            }
            self.context_menu_opened_frame = ui.ctx().cumulative_frame_nr();
        }

        if pointer_in_tab {
            self.hovered_tab = Some(id);
        } else if self.hovered_tab == Some(id) {
            self.hovered_tab = None;
        }

        (action, tab_rect)
    }
}

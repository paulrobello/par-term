//! Horizontal tab bar layout rendering.
//!
//! Contains the [`TabBarUI`] `render_horizontal` method and its helpers.

use crate::config::{Config, TabBarPosition};
use crate::tab::TabManager;
use crate::ui_constants::{
    TAB_DRAW_SHRINK_Y, TAB_LEFT_PADDING, TAB_NEW_BTN_BASE_WIDTH, TAB_SCROLL_BTN_WIDTH, TAB_SPACING,
};

use super::CHEVRON_RESERVED;
use super::TabBarAction;
use super::state::TabBarUI;

impl TabBarUI {
    /// Render the tab bar in horizontal layout (top or bottom)
    pub(super) fn render_horizontal(
        &mut self,
        ctx: &egui::Context,
        tabs: &TabManager,
        config: &Config,
        profiles: &crate::profile::ProfileManager,
        right_reserved_width: f32,
    ) -> TabBarAction {
        let tab_count = tabs.tab_count();

        // Clear per-frame tab rect cache
        self.tab_rects.clear();

        let mut action = TabBarAction::None;
        let active_tab_id = tabs.active_tab_id();

        // Layout constants
        let tab_spacing = TAB_SPACING;
        let left_padding = TAB_LEFT_PADDING;
        // Show the chevron dropdown when there's menu content:
        // profiles to pick from, or the AI assistant toggle.
        let show_chevron = !profiles.is_empty() || config.ai_inspector_enabled;
        let new_tab_btn_width =
            TAB_NEW_BTN_BASE_WIDTH + if show_chevron { CHEVRON_RESERVED } else { 0.0 };
        let scroll_btn_width = TAB_SCROLL_BTN_WIDTH;

        let bar_bg = config.tab_bar_background;
        let frame =
            egui::Frame::NONE.fill(egui::Color32::from_rgb(bar_bg[0], bar_bg[1], bar_bg[2]));

        let panel = if config.tab_bar_position == TabBarPosition::Bottom {
            egui::TopBottomPanel::bottom("tab_bar").exact_height(config.tab_bar_height)
        } else {
            egui::TopBottomPanel::top("tab_bar").exact_height(config.tab_bar_height)
        };

        panel.frame(frame).show(ctx, |ui| {
            // Reserve space on the right for overlay panels (e.g. AI inspector Area)
            // so tabs/buttons don't render underneath them.
            let total_bar_width = (ui.available_width() - right_reserved_width.max(0.0)).max(0.0);

            // Calculate minimum total width needed for all tabs at min_width
            let min_total_tabs_width = if tab_count > 0 {
                tab_count as f32 * config.tab_min_width + (tab_count - 1) as f32 * tab_spacing
            } else {
                0.0
            };

            // Available width for tabs (without scroll buttons initially).
            // Budget: left_padding + tabs + tab_spacing (cursor gap) + new_tab_btn_width = total
            let base_tabs_area_width =
                (total_bar_width - new_tab_btn_width - tab_spacing - left_padding).max(0.0);

            // Determine if scrolling is needed
            let needs_scroll = tab_count > 0 && min_total_tabs_width > base_tabs_area_width;

            // Actual tabs area width (accounting for scroll buttons if needed)
            let tabs_area_width = if needs_scroll {
                (base_tabs_area_width - 2.0 * scroll_btn_width - 2.0 * tab_spacing).max(0.0)
            } else {
                base_tabs_area_width
            };

            // Calculate tab width
            let tab_width = if tab_count == 0 || needs_scroll {
                config.tab_min_width
            } else if config.tab_stretch_to_fill {
                let total_spacing = (tab_count - 1) as f32 * tab_spacing;
                let stretched = (tabs_area_width - total_spacing) / tab_count as f32;
                stretched.max(config.tab_min_width)
            } else {
                config.tab_min_width
            };

            // Calculate max scroll offset
            let max_scroll = if needs_scroll {
                (min_total_tabs_width - tabs_area_width).max(0.0)
            } else {
                0.0
            };

            // Clamp scroll offset
            self.scroll_offset = self.scroll_offset.clamp(0.0, max_scroll);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(tab_spacing, 0.0);
                // Small left padding so the first tab's border isn't clipped by the panel edge
                ui.add_space(left_padding);

                if needs_scroll {
                    // Left scroll button
                    let can_scroll_left = self.scroll_offset > 0.0;
                    let left_btn = ui.add_enabled(
                        can_scroll_left,
                        egui::Button::new("◀")
                            .min_size(egui::vec2(
                                scroll_btn_width,
                                config.tab_bar_height - TAB_DRAW_SHRINK_Y * 2.0,
                            ))
                            .fill(egui::Color32::TRANSPARENT),
                    );
                    if left_btn.clicked() {
                        self.scroll_offset =
                            (self.scroll_offset - tab_width - tab_spacing).max(0.0);
                    }

                    // Scrollable tab area
                    let scroll_area_response = egui::ScrollArea::horizontal()
                        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                        .max_width(tabs_area_width)
                        .horizontal_scroll_offset(self.scroll_offset)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(tab_spacing, 0.0);

                                for (index, tab) in tabs.tabs().iter().enumerate() {
                                    let is_active = Some(tab.id) == active_tab_id;
                                    let is_bell_active = tab.is_bell_active();
                                    let (tab_action, tab_rect) = self.render_tab_with_width(
                                        ui,
                                        tab.id,
                                        index,
                                        &tab.title,
                                        tab.custom_icon
                                            .as_deref()
                                            .or(tab.profile.profile_icon.as_deref()),
                                        tab.custom_icon.as_deref(),
                                        is_active,
                                        tab.has_activity,
                                        is_bell_active,
                                        tab.custom_color,
                                        config,
                                        tab_width,
                                        tab_count,
                                    );
                                    self.tab_rects.push((tab.id, tab_rect));

                                    if tab_action != TabBarAction::None {
                                        action = tab_action;
                                    }
                                }
                            });
                        });

                    // Update scroll offset from scroll area
                    self.scroll_offset = scroll_area_response.state.offset.x;

                    // Right scroll button
                    let can_scroll_right = self.scroll_offset < max_scroll;
                    let right_btn = ui.add_enabled(
                        can_scroll_right,
                        egui::Button::new("▶")
                            .min_size(egui::vec2(
                                scroll_btn_width,
                                config.tab_bar_height - TAB_DRAW_SHRINK_Y * 2.0,
                            ))
                            .fill(egui::Color32::TRANSPARENT),
                    );
                    if right_btn.clicked() {
                        self.scroll_offset =
                            (self.scroll_offset + tab_width + tab_spacing).min(max_scroll);
                    }
                } else {
                    // No scrolling needed - render all tabs with equal width
                    for (index, tab) in tabs.tabs().iter().enumerate() {
                        let is_active = Some(tab.id) == active_tab_id;
                        let is_bell_active = tab.is_bell_active();
                        let (tab_action, tab_rect) = self.render_tab_with_width(
                            ui,
                            tab.id,
                            index,
                            &tab.title,
                            tab.custom_icon
                                .as_deref()
                                .or(tab.profile.profile_icon.as_deref()),
                            tab.custom_icon.as_deref(),
                            is_active,
                            tab.has_activity,
                            is_bell_active,
                            tab.custom_color,
                            config,
                            tab_width,
                            tab_count,
                        );
                        self.tab_rects.push((tab.id, tab_rect));

                        if tab_action != TabBarAction::None {
                            action = tab_action;
                        }
                    }
                }

                // New tab split button: [+][▾]
                // The 4px gap from the last widget's cursor advance provides the
                // natural spacing between tabs and the button.

                // Use zero spacing between + and ▾ so they render as one split button
                let prev_spacing = ui.spacing().item_spacing.x;
                ui.spacing_mut().item_spacing.x = 0.0;

                // "+" button — creates default tab
                let plus_btn = ui.add(
                    egui::Button::new("+")
                        .min_size(egui::vec2(
                            TAB_NEW_BTN_BASE_WIDTH,
                            config.tab_bar_height - TAB_DRAW_SHRINK_Y * 2.0,
                        ))
                        .fill(egui::Color32::TRANSPARENT),
                );
                if plus_btn.clicked_by(egui::PointerButton::Primary) {
                    action = TabBarAction::NewTab;
                }
                if plus_btn.hovered() {
                    #[cfg(target_os = "macos")]
                    plus_btn.on_hover_text("New Tab (Cmd+T)");
                    #[cfg(not(target_os = "macos"))]
                    plus_btn.on_hover_text("New Tab (Ctrl+Shift+T)");
                }

                // "▾" chevron — opens dropdown (profiles and/or assistant toggle)
                if show_chevron {
                    let chevron_btn = ui.add(
                        egui::Button::new("⏷")
                            .min_size(egui::vec2(
                                CHEVRON_RESERVED / 2.0,
                                config.tab_bar_height - TAB_DRAW_SHRINK_Y * 2.0,
                            ))
                            .fill(egui::Color32::TRANSPARENT),
                    );
                    if chevron_btn.clicked_by(egui::PointerButton::Primary) {
                        self.show_new_tab_profile_menu = !self.show_new_tab_profile_menu;
                    }
                    if chevron_btn.hovered() {
                        chevron_btn.on_hover_text("New tab from profile");
                    }
                }

                // Restore original spacing
                ui.spacing_mut().item_spacing.x = prev_spacing;
            });

            // Handle drag feedback and drop detection (outside horizontal layout
            // so we can paint over the tab bar)
            if self.drag_in_progress {
                let drag_action = self.render_drag_feedback(ui, config);
                if drag_action != TabBarAction::None {
                    action = drag_action;
                }
            }
        });

        // Render floating ghost tab during drag (must be outside the panel)
        if self.drag_in_progress && self.dragging_tab.is_some() {
            self.render_ghost_tab(ctx, config);
        }

        // Handle context menu (color picker popup)
        if let Some(context_tab_id) = self.context_menu_tab {
            let menu_action = self.render_context_menu(ctx, context_tab_id);
            if menu_action != TabBarAction::None {
                action = menu_action;
            }
        }

        // Render new-tab profile menu if open
        let menu_action = self.render_new_tab_profile_menu(ctx, profiles, config);
        if menu_action != TabBarAction::None {
            action = menu_action;
        }

        action
    }
}

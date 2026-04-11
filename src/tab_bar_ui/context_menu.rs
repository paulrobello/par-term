//! Context menu for tab options (rename, color, icon, duplicate, close).
//!
//! Contains the [`TabBarUI`] method for rendering the right-click context menu
//! that appears when a tab is right-clicked in either horizontal or vertical layout.

use crate::tab::TabId;
use crate::ui_constants::{
    TAB_COLOR_HEX_EDIT_WIDTH, TAB_COLOR_SWATCH_ROUNDING, TAB_COLOR_SWATCH_SIZE,
    TAB_CONTEXT_MENU_ITEM_HEIGHT, TAB_CONTEXT_MENU_MIN_WIDTH, TAB_ICON_PICKER_GLYPH_SIZE,
    TAB_ICON_PICKER_MAX_HEIGHT, TAB_ICON_PICKER_MIN_WIDTH, TAB_RENAME_EDIT_WIDTH,
};

use super::TabBarAction;
use super::TabBarUI;

impl TabBarUI {
    /// Render the context menu for tab options.
    pub(super) fn render_context_menu(
        &mut self,
        ctx: &egui::Context,
        tab_id: TabId,
    ) -> TabBarAction {
        let mut action = TabBarAction::None;
        let mut close_menu = false;

        // Handle Escape: cancel rename/icon picker if active, otherwise close menu
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.renaming_tab {
                self.renaming_tab = false;
            } else if self.picking_icon {
                self.picking_icon = false;
            } else {
                close_menu = true;
            }
        }

        let area_response = egui::Area::new(egui::Id::new("tab_context_menu"))
            .fixed_pos(self.context_menu_pos)
            .constrain(true)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::symmetric(1, 4))
                    .show(ui, |ui| {
                        ui.set_min_width(TAB_CONTEXT_MENU_MIN_WIDTH);
                        ui.style_mut().spacing.item_spacing = egui::vec2(0.0, 0.0);

                        // Menu item helper
                        let menu_item = |ui: &mut egui::Ui, label: &str| -> bool {
                            let response = ui.add_sized(
                                [ui.available_width(), TAB_CONTEXT_MENU_ITEM_HEIGHT],
                                egui::Button::new(label)
                                    .frame(false)
                                    .fill(egui::Color32::TRANSPARENT),
                            );
                            response.clicked()
                        };

                        // Rename Tab
                        if self.renaming_tab {
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                let response = ui.add(
                                    egui::TextEdit::singleline(&mut self.rename_buffer)
                                        .desired_width(TAB_RENAME_EDIT_WIDTH)
                                        .hint_text("Tab name"),
                                );
                                // Auto-focus when first shown
                                if !response.has_focus() {
                                    response.request_focus();
                                }
                                // Submit on Enter
                                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    let name = self.rename_buffer.trim().to_string();
                                    action = TabBarAction::RenameTab(tab_id, name);
                                    self.renaming_tab = false;
                                    close_menu = true;
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("Leave blank to use auto title")
                                        .weak()
                                        .small(),
                                );
                            });
                            ui.add_space(2.0);
                        } else if menu_item(ui, "Rename Tab") {
                            self.renaming_tab = true;
                            self.rename_activated_frame = ui.ctx().cumulative_frame_nr();
                            self.rename_buffer = self.context_menu_title.clone();
                        }

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // Tab Icon section
                        if self.picking_icon {
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                ui.label("Icon:");
                                let response = ui.add(
                                    egui::TextEdit::singleline(&mut self.icon_buffer)
                                        .desired_width(TAB_COLOR_HEX_EDIT_WIDTH)
                                        .hint_text("Icon"),
                                );
                                if !response.has_focus() {
                                    response.request_focus();
                                }
                                // Nerd Font picker button
                                let picker_label = if self.icon_buffer.is_empty() {
                                    "\u{ea7b}"
                                } else {
                                    &self.icon_buffer
                                };
                                let picker_btn =
                                    ui.button(picker_label).on_hover_text("Icon picker");
                                egui::Popup::from_toggle_button_response(&picker_btn)
                                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                                    .show(|ui| {
                                        ui.set_min_width(TAB_ICON_PICKER_MIN_WIDTH);
                                        egui::ScrollArea::vertical()
                                            .max_height(TAB_ICON_PICKER_MAX_HEIGHT)
                                            .show(ui, |ui| {
                                                egui::Frame::NONE
                                                    .inner_margin(egui::Margin {
                                                        right: 10,
                                                        ..Default::default()
                                                    })
                                                    .show(ui, |ui| {
                                                        for (category, icons) in
                                                            crate::settings_ui::nerd_font::NERD_FONT_PRESETS
                                                        {
                                                            ui.label(
                                                                egui::RichText::new(*category)
                                                                    .small()
                                                                    .strong(),
                                                            );
                                                            ui.horizontal_wrapped(|ui| {
                                                                for (icon, label) in *icons {
                                                                    let btn = ui.add_sized(
                                                                        [
                                                                            TAB_ICON_PICKER_GLYPH_SIZE
                                                                                + 12.0,
                                                                            TAB_ICON_PICKER_GLYPH_SIZE
                                                                                + 12.0,
                                                                        ],
                                                                        egui::Button::new(
                                                                            egui::RichText::new(*icon)
                                                                                .size(
                                                                                TAB_ICON_PICKER_GLYPH_SIZE,
                                                                            ),
                                                                        )
                                                                        .frame(false),
                                                                    );
                                                                    if btn.on_hover_text(*label).clicked() {
                                                                        self.icon_buffer = icon.to_string();
                                                                        egui::Popup::close_all(ui.ctx());
                                                                    }
                                                                }
                                                            });
                                                            ui.add_space(2.0);
                                                        }
                                                        ui.add_space(4.0);
                                                        if ui.button("Clear icon").clicked() {
                                                            self.icon_buffer.clear();
                                                            egui::Popup::close_all(ui.ctx());
                                                        }
                                                    });
                                            });
                                    });
                            });
                            // Submit on Enter
                            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                                let icon = self.icon_buffer.trim().to_string();
                                action = TabBarAction::SetTabIcon(
                                    tab_id,
                                    if icon.is_empty() { None } else { Some(icon) },
                                );
                                self.picking_icon = false;
                                close_menu = true;
                            }
                        } else if menu_item(ui, "Set Icon") {
                            self.picking_icon = true;
                            self.icon_activated_frame = ui.ctx().cumulative_frame_nr();
                        }

                        // Clear Icon (only show when tab has a custom icon)
                        if self.context_menu_icon.is_some()
                            && !self.picking_icon
                            && menu_item(ui, "Clear Icon")
                        {
                            action = TabBarAction::SetTabIcon(tab_id, None);
                            close_menu = true;
                        }

                        // Duplicate Tab
                        if menu_item(ui, "Duplicate Tab") {
                            action = TabBarAction::Duplicate(tab_id);
                            close_menu = true;
                        }

                        // Close Tab
                        if menu_item(ui, "Close Tab") {
                            action = TabBarAction::Close(tab_id);
                            close_menu = true;
                        }

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // ----- Move Tab entries -----
                        let can_move = !self.move_gateway_active;
                        let has_other_windows = !self.move_candidates.is_empty();

                        // "Move Tab to New Window" — disabled for gateway-active windows
                        // and for solo-tab source windows (visually a no-op).
                        let new_window_enabled =
                            can_move && self.move_source_tab_count >= 2;
                        ui.add_enabled_ui(new_window_enabled, |ui| {
                            if menu_item(ui, "Move Tab to New Window") {
                                action = TabBarAction::MoveTabToNewWindow(tab_id);
                                close_menu = true;
                            }
                        });

                        // "Move Tab to Window →" submenu — hidden entirely if there
                        // are no other windows or the move is disabled by gateway.
                        if can_move && has_other_windows {
                            let candidates = self.move_candidates.clone();
                            ui.menu_button("Move Tab to Window ▸", |ui| {
                                for (win_id, label) in candidates {
                                    if ui.button(&label).clicked() {
                                        action = TabBarAction::MoveTabToExistingWindow(
                                            tab_id, win_id,
                                        );
                                        close_menu = true;
                                    }
                                }
                            });
                        }

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);
                        // ----- end Move Tab entries -----

                        // Tab Color section
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            ui.label("Tab Color:");
                        });

                        ui.add_space(4.0);

                        // Color presets row
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);

                            let presets: &[([u8; 3], &str)] = &[
                                ([165, 38, 38], "Red"),
                                ([165, 98, 38], "Orange"),
                                ([165, 135, 38], "Yellow"),
                                ([38, 135, 38], "Green"),
                                ([38, 135, 135], "Cyan"),
                                ([38, 75, 165], "Blue"),
                                ([135, 38, 135], "Purple"),
                            ];

                            for (color, name) in presets {
                                let btn = ui.add(
                                    egui::Button::new("")
                                        .fill(egui::Color32::from_rgb(color[0], color[1], color[2]))
                                        .min_size(egui::vec2(
                                            TAB_COLOR_SWATCH_SIZE,
                                            TAB_COLOR_SWATCH_SIZE,
                                        ))
                                        .corner_radius(TAB_COLOR_SWATCH_ROUNDING),
                                );
                                if btn.clicked() {
                                    action = TabBarAction::SetColor(tab_id, *color);
                                    close_menu = true;
                                }
                                if btn.hovered() {
                                    btn.on_hover_text(*name);
                                }
                            }

                            ui.add_space(4.0);

                            // Custom color picker
                            if ui.color_edit_button_srgb(&mut self.editing_color).changed() {
                                action = TabBarAction::SetColor(tab_id, self.editing_color);
                            }
                        });

                        ui.add_space(4.0);

                        // Clear color option
                        if menu_item(ui, "Clear Color") {
                            action = TabBarAction::ClearColor(tab_id);
                            close_menu = true;
                        }
                    });
            });

        // Close menu if clicked outside (but not on the same frame it was opened,
        // rename mode was activated, or icon picker mode was activated)
        let current_frame = ctx.cumulative_frame_nr();
        if current_frame > self.context_menu_opened_frame
            && current_frame > self.rename_activated_frame
            && current_frame > self.icon_activated_frame
            && ctx.input(|i| i.pointer.any_click())
            && !area_response.response.hovered()
            // Only close if no action was taken (let button clicks register)
            && !close_menu
            && action == TabBarAction::None
            // Don't close while icon picker is active — the popup opens on a
            // separate egui layer so clicks inside it appear "outside" this area
            && !self.picking_icon
        {
            // If renaming, submit the current buffer on click-away
            if self.renaming_tab {
                let name = self.rename_buffer.trim().to_string();
                action = TabBarAction::RenameTab(tab_id, name);
            }
            close_menu = true;
        }

        // Close menu if action taken or cancelled
        if close_menu {
            self.context_menu_tab = None;
            self.renaming_tab = false;
            self.picking_icon = false;
        }

        action
    }
}

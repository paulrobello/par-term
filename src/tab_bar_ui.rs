//! Tab bar UI using egui
//!
//! Provides a visual tab bar for switching between terminal tabs.

use crate::config::{Config, TabBarMode};
use crate::tab::{TabId, TabManager};

/// Actions that can be triggered from the tab bar
#[derive(Debug, Clone, PartialEq)]
pub enum TabBarAction {
    /// No action
    None,
    /// Switch to a specific tab
    SwitchTo(TabId),
    /// Close a specific tab
    Close(TabId),
    /// Create a new tab
    NewTab,
    /// Reorder a tab to a new position
    #[allow(dead_code)]
    Reorder(TabId, usize),
    /// Set custom color for a tab
    SetColor(TabId, [u8; 3]),
    /// Clear custom color for a tab (revert to default)
    ClearColor(TabId),
}

/// Tab bar UI state
pub struct TabBarUI {
    /// Currently hovered tab ID
    pub hovered_tab: Option<TabId>,
    /// Tab where close button is hovered
    pub close_hovered: Option<TabId>,
    /// Whether a drag is in progress
    #[allow(dead_code)]
    drag_in_progress: bool,
    /// Tab being dragged
    #[allow(dead_code)]
    dragging_tab: Option<TabId>,
    /// Tab ID for which context menu is open
    context_menu_tab: Option<TabId>,
    /// Position where context menu was opened
    context_menu_pos: egui::Pos2,
    /// Frame when context menu was opened (to avoid closing on same frame)
    context_menu_opened_frame: u64,
    /// Color being edited in the color picker (for the context menu)
    editing_color: [u8; 3],
    /// Horizontal scroll offset for tabs (in pixels)
    scroll_offset: f32,
}

impl TabBarUI {
    /// Create a new tab bar UI
    pub fn new() -> Self {
        Self {
            hovered_tab: None,
            close_hovered: None,
            drag_in_progress: false,
            dragging_tab: None,
            context_menu_tab: None,
            context_menu_pos: egui::Pos2::ZERO,
            context_menu_opened_frame: 0,
            editing_color: [100, 100, 100],
            scroll_offset: 0.0,
        }
    }

    /// Check if tab bar should be visible
    pub fn should_show(&self, tab_count: usize, mode: TabBarMode) -> bool {
        match mode {
            TabBarMode::Always => true,
            TabBarMode::WhenMultiple => tab_count > 1,
            TabBarMode::Never => false,
        }
    }

    /// Render the tab bar and return any action triggered
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        tabs: &TabManager,
        config: &Config,
    ) -> TabBarAction {
        let tab_count = tabs.tab_count();

        // Don't show if configured to hide
        if !self.should_show(tab_count, config.tab_bar_mode) {
            return TabBarAction::None;
        }

        let mut action = TabBarAction::None;
        let active_tab_id = tabs.active_tab_id();

        // Layout constants
        let tab_spacing = 4.0;
        let new_tab_btn_width = 28.0;
        let scroll_btn_width = 24.0;

        // Tab bar area at the top
        let bar_bg = config.tab_bar_background;
        egui::TopBottomPanel::top("tab_bar")
            .exact_height(config.tab_bar_height)
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(bar_bg[0], bar_bg[1], bar_bg[2])))
            .show(ctx, |ui| {
                let total_bar_width = ui.available_width();

                // Calculate minimum total width needed for all tabs at min_width
                let min_total_tabs_width = if tab_count > 0 {
                    tab_count as f32 * config.tab_min_width + (tab_count - 1) as f32 * tab_spacing
                } else {
                    0.0
                };

                // Available width for tabs (without scroll buttons initially)
                let base_tabs_area_width = total_bar_width - new_tab_btn_width - tab_spacing;

                // Determine if scrolling is needed
                let needs_scroll = tab_count > 0 && min_total_tabs_width > base_tabs_area_width;

                // Actual tabs area width (accounting for scroll buttons if needed)
                let tabs_area_width = if needs_scroll {
                    base_tabs_area_width - 2.0 * scroll_btn_width - 2.0 * tab_spacing
                } else {
                    base_tabs_area_width
                };

                // Calculate tab width
                let tab_width = if tab_count == 0 || needs_scroll {
                    // Use minimum width when scrolling or no tabs
                    config.tab_min_width
                } else {
                    // Equal distribution: divide available space among all tabs
                    let total_spacing = (tab_count - 1) as f32 * tab_spacing;
                    (tabs_area_width - total_spacing) / tab_count as f32
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

                    if needs_scroll {
                        // Left scroll button
                        let can_scroll_left = self.scroll_offset > 0.0;
                        let left_btn = ui.add_enabled(
                            can_scroll_left,
                            egui::Button::new("◀")
                                .min_size(egui::vec2(scroll_btn_width, config.tab_bar_height - 4.0))
                                .fill(egui::Color32::TRANSPARENT),
                        );
                        if left_btn.clicked() {
                            self.scroll_offset =
                                (self.scroll_offset - tab_width - tab_spacing).max(0.0);
                        }

                        // Scrollable tab area
                        let scroll_area_response = egui::ScrollArea::horizontal()
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                            )
                            .max_width(tabs_area_width)
                            .horizontal_scroll_offset(self.scroll_offset)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(tab_spacing, 0.0);

                                    for (index, tab) in tabs.tabs().iter().enumerate() {
                                        let is_active = Some(tab.id) == active_tab_id;
                                        let is_bell_active = tab.is_bell_active();
                                        let tab_action = self.render_tab_with_width(
                                            ui,
                                            tab.id,
                                            index,
                                            &tab.title,
                                            is_active,
                                            tab.has_activity,
                                            is_bell_active,
                                            tab.custom_color,
                                            config,
                                            tab_width,
                                        );

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
                                .min_size(egui::vec2(scroll_btn_width, config.tab_bar_height - 4.0))
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
                            let tab_action = self.render_tab_with_width(
                                ui,
                                tab.id,
                                index,
                                &tab.title,
                                is_active,
                                tab.has_activity,
                                is_bell_active,
                                tab.custom_color,
                                config,
                                tab_width,
                            );

                            if tab_action != TabBarAction::None {
                                action = tab_action;
                            }
                        }
                    }

                    // New tab button
                    ui.add_space(tab_spacing);
                    let new_tab_btn = ui.add(
                        egui::Button::new("+")
                            .min_size(egui::vec2(new_tab_btn_width, config.tab_bar_height - 4.0))
                            .fill(egui::Color32::TRANSPARENT),
                    );

                    if new_tab_btn.clicked() {
                        action = TabBarAction::NewTab;
                    }

                    if new_tab_btn.hovered() {
                        new_tab_btn.on_hover_text("New Tab (Cmd+T)");
                    }
                });
            });

        // Handle context menu (color picker popup)
        if let Some(context_tab_id) = self.context_menu_tab {
            let menu_action = self.render_context_menu(ctx, context_tab_id);
            if menu_action != TabBarAction::None {
                action = menu_action;
            }
        }

        action
    }

    /// Render a single tab with specified width and return any action triggered
    #[allow(clippy::too_many_arguments)]
    fn render_tab_with_width(
        &mut self,
        ui: &mut egui::Ui,
        id: TabId,
        index: usize,
        title: &str,
        is_active: bool,
        has_activity: bool,
        is_bell_active: bool,
        custom_color: Option<[u8; 3]>,
        config: &Config,
        tab_width: f32,
    ) -> TabBarAction {
        let mut action = TabBarAction::None;

        // Determine if this tab should be dimmed
        // Active tabs and hovered inactive tabs are NOT dimmed
        let is_hovered = self.hovered_tab == Some(id);
        let should_dim = config.dim_inactive_tabs && !is_active && !is_hovered;
        let opacity = if should_dim {
            (config.inactive_tab_opacity * 255.0) as u8
        } else {
            255
        };

        // Tab background color with opacity
        // Custom color overrides config colors for inactive/active background
        let bg_color = if let Some(custom) = custom_color {
            // Use custom color with appropriate opacity/brightness adjustment
            if is_active {
                egui::Color32::from_rgba_unmultiplied(custom[0], custom[1], custom[2], 255)
            } else if is_hovered {
                // Lighten the custom color slightly for hover
                let lighten = |c: u8| c.saturating_add(20);
                egui::Color32::from_rgba_unmultiplied(
                    lighten(custom[0]),
                    lighten(custom[1]),
                    lighten(custom[2]),
                    255,
                )
            } else {
                // Darken the custom color slightly for inactive
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

        // Tab frame - use allocate_ui_with_layout to get a proper interactive response
        let (tab_rect, tab_response) = ui.allocate_exact_size(
            egui::vec2(tab_width, config.tab_bar_height),
            egui::Sense::click(),
        );

        // Draw tab background with pill shape
        // Use rounding based on tab height for a smooth pill appearance
        // Shrink vertically so borders are fully visible within tab bar
        let tab_draw_rect = tab_rect.shrink2(egui::vec2(0.0, 2.0));
        let tab_rounding = tab_draw_rect.height() / 2.0;
        if ui.is_rect_visible(tab_rect) {
            ui.painter()
                .rect_filled(tab_draw_rect, tab_rounding, bg_color);

            // Draw border around tab
            // Active tabs get a highlighted border using the indicator color
            if config.tab_border_width > 0.0 || is_active {
                let (border_color, border_width) = if is_active {
                    // Active tab: use indicator color and slightly thicker border
                    let c = if let Some(custom) = custom_color {
                        // Lighten the custom color for the indicator
                        let lighten = |v: u8| v.saturating_add(50);
                        [lighten(custom[0]), lighten(custom[1]), lighten(custom[2])]
                    } else {
                        config.tab_active_indicator
                    };
                    (c, config.tab_border_width.max(1.5))
                } else {
                    // Inactive tabs: use normal border color
                    (config.tab_border_color, config.tab_border_width)
                };

                if border_width > 0.0 {
                    ui.painter().rect_stroke(
                        tab_draw_rect,
                        tab_rounding,
                        egui::Stroke::new(
                            border_width,
                            egui::Color32::from_rgb(
                                border_color[0],
                                border_color[1],
                                border_color[2],
                            ),
                        ),
                        egui::StrokeKind::Middle,
                    );
                }
            }

            // Create a child UI for the tab content
            let mut content_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(tab_rect.shrink2(egui::vec2(8.0, 4.0)))
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );

            content_ui.horizontal(|ui| {
                // Bell indicator (takes priority over activity indicator)
                if is_bell_active {
                    let c = config.tab_bell_indicator;
                    ui.colored_label(egui::Color32::from_rgb(c[0], c[1], c[2]), "🔔");
                    ui.add_space(4.0);
                } else if has_activity && !is_active {
                    // Activity indicator
                    let c = config.tab_activity_indicator;
                    ui.colored_label(egui::Color32::from_rgb(c[0], c[1], c[2]), "•");
                    ui.add_space(4.0);
                }

                // Tab index if configured
                if config.tab_show_index {
                    // We'd need to get the index, skip for now
                }

                // Title (truncated)
                let max_title_len = if config.tab_show_close_button { 15 } else { 20 };
                let display_title = if title.len() > max_title_len {
                    format!("{}…", &title[..max_title_len - 1])
                } else {
                    title.to_string()
                };

                let text_color = if is_active {
                    let c = config.tab_active_text;
                    egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 255)
                } else {
                    let c = config.tab_inactive_text;
                    egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], opacity)
                };

                ui.label(egui::RichText::new(&display_title).color(text_color));

                // Spacer to push close button and hotkey to the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Close button
                    if config.tab_show_close_button {
                        let close_color = if self.close_hovered == Some(id) {
                            let c = config.tab_close_button_hover;
                            egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 255)
                        } else {
                            let c = config.tab_close_button;
                            egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], opacity)
                        };

                        let close_btn = ui.add(
                            egui::Button::new(
                                egui::RichText::new("×").color(close_color).size(14.0),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .frame(false),
                        );

                        if close_btn.hovered() {
                            self.close_hovered = Some(id);
                        } else if self.close_hovered == Some(id) {
                            self.close_hovered = None;
                        }

                        if close_btn.clicked() {
                            action = TabBarAction::Close(id);
                        }
                    }

                    // Hotkey indicator (only for tabs 1-9)
                    if index < 9 {
                        // Use ⌘ on macOS, ^ on other platforms
                        let modifier_symbol = if cfg!(target_os = "macos") {
                            "⌘"
                        } else {
                            "^"
                        };
                        let hotkey_text = format!("{}{}", modifier_symbol, index + 1);
                        let hotkey_color =
                            egui::Color32::from_rgba_unmultiplied(180, 180, 180, opacity);
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(hotkey_text)
                                .color(hotkey_color)
                                .size(11.0),
                        );
                    }
                });
            });
        }

        // Handle tab click (switch to tab)
        if tab_response.clicked() && action == TabBarAction::None {
            action = TabBarAction::SwitchTo(id);
        }

        // Handle right-click for context menu
        if tab_response.secondary_clicked() {
            // Initialize editing color from custom color or a default
            self.editing_color = custom_color.unwrap_or([100, 100, 100]);
            self.context_menu_tab = Some(id);
            // Store click position for menu placement
            if let Some(pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
                self.context_menu_pos = pos;
            }
            // Store frame number to avoid closing on same frame
            self.context_menu_opened_frame = ui.ctx().cumulative_frame_nr();
        }

        // Update hover state
        if tab_response.hovered() {
            self.hovered_tab = Some(id);
        } else if self.hovered_tab == Some(id) {
            self.hovered_tab = None;
        }

        action
    }

    /// Render the context menu for tab options
    fn render_context_menu(&mut self, ctx: &egui::Context, tab_id: TabId) -> TabBarAction {
        let mut action = TabBarAction::None;
        let mut close_menu = false;

        // Close on Escape
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close_menu = true;
        }

        let area_response = egui::Area::new(egui::Id::new("tab_context_menu"))
            .fixed_pos(self.context_menu_pos)
            .constrain(true)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::symmetric(1, 4))
                    .show(ui, |ui| {
                        ui.set_min_width(160.0);
                        ui.style_mut().spacing.item_spacing = egui::vec2(0.0, 0.0);

                        // Menu item helper
                        let menu_item = |ui: &mut egui::Ui, label: &str| -> bool {
                            let response = ui.add_sized(
                                [ui.available_width(), 24.0],
                                egui::Button::new(label)
                                    .frame(false)
                                    .fill(egui::Color32::TRANSPARENT),
                            );
                            response.clicked()
                        };

                        // Close Tab
                        if menu_item(ui, "Close Tab") {
                            action = TabBarAction::Close(tab_id);
                            close_menu = true;
                        }

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

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
                                ([220, 50, 50], "Red"),
                                ([220, 130, 50], "Orange"),
                                ([220, 180, 50], "Yellow"),
                                ([50, 180, 50], "Green"),
                                ([50, 180, 180], "Cyan"),
                                ([50, 100, 220], "Blue"),
                                ([180, 50, 180], "Purple"),
                            ];

                            for (color, name) in presets {
                                let btn = ui.add(
                                    egui::Button::new("")
                                        .fill(egui::Color32::from_rgb(color[0], color[1], color[2]))
                                        .min_size(egui::vec2(18.0, 18.0))
                                        .corner_radius(2.0),
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

        // Close menu if clicked outside (but not on the same frame it was opened)
        let current_frame = ctx.cumulative_frame_nr();
        if current_frame > self.context_menu_opened_frame
            && ctx.input(|i| i.pointer.any_click())
            && !area_response.response.hovered()
            // Only close if no action was taken (let button clicks register)
            && !close_menu
            && action == TabBarAction::None
        {
            close_menu = true;
        }

        // Close menu if action taken or cancelled
        if close_menu {
            self.context_menu_tab = None;
        }

        action
    }

    /// Get the tab bar height (0 if hidden)
    pub fn get_height(&self, tab_count: usize, config: &Config) -> f32 {
        if self.should_show(tab_count, config.tab_bar_mode) {
            config.tab_bar_height
        } else {
            0.0
        }
    }

    /// Check if the context menu is currently open
    pub fn is_context_menu_open(&self) -> bool {
        self.context_menu_tab.is_some()
    }
}

impl Default for TabBarUI {
    fn default() -> Self {
        Self::new()
    }
}

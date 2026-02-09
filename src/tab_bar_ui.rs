//! Tab bar UI using egui
//!
//! Provides a visual tab bar for switching between terminal tabs.

use crate::config::{Config, TabBarMode};
use crate::tab::{TabId, TabManager};

/// Styled text segment for rich tab titles
#[derive(Debug, Clone, PartialEq, Eq)]
struct StyledSegment {
    text: String,
    bold: bool,
    italic: bool,
    underline: bool,
    color: Option<[u8; 3]>,
}

#[derive(Clone, Copy, Debug)]
struct TitleStyle {
    bold: bool,
    italic: bool,
    underline: bool,
    color: Option<[u8; 3]>,
}

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

                    // Use clicked_by() to only respond to mouse clicks, not keyboard
                    if new_tab_btn.clicked_by(egui::PointerButton::Primary) {
                        action = TabBarAction::NewTab;
                    }

                    if new_tab_btn.hovered() {
                        #[cfg(target_os = "macos")]
                        new_tab_btn.on_hover_text("New Tab (Cmd+T)");
                        #[cfg(not(target_os = "macos"))]
                        new_tab_btn.on_hover_text("New Tab (Ctrl+Shift+T)");
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

        // Tab frame - allocate space for the tab
        let (tab_rect, _) = ui.allocate_exact_size(
            egui::vec2(tab_width, config.tab_bar_height),
            egui::Sense::hover(),
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

                // Title rendering with width-aware truncation
                let base_font_id = ui.style().text_styles[&egui::TextStyle::Button].clone();
                let indicator_width = if is_bell_active {
                    18.0
                } else if has_activity && !is_active {
                    14.0
                } else {
                    0.0
                };
                let hotkey_width = if index < 9 { 26.0 } else { 0.0 };
                let close_width = if config.tab_show_close_button {
                    24.0
                } else {
                    0.0
                };
                let padding = 12.0;
                let title_available_width =
                    (tab_width - indicator_width - hotkey_width - close_width - padding).max(24.0);

                let max_chars = estimate_max_chars(ui, &base_font_id, title_available_width);

                let text_color = if is_active {
                    let c = config.tab_active_text;
                    egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 255)
                } else {
                    let c = config.tab_inactive_text;
                    egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], opacity)
                };

                if config.tab_html_titles {
                    let segments = parse_html_title(title);
                    let truncated = truncate_segments(&segments, max_chars);
                    render_segments(ui, &truncated, text_color);
                } else {
                    let display_title = truncate_plain(title, max_chars);
                    ui.label(egui::RichText::new(display_title).color(text_color));
                }

                // Hotkey indicator (only for tabs 1-9) - show on right side, leave space for close button
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Add space for close button if shown
                    if config.tab_show_close_button {
                        ui.add_space(24.0);
                    }
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
                        ui.label(
                            egui::RichText::new(hotkey_text)
                                .color(hotkey_color)
                                .size(11.0),
                        );
                    }
                });
            });
        }

        // Close button - render AFTER the content so it's on top
        // Position at far right edge of tab
        let close_btn_size = 20.0;
        let close_btn_rect = if config.tab_show_close_button {
            Some(egui::Rect::from_min_size(
                egui::pos2(
                    tab_rect.right() - close_btn_size - 4.0,
                    tab_rect.center().y - close_btn_size / 2.0,
                ),
                egui::vec2(close_btn_size, close_btn_size),
            ))
        } else {
            None
        };

        // Check if pointer is over close button using egui's input state
        let pointer_pos = ui.ctx().input(|i| i.pointer.hover_pos());
        let close_hovered = close_btn_rect
            .zip(pointer_pos)
            .is_some_and(|(rect, pos)| rect.contains(pos));

        if close_hovered {
            self.close_hovered = Some(id);
        } else if self.close_hovered == Some(id) {
            self.close_hovered = None;
        }

        // Draw close button if configured
        if let Some(close_rect) = close_btn_rect {
            let close_color = if self.close_hovered == Some(id) {
                let c = config.tab_close_button_hover;
                egui::Color32::from_rgb(c[0], c[1], c[2])
            } else {
                let c = config.tab_close_button;
                egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], opacity)
            };

            // Draw the × character centered in the close button rect
            ui.painter().text(
                close_rect.center(),
                egui::Align2::CENTER_CENTER,
                "×",
                egui::FontId::proportional(14.0),
                close_color,
            );
        }

        // Handle tab click (switch to tab)
        // Use egui's built-in interact() for proper hit testing
        let tab_response = ui.interact(
            tab_rect,
            egui::Id::new(("tab_click", id)),
            egui::Sense::click(),
        );

        // Use egui's response for click detection
        let pointer_in_tab = tab_response.hovered();
        let clicked = tab_response.clicked_by(egui::PointerButton::Primary);

        // Detect click using clicked_by() to only respond to mouse clicks, not keyboard
        // This prevents Enter key from triggering tab switches when a tab has keyboard focus
        // IMPORTANT: Skip if close button is hovered - let the close button handle the click
        if clicked && action == TabBarAction::None && self.close_hovered != Some(id) {
            action = TabBarAction::SwitchTo(id);
        }

        // Handle close button click - check if close button is hovered
        if clicked && self.close_hovered == Some(id) {
            action = TabBarAction::Close(id);
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

        // Update hover state (using manual detection)
        if pointer_in_tab {
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

fn truncate_plain(title: &str, max_len: usize) -> String {
    if max_len == 0 {
        return "…".to_string();
    }
    let mut chars = title.chars();
    let mut taken = String::new();
    for _ in 0..max_len {
        if let Some(c) = chars.next() {
            taken.push(c);
        } else {
            return taken;
        }
    }
    if chars.next().is_some() {
        if max_len > 0 {
            taken.pop();
        }
        taken.push('…');
    }
    taken
}

fn truncate_segments(segments: &[StyledSegment], max_len: usize) -> Vec<StyledSegment> {
    if max_len == 0 {
        return vec![StyledSegment {
            text: "…".to_string(),
            bold: false,
            italic: false,
            underline: false,
            color: None,
        }];
    }
    let mut remaining = max_len;
    let mut out: Vec<StyledSegment> = Vec::new();
    for seg in segments {
        if remaining == 0 {
            break;
        }
        let seg_len = seg.text.chars().count();
        if seg_len == 0 {
            continue;
        }
        if seg_len <= remaining {
            out.push(seg.clone());
            remaining -= seg_len;
        } else {
            let truncated_text: String =
                seg.text.chars().take(remaining.saturating_sub(1)).collect();
            let mut truncated = seg.clone();
            truncated.text = truncated_text;
            truncated.text.push('…');
            out.push(truncated);
            remaining = 0;
        }
    }
    out
}

fn render_segments(ui: &mut egui::Ui, segments: &[StyledSegment], fallback_color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for segment in segments {
            let mut rich = egui::RichText::new(&segment.text);
            if segment.bold {
                rich = rich.strong();
            }
            if segment.italic {
                rich = rich.italics();
            }
            if segment.underline {
                rich = rich.underline();
            }
            if let Some(color) = segment.color {
                rich = rich.color(egui::Color32::from_rgb(color[0], color[1], color[2]));
            } else {
                rich = rich.color(fallback_color);
            }
            ui.label(rich);
        }
    });
}

fn estimate_max_chars(_ui: &egui::Ui, font_id: &egui::FontId, available_width: f32) -> usize {
    let char_width = (font_id.size * 0.55).max(4.0); // heuristic: ~0.55em per character
    ((available_width / char_width).floor() as usize).max(4)
}

fn parse_html_title(input: &str) -> Vec<StyledSegment> {
    let mut segments: Vec<StyledSegment> = Vec::new();
    let mut style_stack: Vec<TitleStyle> = vec![TitleStyle {
        bold: false,
        italic: false,
        underline: false,
        color: None,
    }];
    let mut buffer = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            // flush buffer
            if !buffer.is_empty() {
                let style = *style_stack.last().unwrap_or(&TitleStyle {
                    bold: false,
                    italic: false,
                    underline: false,
                    color: None,
                });
                segments.push(StyledSegment {
                    text: buffer.clone(),
                    bold: style.bold,
                    italic: style.italic,
                    underline: style.underline,
                    color: style.color,
                });
                buffer.clear();
            }

            // read tag
            let mut tag = String::new();
            while let Some(&c) = chars.peek() {
                chars.next();
                if c == '>' {
                    break;
                }
                tag.push(c);
            }

            let tag_trimmed = tag.trim().to_lowercase();
            match tag_trimmed.as_str() {
                "b" => {
                    let mut style = *style_stack.last().unwrap();
                    style.bold = true;
                    style_stack.push(style);
                }
                "/b" => {
                    pop_style(&mut style_stack, |s| s.bold);
                }
                "i" => {
                    let mut style = *style_stack.last().unwrap();
                    style.italic = true;
                    style_stack.push(style);
                }
                "/i" => {
                    pop_style(&mut style_stack, |s| s.italic);
                }
                "u" => {
                    let mut style = *style_stack.last().unwrap();
                    style.underline = true;
                    style_stack.push(style);
                }
                "/u" => {
                    pop_style(&mut style_stack, |s| s.underline);
                }
                t if t.starts_with("span") => {
                    if let Some(color) = parse_span_color(&tag_trimmed) {
                        let mut style = *style_stack.last().unwrap();
                        style.color = Some(color);
                        style_stack.push(style);
                    } else {
                        // unsupported span attributes: ignore tag
                    }
                }
                "/span" => {
                    pop_style(&mut style_stack, |s| s.color.is_some());
                }
                _ => {
                    // Unknown or unsupported tag, ignore
                }
            }
        } else {
            buffer.push(ch);
        }
    }

    if !buffer.is_empty() {
        let style = *style_stack.last().unwrap_or(&TitleStyle {
            bold: false,
            italic: false,
            underline: false,
            color: None,
        });
        segments.push(StyledSegment {
            text: buffer,
            bold: style.bold,
            italic: style.italic,
            underline: style.underline,
            color: style.color,
        });
    }

    segments
}

fn pop_style<F>(stack: &mut Vec<TitleStyle>, predicate: F)
where
    F: Fn(&TitleStyle) -> bool,
{
    if stack.len() <= 1 {
        return;
    }
    for idx in (1..stack.len()).rev() {
        let style = stack[idx];
        if predicate(&style) {
            stack.remove(idx);
            return;
        }
    }
}

fn parse_span_color(tag: &str) -> Option<[u8; 3]> {
    // expect like: span style="color:#rrggbb" or color:rgb(r,g,b)
    let style_attr = tag.split("style=").nth(1)?;
    let style_val = style_attr
        .trim_start_matches(['\"', '\''])
        .trim_end_matches(['\"', '\'']);
    let mut color_part = None;
    for decl in style_val.split(';') {
        let mut kv = decl.splitn(2, ':');
        let key = kv.next()?.trim();
        let val = kv.next()?.trim();
        if key == "color" {
            color_part = Some(val);
            break;
        }
    }
    let color_str = color_part?;
    if let Some(hex) = color_str.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some([r, g, b]);
        }
    } else if let Some(rgb) = color_str
        .strip_prefix("rgb(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let parts: Vec<&str> = rgb.split(',').map(|p| p.trim()).collect();
        if parts.len() == 3 {
            let r = parts[0].parse::<u8>().ok()?;
            let g = parts[1].parse::<u8>().ok()?;
            let b = parts[2].parse::<u8>().ok()?;
            return Some([r, g, b]);
        }
    }
    None
}

impl Default for TabBarUI {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_html_title_basic_tags() {
        let segments = parse_html_title("<b>Hello</b> <i>world</i>");
        assert_eq!(
            segments,
            vec![
                StyledSegment {
                    text: "Hello".to_string(),
                    bold: true,
                    italic: false,
                    underline: false,
                    color: None
                },
                StyledSegment {
                    text: " ".to_string(),
                    bold: false,
                    italic: false,
                    underline: false,
                    color: None
                },
                StyledSegment {
                    text: "world".to_string(),
                    bold: false,
                    italic: true,
                    underline: false,
                    color: None
                }
            ]
        );
    }

    #[test]
    fn parse_html_title_span_color() {
        let segments = parse_html_title("<span style=\"color:#ff0000\">Red</span> text");
        assert_eq!(segments.len(), 2);
        assert_eq!(
            segments[0],
            StyledSegment {
                text: "Red".to_string(),
                bold: false,
                italic: false,
                underline: false,
                color: Some([255, 0, 0])
            }
        );
    }

    #[test]
    fn truncate_segments_adds_ellipsis() {
        let segs = vec![StyledSegment {
            text: "HelloWorld".to_string(),
            bold: false,
            italic: false,
            underline: false,
            color: None,
        }];
        let truncated = truncate_segments(&segs, 6);
        assert_eq!(truncated[0].text, "Hello…");
    }

    #[test]
    fn truncate_plain_handles_short_text() {
        assert_eq!(truncate_plain("abc", 5), "abc");
        assert_eq!(truncate_plain("abcdef", 5), "abcd…");
    }
}

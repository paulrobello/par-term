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
    /// Color being edited in the color picker (for the context menu)
    editing_color: [u8; 3],
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
            editing_color: [100, 100, 100],
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

        // Tab bar area at the top
        let bar_bg = config.tab_bar_background;
        egui::TopBottomPanel::top("tab_bar")
            .exact_height(config.tab_bar_height)
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(bar_bg[0], bar_bg[1], bar_bg[2])))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Style for tabs
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                    // Render each tab
                    for tab in tabs.tabs() {
                        let is_active = Some(tab.id) == active_tab_id;
                        let is_bell_active = tab.is_bell_active();
                        let tab_action = self.render_tab(
                            ui,
                            tab.id,
                            &tab.title,
                            is_active,
                            tab.has_activity,
                            is_bell_active,
                            tab.custom_color,
                            config,
                        );

                        if tab_action != TabBarAction::None {
                            action = tab_action;
                        }
                    }

                    // New tab button
                    ui.add_space(4.0);
                    let new_tab_btn = ui.add(
                        egui::Button::new("+")
                            .min_size(egui::vec2(24.0, config.tab_bar_height - 4.0))
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

    /// Render a single tab and return any action triggered
    #[allow(clippy::too_many_arguments)]
    fn render_tab(
        &mut self,
        ui: &mut egui::Ui,
        id: TabId,
        title: &str,
        is_active: bool,
        has_activity: bool,
        is_bell_active: bool,
        custom_color: Option<[u8; 3]>,
        config: &Config,
    ) -> TabBarAction {
        let mut action = TabBarAction::None;

        // Calculate tab width (min 80px, max 200px)
        let available_width = ui.available_width();
        let tab_count = 10; // estimate
        let tab_width = (available_width / tab_count as f32).clamp(80.0, 200.0);

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

        // Draw tab background
        if ui.is_rect_visible(tab_rect) {
            ui.painter().rect_filled(tab_rect, 0.0, bg_color);

            // Draw a small color indicator dot if custom color is set (for inactive tabs)
            if custom_color.is_some() && !is_active {
                let dot_radius = 3.0;
                let dot_center = egui::pos2(tab_rect.right() - 8.0, tab_rect.top() + 8.0);
                if let Some(c) = custom_color {
                    ui.painter().circle_filled(
                        dot_center,
                        dot_radius,
                        egui::Color32::from_rgb(c[0], c[1], c[2]),
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

                // Spacer to push close button to the right
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
        }

        // Update hover state
        if tab_response.hovered() {
            self.hovered_tab = Some(id);
        } else if self.hovered_tab == Some(id) {
            self.hovered_tab = None;
        }

        // Active tab indicator (bottom border)
        if is_active {
            // Use custom color for indicator if set, otherwise use config
            let indicator_color = if let Some(c) = custom_color {
                // Lighten the custom color for the indicator
                let lighten = |v: u8| v.saturating_add(50);
                [lighten(c[0]), lighten(c[1]), lighten(c[2])]
            } else {
                config.tab_active_indicator
            };
            ui.painter().hline(
                tab_rect.left()..=tab_rect.right(),
                tab_rect.bottom() - 2.0,
                egui::Stroke::new(
                    2.0,
                    egui::Color32::from_rgb(
                        indicator_color[0],
                        indicator_color[1],
                        indicator_color[2],
                    ),
                ),
            );
        }

        action
    }

    /// Render the context menu for tab color selection
    fn render_context_menu(&mut self, ctx: &egui::Context, tab_id: TabId) -> TabBarAction {
        let mut action = TabBarAction::None;
        let mut close_menu = false;

        egui::Window::new("Tab Color")
            .id(egui::Id::new("tab_color_menu"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.label("Set custom tab color:");
                    ui.add_space(8.0);

                    // Color picker
                    ui.horizontal(|ui| {
                        ui.label("Color:");
                        ui.color_edit_button_srgb(&mut self.editing_color);
                    });

                    ui.add_space(8.0);

                    // Preset colors
                    ui.label("Presets:");
                    ui.horizontal(|ui| {
                        let presets: &[([u8; 3], &str)] = &[
                            ([220, 50, 50], "Red"),
                            ([50, 180, 50], "Green"),
                            ([50, 100, 220], "Blue"),
                            ([220, 180, 50], "Yellow"),
                            ([180, 50, 180], "Purple"),
                            ([50, 180, 180], "Cyan"),
                            ([220, 130, 50], "Orange"),
                        ];

                        for (color, name) in presets {
                            let btn = ui.add(
                                egui::Button::new("")
                                    .fill(egui::Color32::from_rgb(color[0], color[1], color[2]))
                                    .min_size(egui::vec2(24.0, 24.0)),
                            );
                            if btn.clicked() {
                                self.editing_color = *color;
                            }
                            if btn.hovered() {
                                btn.on_hover_text(*name);
                            }
                        }
                    });

                    ui.add_space(12.0);

                    // Action buttons
                    ui.horizontal(|ui| {
                        if ui.button("Apply").clicked() {
                            action = TabBarAction::SetColor(tab_id, self.editing_color);
                            close_menu = true;
                        }

                        if ui.button("Clear Color").clicked() {
                            action = TabBarAction::ClearColor(tab_id);
                            close_menu = true;
                        }

                        if ui.button("Cancel").clicked() {
                            close_menu = true;
                        }
                    });
                });
            });

        // Close menu if action taken or cancelled
        if close_menu {
            self.context_menu_tab = None;
        }

        action
    }

    /// Get the tab bar height (0 if hidden)
    #[allow(dead_code)]
    pub fn get_height(&self, tab_count: usize, config: &Config) -> f32 {
        if self.should_show(tab_count, config.tab_bar_mode) {
            config.tab_bar_height
        } else {
            0.0
        }
    }
}

impl Default for TabBarUI {
    fn default() -> Self {
        Self::new()
    }
}

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
}

impl TabBarUI {
    /// Create a new tab bar UI
    pub fn new() -> Self {
        Self {
            hovered_tab: None,
            close_hovered: None,
            drag_in_progress: false,
            dragging_tab: None,
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
        config: &Config,
    ) -> TabBarAction {
        let mut action = TabBarAction::None;

        // Calculate tab width (min 80px, max 200px)
        let available_width = ui.available_width();
        let tab_count = 10; // estimate
        let tab_width = (available_width / tab_count as f32).clamp(80.0, 200.0);

        // Tab background color
        let bg_color = if is_active {
            let c = config.tab_active_background;
            egui::Color32::from_rgb(c[0], c[1], c[2])
        } else if self.hovered_tab == Some(id) {
            let c = config.tab_hover_background;
            egui::Color32::from_rgb(c[0], c[1], c[2])
        } else {
            let c = config.tab_inactive_background;
            egui::Color32::from_rgb(c[0], c[1], c[2])
        };

        // Tab frame - use allocate_ui_with_layout to get a proper interactive response
        let (tab_rect, tab_response) = ui.allocate_exact_size(
            egui::vec2(tab_width, config.tab_bar_height),
            egui::Sense::click(),
        );

        // Draw tab background
        if ui.is_rect_visible(tab_rect) {
            ui.painter().rect_filled(tab_rect, 0.0, bg_color);

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
                    egui::Color32::from_rgb(c[0], c[1], c[2])
                } else {
                    let c = config.tab_inactive_text;
                    egui::Color32::from_rgb(c[0], c[1], c[2])
                };

                ui.label(egui::RichText::new(&display_title).color(text_color));

                // Spacer to push close button to the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Close button
                    if config.tab_show_close_button {
                        let close_color = if self.close_hovered == Some(id) {
                            let c = config.tab_close_button_hover;
                            egui::Color32::from_rgb(c[0], c[1], c[2])
                        } else {
                            let c = config.tab_close_button;
                            egui::Color32::from_rgb(c[0], c[1], c[2])
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

        // Update hover state
        if tab_response.hovered() {
            self.hovered_tab = Some(id);
        } else if self.hovered_tab == Some(id) {
            self.hovered_tab = None;
        }

        // Active tab indicator (bottom border)
        if is_active {
            let c = config.tab_active_indicator;
            ui.painter().hline(
                tab_rect.left()..=tab_rect.right(),
                tab_rect.bottom() - 2.0,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(c[0], c[1], c[2])),
            );
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

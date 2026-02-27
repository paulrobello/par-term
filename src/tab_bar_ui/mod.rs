//! Tab bar UI using egui
//!
//! Provides a visual tab bar for switching between terminal tabs.
//!
//! Sub-modules:
//! - [`title_utils`]: HTML title parsing, emoji sanitization, and styled segment rendering.
//! - [`context_menu`]: Right-click context menu (rename, color, icon, duplicate, close).
//! - [`drag_drop`]: Drag-and-drop state and rendering for tab reordering.
//! - [`profile_menu`]: Profile selection popup for the new-tab chevron button.
//! - [`tab_rendering`]: Individual tab rendering for horizontal and vertical layouts.

mod context_menu;
mod drag_drop;
mod profile_menu;
mod tab_rendering;
mod title_utils;

use crate::config::{Config, TabBarMode, TabBarPosition};
use crate::tab::{TabId, TabManager};
use crate::ui_constants::{
    TAB_DRAW_SHRINK_Y, TAB_LEFT_PADDING, TAB_NEW_BTN_BASE_WIDTH, TAB_SCROLL_BTN_WIDTH, TAB_SPACING,
};

/// Width reserved for the profile chevron (▾) button in the tab bar split button.
/// Accounts for the button min_size (14px) plus egui button frame padding (~4px each side).
const CHEVRON_RESERVED: f32 = 28.0;

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
    /// Create a new tab from a specific profile
    NewTabWithProfile(crate::profile::ProfileId),
    /// Reorder a tab to a new position
    Reorder(TabId, usize),
    /// Set custom color for a tab
    SetColor(TabId, [u8; 3]),
    /// Clear custom color for a tab (revert to default)
    ClearColor(TabId),
    /// Duplicate a specific tab
    Duplicate(TabId),
    /// Rename a specific tab
    RenameTab(TabId, String),
    /// Set custom icon for a tab (None = clear)
    SetTabIcon(TabId, Option<String>),
    /// Toggle the AI assistant panel
    ToggleAssistantPanel,
}

/// Tab bar UI state
pub struct TabBarUI {
    /// Currently hovered tab ID
    pub hovered_tab: Option<TabId>,
    /// Tab where close button is hovered
    pub close_hovered: Option<TabId>,
    /// Whether a drag is in progress
    drag_in_progress: bool,
    /// Tab being dragged
    dragging_tab: Option<TabId>,
    /// Cached title of the tab being dragged (for ghost rendering)
    dragging_title: String,
    /// Cached color of the tab being dragged
    dragging_color: Option<[u8; 3]>,
    /// Width of the tab being dragged (for ghost rendering)
    dragging_tab_width: f32,
    /// Visual indicator for where the dragged tab would be inserted
    drop_target_index: Option<usize>,
    /// Per-frame cache of tab rects for drop target calculation
    tab_rects: Vec<(TabId, egui::Rect)>,
    /// Tab ID for which context menu is open
    context_menu_tab: Option<TabId>,
    /// Position where context menu was opened
    context_menu_pos: egui::Pos2,
    /// Frame when context menu was opened (to avoid closing on same frame)
    context_menu_opened_frame: u64,
    /// Color being edited in the color picker (for the context menu)
    editing_color: [u8; 3],
    /// Whether the rename text field is active in the context menu
    renaming_tab: bool,
    /// Frame when rename mode was activated (to ignore the activating click)
    rename_activated_frame: u64,
    /// Buffer for the rename text field
    rename_buffer: String,
    /// Title of the tab in the context menu (for rename pre-fill)
    context_menu_title: String,
    /// Whether the icon picker is active in the context menu
    picking_icon: bool,
    /// Frame when icon picker mode was activated (to ignore the activating click)
    icon_activated_frame: u64,
    /// Buffer for the icon text field in the context menu
    icon_buffer: String,
    /// Current custom icon of the tab in the context menu (for "Clear Icon" visibility)
    context_menu_icon: Option<String>,
    /// Horizontal scroll offset for tabs (in pixels)
    scroll_offset: f32,
    /// Whether the new-tab profile popup is open
    pub show_new_tab_profile_menu: bool,
}

impl TabBarUI {
    /// Create a new tab bar UI
    pub fn new() -> Self {
        Self {
            hovered_tab: None,
            close_hovered: None,
            drag_in_progress: false,
            dragging_tab: None,
            dragging_title: String::new(),
            dragging_color: None,
            dragging_tab_width: 0.0,
            drop_target_index: None,
            tab_rects: Vec::new(),
            context_menu_tab: None,
            context_menu_pos: egui::Pos2::ZERO,
            context_menu_opened_frame: 0,
            editing_color: [100, 100, 100],
            renaming_tab: false,
            rename_activated_frame: 0,
            rename_buffer: String::new(),
            context_menu_title: String::new(),
            picking_icon: false,
            icon_activated_frame: 0,
            icon_buffer: String::new(),
            context_menu_icon: None,
            scroll_offset: 0.0,
            show_new_tab_profile_menu: false,
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

    /// Check if a drag operation is in progress
    pub fn is_dragging(&self) -> bool {
        self.drag_in_progress
    }

    /// Render the tab bar and return any action triggered
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        tabs: &TabManager,
        config: &Config,
        profiles: &crate::profile::ProfileManager,
        right_reserved_width: f32,
    ) -> TabBarAction {
        let tab_count = tabs.tab_count();

        // Don't show if configured to hide
        if !self.should_show(tab_count, config.tab_bar_mode) {
            return TabBarAction::None;
        }

        match config.tab_bar_position {
            TabBarPosition::Left => self.render_vertical(ctx, tabs, config, profiles),
            _ => self.render_horizontal(ctx, tabs, config, profiles, right_reserved_width),
        }
    }

    /// Render the tab bar in horizontal layout (top or bottom)
    fn render_horizontal(
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
                                        tab.custom_icon.as_deref().or(tab.profile_icon.as_deref()),
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
                            tab.custom_icon.as_deref().or(tab.profile_icon.as_deref()),
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

    /// Render the tab bar in vertical layout (left side panel)
    fn render_vertical(
        &mut self,
        ctx: &egui::Context,
        tabs: &TabManager,
        config: &Config,
        profiles: &crate::profile::ProfileManager,
    ) -> TabBarAction {
        let tab_count = tabs.tab_count();

        self.tab_rects.clear();

        let mut action = TabBarAction::None;
        let active_tab_id = tabs.active_tab_id();

        let bar_bg = config.tab_bar_background;
        let tab_spacing = TAB_SPACING;
        let tab_height = config.tab_bar_height; // Reuse height config for per-tab row height

        egui::SidePanel::left("tab_bar")
            .exact_width(config.tab_bar_width)
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(bar_bg[0], bar_bg[1], bar_bg[2])))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                    )
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, tab_spacing);

                            for (index, tab) in tabs.tabs().iter().enumerate() {
                                let is_active = Some(tab.id) == active_tab_id;
                                let is_bell_active = tab.is_bell_active();
                                let (tab_action, tab_rect) = self.render_vertical_tab(
                                    ui,
                                    tab.id,
                                    index,
                                    &tab.title,
                                    tab.custom_icon.as_deref().or(tab.profile_icon.as_deref()),
                                    tab.custom_icon.as_deref(),
                                    is_active,
                                    tab.has_activity,
                                    is_bell_active,
                                    tab.custom_color,
                                    config,
                                    tab_height,
                                    tab_count,
                                );
                                self.tab_rects.push((tab.id, tab_rect));

                                if tab_action != TabBarAction::None {
                                    action = tab_action;
                                }
                            }

                            // New tab split button
                            ui.add_space(tab_spacing);
                            ui.horizontal(|ui| {
                                // Zero spacing between + and ▾
                                ui.spacing_mut().item_spacing.x = 0.0;

                                let show_chevron_v =
                                    !profiles.is_empty() || config.ai_inspector_enabled;
                                let chevron_space = if show_chevron_v {
                                    CHEVRON_RESERVED
                                } else {
                                    0.0
                                };
                                let plus_btn = ui.add(
                                    egui::Button::new("+")
                                        .min_size(egui::vec2(
                                            ui.available_width() - chevron_space,
                                            tab_height - TAB_DRAW_SHRINK_Y * 2.0,
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

                                if show_chevron_v {
                                    let chevron_btn = ui.add(
                                        egui::Button::new("⏷")
                                            .min_size(egui::vec2(
                                                CHEVRON_RESERVED / 2.0,
                                                tab_height - TAB_DRAW_SHRINK_Y * 2.0,
                                            ))
                                            .fill(egui::Color32::TRANSPARENT),
                                    );
                                    if chevron_btn.clicked_by(egui::PointerButton::Primary) {
                                        self.show_new_tab_profile_menu =
                                            !self.show_new_tab_profile_menu;
                                    }
                                    if chevron_btn.hovered() {
                                        chevron_btn.on_hover_text("New tab from profile");
                                    }
                                }
                            });
                        });
                    });

                // Handle drag feedback for vertical mode
                if self.drag_in_progress {
                    let drag_action = self.render_vertical_drag_feedback(ui, config);
                    if drag_action != TabBarAction::None {
                        action = drag_action;
                    }
                }
            });

        // Render floating ghost tab during drag
        if self.drag_in_progress && self.dragging_tab.is_some() {
            self.render_ghost_tab(ctx, config);
        }

        // Handle context menu
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

    /// Get the tab bar height (0 if hidden or if position is Left)
    pub fn get_height(&self, tab_count: usize, config: &Config) -> f32 {
        if self.should_show(tab_count, config.tab_bar_mode)
            && config.tab_bar_position.is_horizontal()
        {
            config.tab_bar_height
        } else {
            0.0
        }
    }

    /// Get the tab bar width (non-zero only for Left position, 0 if hidden)
    pub fn get_width(&self, tab_count: usize, config: &Config) -> f32 {
        if self.should_show(tab_count, config.tab_bar_mode)
            && config.tab_bar_position == TabBarPosition::Left
        {
            config.tab_bar_width
        } else {
            0.0
        }
    }

    /// Check if the context menu is currently open
    pub fn is_context_menu_open(&self) -> bool {
        self.context_menu_tab.is_some()
    }

    /// Check if the tab rename text field is active
    pub fn is_renaming(&self) -> bool {
        self.renaming_tab && self.context_menu_tab.is_some()
    }

    /// Calculate the drop target insert index for a horizontal drag given a pointer x position.
    ///
    /// Returns `None` if the drop would be a no-op (same position as source), or
    /// `Some(insert_index)` for a valid insertion point.
    ///
    /// This is a pure helper that can be tested without egui rendering.
    pub fn calculate_drop_target_horizontal(
        tab_rects: &[(TabId, egui::Rect)],
        drag_source_index: Option<usize>,
        pointer_x: f32,
    ) -> Option<usize> {
        let mut insert_index = tab_rects.len();
        for (i, (_id, rect)) in tab_rects.iter().enumerate() {
            if pointer_x < rect.center().x {
                insert_index = i;
                break;
            }
        }
        let is_noop =
            drag_source_index.is_some_and(|src| insert_index == src || insert_index == src + 1);
        if is_noop { None } else { Some(insert_index) }
    }

    /// Convert an insertion index to an effective target index, accounting for source removal.
    ///
    /// When a tab is removed from `source_index` and re-inserted at `insert_index`, indices
    /// after the source shift down by one.  This helper applies that adjustment.
    pub fn insertion_to_target_index(
        insert_index: usize,
        drag_source_index: Option<usize>,
    ) -> usize {
        if let Some(src) = drag_source_index {
            if insert_index > src {
                insert_index - 1
            } else {
                insert_index
            }
        } else {
            insert_index
        }
    }

    /// Set drag state directly; used by integration tests to exercise state transitions
    /// without requiring a live egui render loop.
    pub fn test_set_drag_state(&mut self, tab_id: Option<TabId>, in_progress: bool) {
        self.drag_in_progress = in_progress;
        self.dragging_tab = tab_id;
    }

    /// Set the drop target index directly; used by integration tests.
    pub fn test_set_drop_target(&mut self, index: Option<usize>) {
        self.drop_target_index = index;
    }

    /// Get the current drop target index; used by integration tests.
    pub fn test_drop_target_index(&self) -> Option<usize> {
        self.drop_target_index
    }

    /// Get the id of the tab currently being dragged; used by integration tests.
    pub fn test_dragging_tab(&self) -> Option<TabId> {
        self.dragging_tab
    }

    /// Open the context menu for a specific tab; used by integration tests.
    pub fn test_open_context_menu(&mut self, tab_id: TabId) {
        self.context_menu_tab = Some(tab_id);
        self.context_menu_opened_frame = 0;
        self.renaming_tab = false;
        self.picking_icon = false;
    }

    /// Close the context menu; used by integration tests.
    pub fn test_close_context_menu(&mut self) {
        self.context_menu_tab = None;
        self.renaming_tab = false;
        self.picking_icon = false;
    }

    /// Get the context menu tab id; used by integration tests.
    pub fn test_context_menu_tab(&self) -> Option<TabId> {
        self.context_menu_tab
    }

    /// Set rename mode active/inactive; used by integration tests.
    pub fn test_set_renaming(&mut self, value: bool) {
        self.renaming_tab = value;
    }
}

impl Default for TabBarUI {
    fn default() -> Self {
        Self::new()
    }
}

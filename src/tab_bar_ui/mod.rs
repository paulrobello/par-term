//! Tab bar UI using egui
//!
//! Provides a visual tab bar for switching between terminal tabs.
//!
//! ## Module layout
//!
//! - [`state`]: `TabBarUI` struct definition and constructor.
//! - [`horizontal`]: Horizontal layout rendering (`render_horizontal`).
//! - [`context_menu`]: Right-click context menu (rename, color, icon, duplicate, close).
//! - [`drag_drop`]: Drag-and-drop state and rendering for tab reordering.
//! - [`profile_menu`]: Profile selection popup for the new-tab chevron button.
//! - [`tab_rendering`]: Individual tab rendering for horizontal and vertical layouts.
//! - [`title_utils`]: HTML title parsing, emoji sanitization, and styled segment rendering.

mod context_menu;
mod drag_drop;
mod horizontal;
mod profile_menu;
mod state;
mod tab_rendering;
mod title_utils;

// Re-export TabBarUI so external callers are unaffected.
pub use state::TabBarUI;

use crate::config::{Config, TabBarMode, TabBarPosition};
use crate::tab::{TabId, TabManager};
use crate::ui_constants::{TAB_DRAW_SHRINK_Y, TAB_SPACING};
use tab_rendering::TabRenderParams;

/// Width reserved for the profile chevron (▾) button in the tab bar split button.
/// Accounts for the button min_size (14px) plus egui button frame padding (~4px each side).
pub(super) const CHEVRON_RESERVED: f32 = 28.0;

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

impl TabBarUI {
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
                                    TabRenderParams {
                                        id: tab.id,
                                        index,
                                        title: &tab.title,
                                        profile_icon: tab
                                            .custom_icon
                                            .as_deref()
                                            .or(tab.profile.profile_icon.as_deref()),
                                        custom_icon: tab.custom_icon.as_deref(),
                                        is_active,
                                        has_activity: tab.has_activity,
                                        is_bell_active,
                                        custom_color: tab.custom_color,
                                        config,
                                        tab_size: tab_height,
                                        tab_count,
                                    },
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

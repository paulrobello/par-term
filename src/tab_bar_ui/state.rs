//! `TabBarUI` struct definition and constructor.

use crate::tab::TabId;
use crate::ui_constants::TAB_SPACING;
use winit::event::MouseScrollDelta;

/// Tab bar UI state
pub struct TabBarUI {
    /// Currently hovered tab ID
    pub hovered_tab: Option<TabId>,
    /// Tab where close button is hovered
    pub close_hovered: Option<TabId>,
    /// Whether a drag is in progress
    pub(super) drag_in_progress: bool,
    /// Tab being dragged
    pub(super) dragging_tab: Option<TabId>,
    /// Cached title of the tab being dragged (for ghost rendering)
    pub(super) dragging_title: String,
    /// Cached color of the tab being dragged
    pub(super) dragging_color: Option<[u8; 3]>,
    /// Width of the tab being dragged (for ghost rendering)
    pub(super) dragging_tab_width: f32,
    /// Visual indicator for where the dragged tab would be inserted
    pub(super) drop_target_index: Option<usize>,
    /// Per-frame cache of tab rects for drop target calculation
    pub(super) tab_rects: Vec<(TabId, egui::Rect)>,
    /// Tab ID for which context menu is open
    pub(super) context_menu_tab: Option<TabId>,
    /// Position where context menu was opened
    pub(super) context_menu_pos: egui::Pos2,
    /// Frame when context menu was opened (to avoid closing on same frame)
    pub(super) context_menu_opened_frame: u64,
    /// Color being edited in the color picker (for the context menu)
    pub(super) editing_color: [u8; 3],
    /// Whether the rename text field is active in the context menu
    pub(super) renaming_tab: bool,
    /// Frame when rename mode was activated (to ignore the activating click)
    pub(super) rename_activated_frame: u64,
    /// Buffer for the rename text field
    pub(super) rename_buffer: String,
    /// Title of the tab in the context menu (for rename pre-fill)
    pub(super) context_menu_title: String,
    /// Whether the icon picker is active in the context menu
    pub(super) picking_icon: bool,
    /// Frame when icon picker mode was activated (to ignore the activating click)
    pub(super) icon_activated_frame: u64,
    /// Buffer for the icon text field in the context menu
    pub(super) icon_buffer: String,
    /// Current custom icon of the tab in the context menu (for "Clear Icon" visibility)
    pub(super) context_menu_icon: Option<String>,
    /// Horizontal scroll offset for tabs (in pixels)
    pub(super) scroll_offset: f32,
    /// Whether the horizontal tab bar needs scroll (more tabs than fit).
    /// Set each frame by `render_horizontal`.
    pub(super) needs_horizontal_scroll: bool,
    /// Whether the new-tab profile popup is open
    pub show_new_tab_profile_menu: bool,
    /// Set per-frame: candidate destination windows for the "Move Tab to Window →" submenu.
    /// Each entry is `(WindowId, display_label)` (e.g., `"Window 2 — vim"`).
    pub(crate) move_candidates: Vec<(winit::window::WindowId, String)>,
    /// Set per-frame: true if the current window has an active tmux gateway.
    /// When true, the Move Tab menu entries are disabled for every tab.
    pub(crate) move_gateway_active: bool,
    /// Set per-frame: number of tabs in the source window. Used to disable
    /// "Move Tab to New Window" when `== 1` (solo-tab guard).
    pub(crate) move_source_tab_count: usize,
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
            needs_horizontal_scroll: false,
            show_new_tab_profile_menu: false,
            move_candidates: Vec::new(),
            move_gateway_active: false,
            move_source_tab_count: 0,
        }
    }
}

impl Default for TabBarUI {
    fn default() -> Self {
        Self::new()
    }
}

impl TabBarUI {
    /// Handle mouse wheel when hovering over the horizontal tab bar.
    /// Converts vertical scroll delta to horizontal tab scrolling.
    /// Returns `true` if the event was consumed.
    pub fn handle_mouse_wheel(
        &mut self,
        delta: &MouseScrollDelta,
        tab_min_width: f32,
        tab_count: usize,
    ) -> bool {
        if !self.needs_horizontal_scroll || tab_count == 0 {
            return false;
        }

        // Convert vertical wheel delta to horizontal scroll.
        // Positive y = scroll up = reveal tabs to the left (decrease offset).
        let scroll_amount = match delta {
            MouseScrollDelta::LineDelta(_x, y) => *y * (tab_min_width + TAB_SPACING),
            MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
        };

        if scroll_amount.abs() < 0.5 {
            return false;
        }

        // Invert: scroll-up (positive y) reveals tabs to the left (decrease offset).
        self.scroll_offset = (self.scroll_offset - scroll_amount).max(0.0);
        true
    }
}

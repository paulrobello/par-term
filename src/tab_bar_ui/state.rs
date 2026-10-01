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
    /// A session-chip click this frame (UX.md V1), taken by the window
    /// after the egui pass.
    pub(crate) chip_action: Option<crate::session_chip::SessionChipAction>,
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
    /// The active tab the last frame scrolled into view (UX.md V13): a
    /// switch to a different tab brings it into view once, then the user's
    /// own scrolling is left alone.
    pub(super) scrolled_to_active: Option<TabId>,
    /// Whether the horizontal tab bar needs scroll (more tabs than fit).
    /// Set each frame by `render_horizontal`.
    pub(super) needs_horizontal_scroll: bool,
    /// Set per-frame: candidate destination windows for the "Move Tab to Window →" submenu.
    /// Each entry is `(WindowId, display_label)` (e.g., `"Window 2 — vim"`).
    pub(crate) move_candidates: Vec<(winit::window::WindowId, String)>,
    /// Set per-frame: true if the current window has an active tmux gateway.
    /// When true, the Move Tab menu entries are disabled for every tab.
    pub(crate) move_gateway_active: bool,
    /// Set per-frame: number of tabs in the source window. Used to disable
    /// "Move Tab to New Window" when `== 1` (solo-tab guard).
    pub(crate) move_source_tab_count: usize,
    /// Set per-frame: true when the context-menu tab has multiple panes.
    pub(crate) tab_has_multiple_panes: bool,
    /// Set per-frame: why Demote is refused for the context-menu tab (a
    /// par-mux tab that cannot `join-pane`), shown as the disabled item's
    /// hover text. `None` = the standard enable rule applies.
    pub(crate) demote_refusal: Option<&'static str>,
    /// The in-app menu, drawn in the tab bar strip on platforms that cannot
    /// attach a native menu bar. Inert elsewhere — see [`crate::menu::AppMenuUi`].
    pub(super) app_menu: crate::menu::AppMenuUi,
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
            chip_action: None,
            renaming_tab: false,
            rename_activated_frame: 0,
            rename_buffer: String::new(),
            context_menu_title: String::new(),
            picking_icon: false,
            icon_activated_frame: 0,
            icon_buffer: String::new(),
            context_menu_icon: None,
            scroll_offset: 0.0,
            scrolled_to_active: None,
            needs_horizontal_scroll: false,
            move_candidates: Vec::new(),
            move_gateway_active: false,
            move_source_tab_count: 0,
            tab_has_multiple_panes: false,
            demote_refusal: None,
            app_menu: crate::menu::AppMenuUi::new(),
        }
    }

    /// Build the tab bar with the in-app menu sourcing accelerators from the
    /// live config's keybindings.
    pub fn new_with(keybindings: &[par_term_config::KeyBinding]) -> Self {
        let mut bar = Self::new();
        bar.app_menu = crate::menu::AppMenuUi::new_with(keybindings);
        bar
    }

    /// Keep the in-app menu in step with the window: rebuilt when the
    /// bindings change, drawn with the window's state (UX.md MN2/MN3).
    pub fn sync_app_menu(
        &mut self,
        keybindings: &[par_term_config::KeyBinding],
        state: crate::menu::state::MenuState,
    ) {
        self.app_menu.sync(keybindings, state);
    }

    /// The in-app menu, for inspection.
    #[cfg(test)]
    pub(crate) fn app_menu(&self) -> &crate::menu::AppMenuUi {
        &self.app_menu
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

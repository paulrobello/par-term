//! The facts the menu bar reads from a window (UX.md MN2).
//!
//! [`WindowState::menu_state`] gathers them once per tick for the focused
//! window; `WindowManager::sync_menus` adds the cross-window facts (window
//! list, arrangements) and applies the result to both menus.

use crate::app::window_state::WindowState;
use crate::menu::state::{Check, MenuState, MoveBlock};

impl WindowState {
    /// The window-local half of the menu state. The window list and saved
    /// arrangements are filled in by the manager.
    pub(crate) fn menu_state(&self) -> MenuState {
        let cfg = self.config.load();
        let tab = self.tab_manager.active_tab();
        let pane_manager = tab.and_then(|t| t.pane_manager());
        let multiple_panes = tab.is_some_and(|t| t.has_multiple_panes());
        let visible = self.tab_manager.visible_tabs();
        let mux_attached = self.tmux_state.is_mux_attached();
        let session_attached = mux_attached || self.is_gateway_active();
        let move_block = match self.tab_manager.active_tab_id() {
            Some(id) if self.mux_window_for_tab(id).is_some() => Some(MoveBlock::Attached),
            _ if self.is_gateway_active() => Some(MoveBlock::Gateway),
            _ => None,
        };

        let mut checks = std::collections::BTreeSet::new();
        let mut check = |on: bool, which: Check| {
            if on {
                checks.insert(which);
            }
        };
        check(self.is_fullscreen, Check::Fullscreen);
        check(
            pane_manager.is_some_and(|pm| pm.is_zoomed()),
            Check::PaneZoom,
        );
        check(
            self.overlay_ui.profile_drawer_ui.expanded,
            Check::ProfileDrawer,
        );
        check(self.overlay_ui.ai_inspector.open, Check::AssistantPanel);
        check(self.overlay_ui.agent_usage_panel.visible, Check::AgentUsage);
        check(self.debug.show_fps_overlay, Check::FpsOverlay);
        check(cfg.shader.custom_shader_enabled, Check::BackgroundShader);
        check(cfg.shader.custom_shader_animation, Check::ShaderAnimation);
        check(
            cfg.shader.custom_shader_readability_mode,
            Check::ShaderReadability,
        );
        check(cfg.shader.cursor_shader_enabled, Check::CursorShader);
        check(cfg.rendering.maximize_throughput, Check::Throughput);
        check(cfg.window.window_always_on_top, Check::AlwaysOnTop);
        check(tab.is_some_and(|t| t.broadcast_input), Check::BroadcastTab);
        check(
            pane_manager
                .and_then(|pm| pm.focused_pane())
                .is_some_and(|p| p.broadcast_excluded),
            Check::PaneBroadcastExcluded,
        );
        check(
            tab.is_some_and(|t| t.is_session_logging_active()),
            Check::OutputRecording,
        );
        check(self.is_copy_mode_active(), Check::CopyMode);

        MenuState {
            open_toggles: self.open_toggles(),
            modal_open: self.any_modal_ui_visible(),
            multiple_panes,
            tab_count: visible.len(),
            tab_titles: visible.iter().take(9).map(|t| t.title.clone()).collect(),
            windows: Vec::new(),
            mux_available: cfg!(feature = "mux"),
            session_attached,
            mux_attached,
            move_block,
            arrangements: Vec::new(),
            checks,
        }
    }

    /// Registry ids of the toggles whose panel or mode is open: their chord
    /// must keep working behind the open panel so it can close it.
    fn open_toggles(&self) -> std::collections::BTreeSet<&'static str> {
        let o = &self.overlay_ui;
        [
            (o.command_palette.visible, "toggle_command_palette"),
            (o.tree_picker_ui.visible, "toggle_tree_picker"),
            (o.tmux_session_picker_ui.visible, "toggle_session_picker"),
            (o.search_ui.visible, "toggle_search"),
            (o.clipboard_history_ui.visible, "toggle_clipboard_history"),
            (o.command_history_ui.visible, "toggle_command_history"),
            (o.agent_usage_panel.visible, "toggle_agent_usage_panel"),
            (o.ai_inspector.open, "toggle_ai_inspector"),
            (o.profile_drawer_ui.expanded, "toggle_profile_drawer"),
            (self.is_copy_mode_active(), "toggle_copy_mode"),
        ]
        .into_iter()
        .filter_map(|(open, id)| open.then_some(id))
        .collect()
    }

    /// Which accelerators the macOS menu keeps (see
    /// `crate::menu::state::release_captured_accelerators`): `None` while
    /// the terminal owns the keyboard, else the toggles of the open panels.
    pub(crate) fn menu_capture(&self) -> crate::menu::state::Capture {
        self.keyboard_captured().then(|| self.open_toggles())
    }

    /// Whether the keyboard belongs to something other than the terminal's
    /// key bindings — a dialog, a focused text field, an inline rename, the
    /// in-app menu, or a modal mode that owns the next key (copy mode, pane
    /// hints, resize mode, a prefix key). The macOS menu drops the
    /// accelerators those must receive while this holds (see
    /// `crate::menu::sync`).
    pub(crate) fn keyboard_captured(&self) -> bool {
        self.any_modal_ui_visible()
            || self.is_egui_using_keyboard()
            || self.tab_bar_ui.is_renaming()
            || self.overlay_ui.pane_rename_ui.is_open()
            || self.is_copy_mode_active()
            || self.pane_hint_select.is_active()
            || self.pane_resize_mode.is_active()
            || self.custom_action_prefix_state.is_active()
            || self.tmux_state.tmux_prefix_state.is_active()
    }
}

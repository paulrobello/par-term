//! UI state query helpers for `WindowState`.
//!
//! Covers:
//! - egui pointer / keyboard ownership queries (`is_egui_using_pointer`, `is_egui_using_keyboard`)
//! - Modal-visibility query helpers (`any_modal_ui_visible`, `has_egui_text_overlay_visible`)
//! - Scrollbar visibility logic (`should_show_scrollbar`)

use super::WindowState;

impl WindowState {
    // ========================================================================
    // egui / UI state queries
    // ========================================================================

    /// Check if egui is currently using the pointer (mouse is over an egui UI element)
    pub(crate) fn is_egui_using_pointer(&self) -> bool {
        // AI Inspector resize handle uses direct pointer tracking (not egui widgets),
        // so egui doesn't know about it. Check explicitly to prevent mouse events
        // from reaching the terminal during resize drag or initial click on the handle.
        if self.overlay_ui.ai_inspector.wants_pointer() {
            return true;
        }
        // Before first render, egui state is unreliable - allow mouse events through
        if !self.egui.initialized {
            return false;
        }
        // Always check egui context - the tab bar is always rendered via egui
        // and can consume pointer events (e.g., close button clicks)
        if let Some(ctx) = &self.egui.ctx {
            ctx.is_using_pointer() || ctx.wants_pointer_input()
        } else {
            false
        }
    }

    /// Canonical check: is any modal UI overlay visible?
    ///
    /// This is the single source of truth for "should input be blocked from the terminal
    /// because a modal dialog is open?" When adding a new modal panel, add it here.
    ///
    /// Note: Side panels (ai_inspector, profile drawer) and inline edit states
    /// (tab_bar_ui.is_renaming()) are NOT modals — they are checked separately
    /// at call sites that need them. The resize overlay is also not a modal.
    pub(crate) fn any_modal_ui_visible(&self) -> bool {
        self.overlay_ui.help_ui.visible
            || self.overlay_ui.clipboard_history_ui.visible
            || self.overlay_ui.command_history_ui.visible
            || self.overlay_ui.search_ui.visible
            || self.overlay_ui.tmux_session_picker_ui.visible
            || self.overlay_ui.shader_install_ui.visible
            || self.overlay_ui.integrations_ui.visible
            || self.overlay_ui.ssh_connect_ui.is_visible()
            || self.overlay_ui.remote_shell_install_ui.is_visible()
            || self.overlay_ui.quit_confirmation_ui.is_visible()
    }

    /// Check if any egui overlay with text input is visible.
    /// Used to route clipboard operations (paste/copy/select-all) to egui
    /// instead of the terminal when a modal dialog or the AI inspector is active.
    pub(crate) fn has_egui_text_overlay_visible(&self) -> bool {
        self.any_modal_ui_visible() || self.overlay_ui.ai_inspector.open
    }

    /// Check if egui is currently using keyboard input (e.g., text input or ComboBox has focus)
    pub(crate) fn is_egui_using_keyboard(&self) -> bool {
        // If any UI panel is visible, check if egui wants keyboard input
        // Note: Settings are handled by standalone SettingsWindow, not embedded UI
        // Note: Profile drawer does NOT block input - only modal dialogs do
        // Also check ai_inspector (side panel with text input) and tab rename (inline edit)
        let any_ui_visible = self.any_modal_ui_visible()
            || self.overlay_ui.ai_inspector.open
            || self.tab_bar_ui.is_renaming();
        if !any_ui_visible {
            return false;
        }

        // Check egui context for keyboard usage
        if let Some(ctx) = &self.egui.ctx {
            ctx.wants_keyboard_input()
        } else {
            false
        }
    }

    // ========================================================================
    // Scrollbar visibility
    // ========================================================================

    /// Determine if scrollbar should be visible based on autohide setting and recent activity
    pub(crate) fn should_show_scrollbar(&self) -> bool {
        let tab = match self.tab_manager.active_tab() {
            Some(t) => t,
            None => return false,
        };

        // No scrollbar needed if no scrollback available
        if tab.active_cache().scrollback_len == 0 {
            return false;
        }

        // Always show when dragging or moving
        if tab.active_scroll_state().dragging {
            return true;
        }

        // If autohide disabled, always show
        if self.config.load().scrollbar_autohide_delay == 0 {
            return true;
        }

        // If scrolled away from bottom, keep visible
        if tab.active_scroll_state().offset > 0 || tab.active_scroll_state().target_offset > 0 {
            return true;
        }

        // Show when pointer is near the scrollbar edge (hover reveal)
        if let Some(window) = &self.window {
            let padding = 32.0; // px hover band
            let width = window.inner_size().width as f64;
            let near_right = self.config.load().scrollbar_position != "left"
                && (width - tab.active_mouse().position.0) <= padding;
            let near_left = self.config.load().scrollbar_position == "left"
                && tab.active_mouse().position.0 <= padding;
            if near_left || near_right {
                return true;
            }
        }

        // Otherwise, hide after delay
        tab.active_scroll_state()
            .last_activity
            .elapsed()
            .as_millis()
            < self.config.load().scrollbar_autohide_delay as u128
    }
}

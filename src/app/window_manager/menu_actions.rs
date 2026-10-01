//! Menu action handling for the window manager.
//!
//! Almost every menu item runs a registry action ([`MenuAction::Action`])
//! through `execute_keybinding_action` — the same handler its chord runs, so
//! the Clear Scrollback item clears the focused pane (B58) and New Tab
//! honors `new_tab_shortcut_shows_profiles` (B10). The arms here are the
//! manager-level commands and the menu-only ones (Copy, Paste, profiles,
//! arrangements, About).

use std::sync::Arc;

use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::menu::MenuAction;

use super::WindowManager;

/// Documentation opened by Help › par-term Help.
const DOCS_URL: &str = "https://github.com/paulrobello/par-term/tree/main/docs";

/// Upper bound on bridge drains per tick: a registry action may queue a
/// manager-level action (new window, quit, window cycling) that must run in
/// the same tick, but a handler that re-queued itself must not spin.
const MAX_BRIDGE_DRAINS: usize = 4;

impl WindowManager {
    /// Handle a menu action
    pub fn handle_menu_action(
        &mut self,
        action: MenuAction,
        event_loop: &ActiveEventLoop,
        focused_window: Option<WindowId>,
    ) {
        match action {
            MenuAction::NewWindow => {
                self.create_window(event_loop);
            }
            MenuAction::CloseWindow => {
                // Smart close: close tab if multiple tabs, close window if single tab
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && window_state.close_current_tab()
                {
                    // Last tab closed, close the window
                    self.close_window(window_id);
                }
            }
            MenuAction::CloseWholeWindow => self.close_whole_window(focused_window),
            MenuAction::Action(id) => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.execute_keybinding_action(id);
                    window_state.request_redraw();
                }
            }
            MenuAction::CycleWindow(step) => self.cycle_window_focus(focused_window, step),
            MenuAction::FocusWindowNumber(n) => self.focus_window_number(n),
            MenuAction::Quit => {
                // TW2: capture every window before the first close — the
                // close loop below tears down one window at a time and the
                // last-window save alone recorded only the final survivor.
                self.save_session_for_quit();
                // Close all windows
                let window_ids: Vec<_> = self.windows.keys().copied().collect();
                for window_id in window_ids {
                    self.close_window(window_id);
                }
            }
            MenuAction::Copy => self.menu_copy(focused_window),
            MenuAction::Paste => self.menu_paste(focused_window),
            MenuAction::SelectAll => self.menu_select_all(focused_window),
            MenuAction::OpenSettings => {
                self.open_settings_window(event_loop);
            }
            MenuAction::Minimize => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get(&window_id)
                    && let Some(window) = &window_state.window
                {
                    window.set_minimized(true);
                }
            }
            MenuAction::Zoom => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get(&window_id)
                    && let Some(window) = &window_state.window
                {
                    window.set_maximized(!window.is_maximized());
                }
            }
            MenuAction::About => {
                // The help overlay opens with the "About par-term" section
                // (version, description, author, license, repository), so reuse
                // it for the About menu item. Force it visible rather than
                // toggling — selecting "About" should always open it.
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.overlay_ui.help_ui.visible = true;
                    if let Some(window) = &window_state.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::OpenDocs => {
                if let Err(e) = open::that(DOCS_URL) {
                    log::error!("Failed to open the documentation: {e}");
                }
            }
            MenuAction::ManageProfiles => {
                self.open_settings_window(event_loop);
                if let Some(sw) = &mut self.settings_window {
                    sw.settings_ui
                        .open_section(par_term_settings_ui::layout::deep_link::PROFILES);
                }
            }
            MenuAction::OpenProfile(profile_id) => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.open_profile(profile_id);
                }
            }
            MenuAction::SaveArrangement => {
                // Open settings at Sessions › Arrangements › Save Current Layout (B51)
                self.open_settings_window(event_loop);
                if let Some(sw) = &mut self.settings_window {
                    sw.settings_ui
                        .open_section(par_term_settings_ui::layout::deep_link::SAVE_ARRANGEMENT);
                }
            }
            MenuAction::RestoreArrangement(id) => self.restore_arrangement(id, event_loop),
            MenuAction::InstallShellIntegrationRemote => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state
                        .overlay_ui
                        .remote_shell_install_ui
                        .show_dialog();
                    window_state.focus_state.needs_redraw = true;
                }
            }
        }
    }

    /// Edit › Copy: routed into a focused egui text field when one owns the
    /// keyboard (the native accelerator intercepts Cmd+C before egui).
    fn menu_copy(&mut self, focused_window: Option<WindowId>) {
        if let Some(sw) = &self.settings_window
            && sw.is_focused()
        {
            if let Some(sw) = &mut self.settings_window {
                sw.inject_event(egui::Event::Copy);
            }
            return;
        }
        if let Some(window_id) = focused_window
            && let Some(window_state) = self.windows.get_mut(&window_id)
            && window_state.has_egui_text_overlay_visible()
        {
            window_state.egui.pending_events.push(egui::Event::Copy);
            return;
        }
        if let Some(window_id) = focused_window
            && let Some(window_state) = self.windows.get_mut(&window_id)
            && let Some(text) = window_state.get_selected_text_for_copy()
        {
            if let Err(e) = window_state.input_handler.copy_to_clipboard(&text) {
                log::error!("Failed to copy to clipboard: {}", e);
            } else {
                // Sync to tmux paste buffer if connected
                window_state.sync_clipboard_to_tmux(&text);
            }
        }
    }

    /// Edit › Paste: routed into a focused egui text field when one owns the
    /// keyboard (the native accelerator intercepts Cmd+V before egui).
    fn menu_paste(&mut self, focused_window: Option<WindowId>) {
        if let Some(sw) = &self.settings_window
            && sw.is_focused()
        {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                if let Ok(text) = clipboard.get_text() {
                    if let Some(sw) = &mut self.settings_window {
                        sw.inject_paste(text);
                    }
                    return;
                }
                // Clipboard has no text — check for image below.
                if clipboard.get_image().is_err() {
                    // Neither text nor image — nothing to paste
                    return;
                }
            } else {
                return;
            }
        }
        if let Some(window_id) = focused_window
            && let Some(window_state) = self.windows.get_mut(&window_id)
            && window_state.has_egui_text_overlay_visible()
        {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                if let Ok(text) = clipboard.get_text() {
                    window_state
                        .egui
                        .pending_events
                        .push(egui::Event::Paste(text));
                    return;
                }
                // Clipboard has no text — fall through to check for image
                if clipboard.get_image().is_err() {
                    return;
                }
            } else {
                return;
            }
        }
        if let Some(window_id) = focused_window
            && let Some(window_state) = self.windows.get_mut(&window_id)
        {
            if let Some(text) = window_state.input_handler.paste_from_clipboard() {
                window_state.paste_text(&text);
            } else if window_state.input_handler.clipboard_has_image() {
                // Clipboard has an image but no text — forward as Ctrl+V (0x16) so
                // image-aware child processes (e.g., Claude Code) can handle image paste
                if let Some(tab) = window_state.tab_manager.active_tab() {
                    // A mux tab's `tab.terminal` is a hidden login
                    // shell — route the daemon pane first.
                    if !window_state.route_mux_tab_write(tab, b"\x16") {
                        let terminal_clone = Arc::clone(&tab.terminal);
                        window_state.runtime.spawn(async move {
                            let term = terminal_clone.read().await;
                            if let Err(e) = term.write(b"\x16") {
                                crate::debug_error!(
                                    "INPUT",
                                    "PTY write failed (menu image paste): {e}"
                                );
                            }
                        });
                    }
                }
            }
        }
    }

    /// Edit › Select All: the focused text field when one owns the keyboard,
    /// else the whole terminal buffer.
    fn menu_select_all(&mut self, focused_window: Option<WindowId>) {
        let select_all_key = egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        };
        if let Some(sw) = &self.settings_window
            && sw.is_focused()
        {
            if let Some(sw) = &mut self.settings_window {
                // egui has no dedicated SelectAll event; use Cmd+A key event
                sw.inject_event(select_all_key);
            }
            return;
        }
        if let Some(window_id) = focused_window
            && let Some(window_state) = self.windows.get_mut(&window_id)
            && window_state.has_egui_text_overlay_visible()
        {
            window_state.egui.pending_events.push(select_all_key);
            return;
        }
        // Terminal focused: select the entire buffer (scrollback + visible
        // screen). The visible screen is highlighted; a subsequent Copy
        // pulls the full buffer via export_text.
        if let Some(window_id) = focused_window
            && let Some(window_state) = self.windows.get_mut(&window_id)
        {
            window_state.select_all();
            if let Some(window) = &window_state.window {
                window.request_redraw();
            }
        }
    }

    /// Process any pending menu events
    ///
    /// Drains both menus: the native one, whose activations arrive through
    /// muda's event channel, and the in-app egui one, which queues actions in
    /// [`crate::menu::drain_pending_actions`] from inside a window's render
    /// pass. The in-app queue is drained even when no [`crate::menu::MenuManager`]
    /// exists, since that menu does not depend on one.
    ///
    /// A native activation that arrives while the focused window's keyboard
    /// belongs to a dialog is dropped unless it is one of the commands the
    /// key handler's modal guard lets through (UX.md B61): the menu item is
    /// disabled then, but the disable lands a tick after the dialog opens.
    pub fn process_menu_events(
        &mut self,
        event_loop: &ActiveEventLoop,
        focused_window: Option<WindowId>,
    ) {
        let mut actions: Vec<MenuAction> = match &mut self.menu {
            Some(menu) => menu.poll_events(),
            None => Vec::new(),
        };
        if let Some(ws) = focused_window.and_then(|id| self.windows.get(&id))
            && ws.any_modal_ui_visible()
        {
            let state = ws.menu_state();
            actions.retain(|action| {
                let allowed = state.passes_with(action, &ws.keybinding_registry);
                if !allowed {
                    log::debug!("menu: {action:?} dropped while a dialog owns the keyboard");
                }
                allowed
            });
        }
        for _ in 0..MAX_BRIDGE_DRAINS {
            actions.extend(crate::menu::drain_pending_actions());
            if actions.is_empty() {
                break;
            }
            for action in std::mem::take(&mut actions) {
                self.handle_menu_action(action, event_loop, focused_window);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::menu::MenuAction;
    use crate::menu::model::{all_items, menu_model};

    /// The menu item `id` in either platform variant.
    fn action_of(id: &str) -> MenuAction {
        for has_native_app_menu in [false, true] {
            let model = menu_model(has_native_app_menu);
            if let Some(spec) = all_items(&model).into_iter().find(|s| s.id == id) {
                return spec.action;
            }
        }
        panic!("no menu item {id:?}");
    }

    /// B58: the Clear Scrollback item runs the `clear_scrollback` registry
    /// action, whose handler is the focused-pane helper — never a
    /// tab.terminal clear (wrong pane in splits, hidden login shell in
    /// par-mux tabs). The accelerator intercepts Cmd+Shift+K before the
    /// keybinding layer on macOS, so this item IS the Cmd+Shift+K path.
    #[test]
    fn menu_clear_scrollback_runs_the_focused_pane_registry_action() {
        assert_eq!(
            action_of("clear_scrollback"),
            MenuAction::Action("clear_scrollback")
        );
        let table = include_str!("../input_events/keybinding_actions.rs");
        assert!(
            table.contains("(\"clear_scrollback\", clear_scrollback)")
                && table
                    .contains("use super::keybinding_helpers::{clear_screen, clear_scrollback}"),
            "clear_scrollback must dispatch to keybinding_helpers::clear_scrollback"
        );
    }

    /// B68: Make Text Normal Size runs the `reset_font_size` registry action,
    /// whose handler restores the configured size, never a hard-coded 14.0.
    #[test]
    fn menu_reset_font_size_restores_the_configured_size() {
        assert_eq!(
            action_of("reset_font_size"),
            MenuAction::Action("reset_font_size")
        );
        let table = include_str!("../input_events/keybinding_display_actions.rs");
        assert!(
            table.contains("super::keybinding_helpers::reset_font_size_to_configured"),
            "reset_font_size must dispatch to the configured-size helper"
        );
    }

    /// B10: New Tab runs the `new_tab` registry action, which honors
    /// `new_tab_shortcut_shows_profiles` — the native accelerator owns the
    /// chord on macOS and Windows, so a menu-side `new_tab()` made the
    /// setting Linux-only.
    #[test]
    fn menu_new_tab_runs_the_registry_action() {
        assert_eq!(action_of("new_tab"), MenuAction::Action("new_tab"));
    }
}

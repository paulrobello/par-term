//! Window teardown: closing a window and reaping its resources.
//!
//! Split from `window_lifecycle` when that file crossed the 800-line gate.
//! Creation and monitor positioning stay there; everything reached from
//! `close_window` lives here.

use std::sync::Arc;

use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use super::WindowManager;

impl WindowManager {
    /// Close a specific window
    pub fn close_window(&mut self, window_id: WindowId) {
        // Save session state before removing the last window (while data is still available).
        if self.config.load().session_restore.restore_session
            && self.windows.len() == 1
            && self.windows.contains_key(&window_id)
        {
            self.save_session_state_background();
        }

        if let Some(window_state) = self.windows.remove(&window_id) {
            log::info!(
                "Closing window {:?} (remaining: {})",
                window_id,
                self.windows.len()
            );
            // Hide window immediately for instant visual feedback
            if let Some(ref window) = window_state.window {
                window.set_visible(false);
            }
            // WindowState's Drop impl handles cleanup
            drop(window_state);
        }

        // Exit app when last window closes
        if self.windows.is_empty() {
            log::info!("Last window closed, exiting application");
            // Close settings window FIRST before marking exit
            if self.settings_window.is_some() {
                log::info!("Closing settings window before exit");
                self.close_settings_window();
            }
            self.should_exit = true;
        }
    }

    /// Produce `(WindowId, display_label)` pairs for every window *other than*
    /// `source_window_id`, suitable for the "Move Tab to Window ->" submenu.
    ///
    /// Label format: `Window N - <active tab title>`, falling back to `Window N`
    /// if the active tab has no meaningful title.
    pub(crate) fn other_window_labels(
        &self,
        source_window_id: WindowId,
    ) -> Vec<(WindowId, String)> {
        self.windows
            .iter()
            .filter(|(id, _)| **id != source_window_id)
            .map(|(id, ws)| {
                let active_title = ws
                    .tab_manager
                    .active_tab()
                    .map(|t| t.title.trim().to_string())
                    .filter(|s| !s.is_empty());
                let label = match active_title {
                    Some(title) => format!("Window {} - {}", ws.window_index, title),
                    None => format!("Window {}", ws.window_index),
                };
                (*id, label)
            })
            .collect()
    }

    /// Move a live `Tab` from `source_window` to `destination`, preserving the
    /// PTY, scrollback, split panes, and all other per-tab state.
    ///
    /// Orchestration:
    /// 1. Pre-flight validation (source/tab exist, no active tmux gateway,
    ///    not a solo-tab pop-out, destination distinct and present).
    /// 2. Record source geometry, resolve/create destination window.
    /// 3. `remove_tab` from source (which also flags source empty if this was
    ///    the last tab), `stop_refresh_task` so the captured source `Arc<Window>`
    ///    is released.
    /// 4. `insert_tab_at` end of destination, rebind `start_refresh_task` against
    ///    the destination's `Arc<Window>`. The destination may renumber the tab
    ///    (its id can already be taken there, since ids are per-window), so the
    ///    rebind looks the tab up by the id `insert_tab_at` returns, not by the
    ///    `tab_id` this function was called with.
    /// 5. Focus destination window. If the source is now empty, close it.
    pub(crate) fn move_tab(
        &mut self,
        event_loop: &ActiveEventLoop,
        source_window: WindowId,
        tab_id: crate::tab::TabId,
        destination: super::MoveDestination,
    ) {
        use super::MoveDestination;

        // --- Pre-flight validation ---
        let Some(source_state) = self.windows.get(&source_window) else {
            log::warn!("move_tab: source window {:?} not found", source_window);
            return;
        };

        if source_state.tab_manager.get_tab(tab_id).is_none() {
            log::warn!("move_tab: tab {} not in window {:?}", tab_id, source_window);
            return;
        }

        // Reject if tmux gateway is active anywhere in this window.
        if source_state.is_gateway_active() {
            log::warn!(
                "move_tab: refusing to move tab {} - source window has an active tmux gateway",
                tab_id
            );
            return;
        }

        match destination {
            MoveDestination::NewWindow => {
                if source_state.tab_manager.tab_count() <= 1 {
                    log::warn!(
                        "move_tab: refusing solo-tab pop-out for tab {} (would be a no-op)",
                        tab_id
                    );
                    return;
                }
            }
            MoveDestination::ExistingWindow(dest_id) => {
                if dest_id == source_window {
                    log::warn!("move_tab: destination == source, ignoring");
                    return;
                }
                if !self.windows.contains_key(&dest_id) {
                    log::warn!("move_tab: destination window {:?} not found", dest_id);
                    return;
                }
            }
        }

        crate::debug_info!(
            "TAB",
            "Moving tab {} from window {:?} to {:?}",
            tab_id,
            source_window,
            destination
        );

        // --- Record source geometry before we mutate anything ---
        let (source_size, source_outer_pos) = {
            let ws = self.windows.get(&source_window).expect("validated above");
            let win = ws.window.as_ref();
            let size = win
                .map(|w| w.inner_size())
                .unwrap_or(winit::dpi::PhysicalSize::new(800, 600));
            let pos = win
                .and_then(|w| w.outer_position().ok())
                .unwrap_or(winit::dpi::PhysicalPosition::new(0, 0));
            (size, pos)
        };

        // --- Resolve destination ---
        let dest_window_id = match destination {
            MoveDestination::ExistingWindow(id) => id,
            MoveDestination::NewWindow => {
                let clamped_pos = Self::compute_moved_tab_outer_position(
                    event_loop,
                    source_outer_pos,
                    source_size,
                );
                match self.create_window_for_moved_tab(event_loop, source_size, clamped_pos) {
                    Some(id) => id,
                    None => {
                        crate::debug_error!(
                            "TAB",
                            "move_tab: create_window_for_moved_tab failed - source state untouched"
                        );
                        return;
                    }
                }
            }
        };

        // --- Extract from source ---
        let Some(source_state) = self.windows.get_mut(&source_window) else {
            crate::debug_error!("TAB", "move_tab: source window disappeared mid-flight");
            return;
        };
        let Some((mut live_tab, source_is_empty)) = source_state.tab_manager.remove_tab(tab_id)
        else {
            crate::debug_error!(
                "TAB",
                "move_tab: remove_tab returned None for tab {}",
                tab_id
            );
            return;
        };

        // Stop the refresh task - its captured Arc<Window> still points at the source.
        live_tab.stop_refresh_task();

        // --- Insert into destination ---
        let Some(dest_state) = self.windows.get_mut(&dest_window_id) else {
            crate::debug_error!(
                "TAB",
                "move_tab: destination window {:?} disappeared - dropping tab",
                dest_window_id
            );
            return;
        };
        let insert_at = dest_state.tab_manager.tab_count();
        // The destination renumbers the tab if `tab_id` is already taken there.
        // Everything below must use `dest_tab_id`: looking up the source id
        // would miss the tab, leaving it with no refresh task and frozen.
        let dest_tab_id = dest_state.tab_manager.insert_tab_at(live_tab, insert_at);

        // --- Rebind refresh task to the destination window ---
        if let Some(dest_win_arc) = dest_state.window.clone() {
            let active_fps = dest_state.config.load().rendering.max_fps;
            let inactive_fps = dest_state.config.load().power.inactive_tab_fps;
            if let Some(tab) = dest_state.tab_manager.get_tab_mut(dest_tab_id) {
                tab.start_refresh_task(
                    Arc::clone(&self.runtime),
                    dest_win_arc,
                    active_fps,
                    inactive_fps,
                );
            }
        }

        // --- Request redraw, raise, and focus destination ---
        if let Some(win) = dest_state.window.as_ref() {
            win.request_redraw();
            win.set_visible(true);
            win.focus_window();
        }

        crate::debug_info!(
            "TAB",
            "move_tab complete (source empty: {})",
            source_is_empty
        );

        // --- Close source if emptied ---
        if source_is_empty {
            self.close_window(source_window);
        }
    }
}

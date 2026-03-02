//! Tmux notification polling — drains, converts and routes control-mode events.
//!
//! `check_tmux_notifications` is called each frame from `about_to_wait`.
//! It pulls raw notifications from the gateway terminal's parser, converts
//! them via `ParserBridge`, then dispatches each to the appropriate handler
//! grouped by type (session/window → layout → output → other).

use crate::app::window_state::WindowState;
use crate::tmux::{ParserBridge, TmuxNotification};

impl WindowState {
    /// Retry any deferred `set_tmux_control_mode(false)` calls on all tabs.
    ///
    /// When `handle_tmux_session_ended` cannot acquire the terminal lock via `try_lock()`
    /// it sets `tab.tmux.pending_tmux_mode_disable = true`. This function is called each frame
    /// from `check_tmux_notifications` and clears the flag as soon as the lock becomes
    /// available, ensuring the terminal parser eventually exits tmux control mode.
    fn retry_pending_tmux_mode_disable(&mut self) {
        for tab in self.tab_manager.tabs_mut() {
            if !tab.tmux.pending_tmux_mode_disable {
                continue;
            }
            // try_lock: intentional — we are in the sync event loop. On miss: leave the
            // flag set and retry next frame. The lock will be free once the PTY reader
            // finishes its current read (which is short-lived).
            if let Ok(term) = tab.terminal.try_write() {
                term.set_tmux_control_mode(false);
                tab.tmux.pending_tmux_mode_disable = false;
                crate::debug_info!(
                    "TAB",
                    "Deferred tmux control mode disable applied to tab {}",
                    tab.id
                );
            }
        }
    }

    /// Poll and process tmux notifications from the control mode session.
    ///
    /// In gateway mode, notifications come from the terminal's tmux control parser
    /// rather than a separate channel. This should be called in about_to_wait.
    ///
    /// Returns true if any notifications were processed (triggers redraw).
    pub(crate) fn check_tmux_notifications(&mut self) -> bool {
        // Early exit if tmux integration is disabled
        if !self.config.tmux_enabled {
            return false;
        }

        // Deferred tmux-control-mode disable: retry on each frame until the lock is
        // available. This resolves the case where `handle_tmux_session_ended` could not
        // acquire the terminal lock at cleanup time, leaving the parser in control mode.
        self.retry_pending_tmux_mode_disable();

        // Check if we have an active gateway session
        let _session = match &self.tmux_state.tmux_session {
            Some(s) if s.is_gateway_active() => s,
            _ => return false,
        };

        // Get the gateway tab ID - this is where the tmux control connection lives
        let gateway_tab_id = match self.tmux_state.tmux_gateway_tab_id {
            Some(id) => id,
            None => return false,
        };

        // Drain notifications from the gateway tab's terminal tmux parser
        let core_notifications = if let Some(tab) = self.tab_manager.get_tab(gateway_tab_id) {
            // try_lock: intentional — called from the sync event loop (about_to_wait) where
            // blocking would stall the entire GUI. On miss: returns false (no notifications
            // processed this frame); they will be picked up on the next poll cycle.
            if let Ok(term) = tab.terminal.try_write() {
                term.drain_tmux_notifications()
            } else {
                return false;
            }
        } else {
            return false;
        };

        if core_notifications.is_empty() {
            return false;
        }

        // Log all raw core notifications for debugging
        for (i, notif) in core_notifications.iter().enumerate() {
            crate::debug_info!(
                "TMUX",
                "Core notification {}/{}: {:?}",
                i + 1,
                core_notifications.len(),
                notif
            );
        }

        // Convert core notifications to frontend notifications
        let notifications = ParserBridge::convert_all(core_notifications);
        if notifications.is_empty() {
            crate::debug_trace!(
                "TMUX",
                "All core notifications were filtered out by parser bridge"
            );
            return false;
        }

        crate::debug_info!(
            "TMUX",
            "Processing {} tmux notifications (gateway mode)",
            notifications.len()
        );

        let mut needs_redraw = false;

        // First, update gateway state based on notifications
        for notification in &notifications {
            crate::debug_trace!("TMUX", "Processing notification: {:?}", notification);
            if let Some(session) = &mut self.tmux_state.tmux_session
                && session.process_gateway_notification(notification)
            {
                crate::debug_info!(
                    "TMUX",
                    "State transition - gateway_state: {:?}, session_state: {:?}",
                    session.gateway_state(),
                    session.state()
                );
                needs_redraw = true;
            }
        }

        // Separate notifications into two buckets:
        //   • direct — handled by dedicated handlers (TmuxSync cannot translate these)
        //   • sync   — routed through TmuxSync for ID translation, then dispatched via
        //              process_sync_actions in priority order: session → layout → output → other
        //
        // Processing in groups preserves the ordering guarantee: session/window structure
        // is set up (and window→tab mappings created) before layout changes are applied,
        // and pane mappings from layout are available before output arrives.

        let mut direct_notifications = Vec::new();
        let mut session_sync = Vec::new();
        let mut layout_sync = Vec::new();
        let mut output_sync = Vec::new();
        let mut other_sync = Vec::new();

        for notification in notifications {
            match &notification {
                TmuxNotification::ControlModeStarted
                | TmuxNotification::SessionStarted(_)
                | TmuxNotification::SessionRenamed(_)
                | TmuxNotification::PaneFocusChanged { .. }
                | TmuxNotification::Error(_) => {
                    direct_notifications.push(notification);
                }
                TmuxNotification::WindowAdd(_)
                | TmuxNotification::WindowClose(_)
                | TmuxNotification::WindowRenamed { .. }
                | TmuxNotification::SessionEnded => {
                    session_sync.push(notification);
                }
                TmuxNotification::LayoutChange { .. } => {
                    layout_sync.push(notification);
                }
                TmuxNotification::Output { .. } => {
                    output_sync.push(notification);
                }
                TmuxNotification::Pause | TmuxNotification::Continue => {
                    other_sync.push(notification);
                }
            }
        }

        // --- Direct dispatch (notifications TmuxSync does not handle) ---
        for notification in direct_notifications {
            match notification {
                TmuxNotification::ControlModeStarted => {
                    crate::debug_info!("TMUX", "Control mode started - tmux is ready");
                }
                TmuxNotification::SessionStarted(session_name) => {
                    self.handle_tmux_session_started(&session_name);
                    needs_redraw = true;
                }
                TmuxNotification::SessionRenamed(session_name) => {
                    self.handle_tmux_session_renamed(&session_name);
                    needs_redraw = true;
                }
                TmuxNotification::Error(msg) => {
                    self.handle_tmux_error(&msg);
                }
                TmuxNotification::PaneFocusChanged { pane_id } => {
                    self.handle_tmux_pane_focus_changed(pane_id);
                    needs_redraw = true;
                }
                _ => {}
            }
        }

        // --- TmuxSync dispatch: group 1 — session/window structure ---
        // Creates window→tab mappings; must run before layout and output.
        let session_actions = self
            .tmux_state
            .tmux_sync
            .process_notifications(&session_sync);
        needs_redraw |= self.process_sync_actions(session_actions);

        // --- TmuxSync dispatch: group 2 — layout changes ---
        // Applies pane layout; requires window mappings from group 1.
        let layout_actions = self
            .tmux_state
            .tmux_sync
            .process_notifications(&layout_sync);
        needs_redraw |= self.process_sync_actions(layout_actions);

        // Fallback: LayoutChange notifications that TmuxSync could not translate
        // (no window→tab mapping yet). The direct handler handles on-the-fly mapping.
        for notification in &layout_sync {
            if let TmuxNotification::LayoutChange { window_id, layout } = notification
                && self.tmux_state.tmux_sync.get_tab(*window_id).is_none()
            {
                self.handle_tmux_layout_change(*window_id, layout);
                needs_redraw = true;
            }
        }

        // --- TmuxSync dispatch: group 3 — pane output ---
        // Routes bytes to native panes; requires pane mappings from group 2.
        let output_actions = self
            .tmux_state
            .tmux_sync
            .process_notifications(&output_sync);
        needs_redraw |= self.process_sync_actions(output_actions);

        // Fallback: Output for panes not yet mapped. The direct handler has multi-level
        // fallback logic (tab-level routing, on-demand tab creation).
        for notification in output_sync {
            if let TmuxNotification::Output { pane_id, data } = notification
                && self.tmux_state.tmux_sync.get_native_pane(pane_id).is_none()
            {
                self.handle_tmux_output(pane_id, &data);
                needs_redraw = true;
            }
        }

        // --- TmuxSync dispatch: group 4 — flow control (pause/continue) ---
        let other_actions = self.tmux_state.tmux_sync.process_notifications(&other_sync);
        needs_redraw |= self.process_sync_actions(other_actions);

        needs_redraw
    }
}

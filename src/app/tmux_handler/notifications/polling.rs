//! Tmux notification polling — drains, converts and routes control-mode events.
//!
//! `check_tmux_notifications` is called each frame from `about_to_wait`.
//! It pulls raw notifications from the gateway terminal's parser, converts
//! them via `ParserBridge`, then dispatches each to the appropriate handler
//! grouped by type (session/window → layout → output → other).

use crate::app::window_state::WindowState;
use crate::tmux::{ParserBridge, TmuxNotification};

impl WindowState {
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
            if let Ok(term) = tab.terminal.try_lock() {
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

        // Process notifications in priority order:
        // 1. Session/Window structure (setup)
        // 2. LayoutChange (creates pane mappings)
        // 3. Output (uses pane mappings)
        // This ensures pane mappings exist before output arrives.

        // Separate notifications by type for ordered processing
        let mut session_notifications = Vec::new();
        let mut layout_notifications = Vec::new();
        let mut output_notifications = Vec::new();
        let mut other_notifications = Vec::new();

        for notification in notifications {
            match &notification {
                TmuxNotification::ControlModeStarted
                | TmuxNotification::SessionStarted(_)
                | TmuxNotification::SessionRenamed(_)
                | TmuxNotification::WindowAdd(_)
                | TmuxNotification::WindowClose(_)
                | TmuxNotification::WindowRenamed { .. }
                | TmuxNotification::SessionEnded => {
                    session_notifications.push(notification);
                }
                TmuxNotification::LayoutChange { .. } => {
                    layout_notifications.push(notification);
                }
                TmuxNotification::Output { .. } => {
                    output_notifications.push(notification);
                }
                TmuxNotification::PaneFocusChanged { .. }
                | TmuxNotification::Error(_)
                | TmuxNotification::Pause
                | TmuxNotification::Continue => {
                    other_notifications.push(notification);
                }
            }
        }

        // Process session/window structure first
        for notification in session_notifications {
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
                TmuxNotification::WindowAdd(window_id) => {
                    self.handle_tmux_window_add(window_id);
                    needs_redraw = true;
                }
                TmuxNotification::WindowClose(window_id) => {
                    self.handle_tmux_window_close(window_id);
                    needs_redraw = true;
                }
                TmuxNotification::WindowRenamed { id, name } => {
                    self.handle_tmux_window_renamed(id, &name);
                    needs_redraw = true;
                }
                TmuxNotification::SessionEnded => {
                    self.handle_tmux_session_ended();
                    needs_redraw = true;
                }
                _ => {}
            }
        }

        // Process layout changes second (creates pane mappings)
        for notification in layout_notifications {
            if let TmuxNotification::LayoutChange { window_id, layout } = notification {
                self.handle_tmux_layout_change(window_id, &layout);
                needs_redraw = true;
            }
        }

        // Process output third (uses pane mappings)
        for notification in output_notifications {
            if let TmuxNotification::Output { pane_id, data } = notification {
                self.handle_tmux_output(pane_id, &data);
                needs_redraw = true;
            }
        }

        // Process other notifications last
        for notification in other_notifications {
            match notification {
                TmuxNotification::Error(msg) => {
                    self.handle_tmux_error(&msg);
                }
                TmuxNotification::Pause => {
                    self.handle_tmux_pause();
                }
                TmuxNotification::Continue => {
                    self.handle_tmux_continue();
                    needs_redraw = true;
                }
                TmuxNotification::PaneFocusChanged { pane_id } => {
                    self.handle_tmux_pane_focus_changed(pane_id);
                    needs_redraw = true;
                }
                _ => {}
            }
        }

        needs_redraw
    }
}

//! Session-level tmux notification handlers.
//!
//! Covers session lifecycle events: session started/renamed/ended, client-size
//! synchronization, and window-title management.

use crate::app::window_state::WindowState;

impl WindowState {
    /// Handle session started notification
    pub(super) fn handle_tmux_session_started(&mut self, session_name: &str) {
        crate::debug_info!("TMUX", "Session started: {}", session_name);

        // Store the session name for later use (e.g., window title updates)
        self.tmux_state.tmux_session_name = Some(session_name.to_string());

        // Update window title with session name: "par-term - [tmux: session_name]"
        self.update_window_title_with_tmux();

        // Check for automatic profile switching based on tmux session name
        self.apply_tmux_session_profile(session_name);

        // Update the gateway tab's title to show tmux session
        if let Some(gateway_tab_id) = self.tmux_state.tmux_gateway_tab_id
            && let Some(tab) = self.tab_manager.get_tab_mut(gateway_tab_id)
        {
            tab.set_title(&format!("[tmux: {}]", session_name));
            crate::debug_info!(
                "TMUX",
                "Updated gateway tab {} title to '[tmux: {}]'",
                gateway_tab_id,
                session_name
            );
        }

        // Enable sync now that session is connected
        self.tmux_state.tmux_sync.enable();

        // Note: tmux_gateway_active was already set on the gateway tab during initiate_tmux_gateway()

        // Set window-size to 'smallest' so tmux respects par-term's size
        // even when other (larger) clients are attached.
        // This is critical for proper multi-client behavior.
        let _ = self.write_to_gateway("set-option -g window-size smallest\n");
        crate::debug_info!(
            "TMUX",
            "Set window-size to smallest for multi-client support"
        );

        // Tell tmux the terminal size so panes can be properly sized
        // Without this, tmux uses a very small default and splits will fail
        self.send_tmux_client_size();

        // Note: Initial pane content comes from layout-change handling which sends Ctrl+L
        // to each pane. We don't send Enter here as it would execute a command.

        // Show success toast
        self.show_toast(format!("tmux: Connected to session '{}'", session_name));
    }

    /// Send the terminal size to tmux so it knows the client dimensions
    ///
    /// In control mode, tmux doesn't know the terminal size unless we tell it.
    /// Without this, tmux uses a very small default and pane splits will fail
    /// with "no space for new pane".
    pub(super) fn send_tmux_client_size(&self) {
        // Get the terminal grid size from the renderer
        if let Some(renderer) = &self.renderer {
            let (cols, rows) = renderer.grid_size();
            let cmd = crate::tmux::TmuxCommand::set_client_size(cols, rows);
            let cmd_str = format!("{}\n", cmd.as_str());

            if self.write_to_gateway(&cmd_str) {
                crate::debug_trace!("TMUX", "Sent client size to tmux: {}x{}", cols, rows);
            } else {
                crate::debug_error!("TMUX", "Failed to send client size to tmux");
            }
        } else {
            crate::debug_error!("TMUX", "Cannot send client size - no renderer available");
        }
    }

    /// Notify tmux of a window/pane resize
    ///
    /// Called when the window is resized to keep tmux in sync with par-term's size.
    /// This sends `refresh-client -C cols,rows` to tmux in gateway mode.
    pub fn notify_tmux_of_resize(&self) {
        // Only send if tmux gateway is active
        if !self.is_gateway_active() {
            return;
        }

        self.send_tmux_client_size();
    }

    /// Update window title with tmux session info
    /// Format: "window_title - [tmux: session_name]"
    pub(crate) fn update_window_title_with_tmux(&self) {
        let title = if let Some(session_name) = &self.tmux_state.tmux_session_name {
            format!("{} - [tmux: {}]", self.config.window_title, session_name)
        } else {
            self.config.window_title.clone()
        };
        let formatted = self.format_title(&title);
        self.with_window(|w| w.set_title(&formatted));
    }

    /// Handle session renamed notification
    pub(super) fn handle_tmux_session_renamed(&mut self, session_name: &str) {
        crate::debug_info!("TMUX", "Session renamed to: {}", session_name);

        // Update stored session name
        self.tmux_state.tmux_session_name = Some(session_name.to_string());

        // Update window title with new session name
        self.update_window_title_with_tmux();
    }

    /// Handle session ended notification
    pub(super) fn handle_tmux_session_ended(&mut self) {
        crate::debug_info!("TMUX", "Session ended");

        // Collect tmux display tabs to close (tabs with tmux_pane_id set, excluding gateway)
        let gateway_tab_id = self.tmux_state.tmux_gateway_tab_id;
        let tmux_tabs_to_close: Vec<crate::tab::TabId> = self
            .tab_manager
            .tabs()
            .iter()
            .filter_map(|tab| {
                // Close tabs that were displaying tmux content (have tmux_pane_id)
                // but not the gateway tab itself
                if tab.tmux_pane_id.is_some() && Some(tab.id) != gateway_tab_id {
                    Some(tab.id)
                } else {
                    None
                }
            })
            .collect();

        // Close tmux display tabs
        for tab_id in tmux_tabs_to_close {
            crate::debug_info!("TMUX", "Closing tmux display tab {}", tab_id);
            let _ = self.tab_manager.close_tab(tab_id);
        }

        // Disable tmux control mode on the gateway tab and clear auto-applied profile
        if let Some(gateway_tab_id) = self.tmux_state.tmux_gateway_tab_id
            && let Some(tab) = self.tab_manager.get_tab_mut(gateway_tab_id)
            && tab.tmux_gateway_active
        {
            tab.tmux_gateway_active = false;
            tab.tmux_pane_id = None;
            tab.clear_auto_profile(); // Clear tmux session profile
            // try_lock: intentional — session-ended cleanup runs from the sync event loop
            // where blocking would stall the entire GUI.
            // On miss: set the deferred flag so the notification poll loop retries on the
            // next frame, guaranteeing the terminal parser eventually exits control mode.
            if let Ok(term) = tab.terminal.try_write() {
                term.set_tmux_control_mode(false);
            } else {
                crate::debug_error!(
                    "TAB",
                    "session-ended: could not acquire terminal lock to disable tmux control mode \
                     on tab {} — deferring to next poll cycle",
                    gateway_tab_id
                );
                tab.pending_tmux_mode_disable = true;
            }
        }
        self.tmux_state.tmux_gateway_tab_id = None;

        // Clean up tmux session state
        if let Some(mut session) = self.tmux_state.tmux_session.take() {
            session.disconnect();
        }
        self.tmux_state.tmux_session_name = None;

        // Clear pane mappings
        self.tmux_state.tmux_pane_to_native_pane.clear();
        self.tmux_state.native_pane_to_tmux_pane.clear();

        // Reset window title (now without tmux info)
        self.update_window_title_with_tmux();

        // Clear sync state
        self.tmux_state.tmux_sync = crate::tmux::TmuxSync::new();

        // Show toast
        self.show_toast("tmux: Session ended");
    }
}

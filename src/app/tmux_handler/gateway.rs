//! tmux gateway session management and I/O routing.
//!
//! Covers:
//! - Session lifecycle: initiate, attach, disconnect, status queries
//! - Input routing: send_input_via_tmux, paste_via_tmux, prefix key handling
//! - Pane operations: split_pane_via_tmux, close_pane_via_tmux
//! - Clipboard + resize synchronization
//! - Profile auto-application on session connect

use crate::app::window_state::WindowState;
use crate::tmux::{SessionState, TmuxSession};

impl WindowState {
    // =========================================================================
    // Gateway Mode Session Management
    // =========================================================================

    /// Initiate a new tmux session via gateway mode.
    ///
    /// This writes `tmux -CC new-session` to the active tab's PTY and enables
    /// tmux control mode parsing. The session will be fully connected once we
    /// receive the `%session-changed` notification.
    ///
    /// # Arguments
    /// * `session_name` - Optional session name. If None, tmux will auto-generate one.
    pub fn initiate_tmux_gateway(&mut self, session_name: Option<&str>) -> anyhow::Result<()> {
        if !self.config.tmux_enabled {
            anyhow::bail!("tmux integration is disabled");
        }

        if self.tmux_state.tmux_session.is_some() && self.is_tmux_connected() {
            anyhow::bail!("Already connected to a tmux session");
        }

        crate::debug_info!(
            "TMUX",
            "Initiating gateway mode session: {:?}",
            session_name.unwrap_or("(auto)")
        );

        // Generate the command
        let cmd = match session_name {
            Some(name) => TmuxSession::create_or_attach_command(name),
            None => TmuxSession::create_new_command(None),
        };

        // Get the active tab ID and write the command to its PTY
        let gateway_tab_id = self
            .tab_manager
            .active_tab_id()
            .ok_or_else(|| anyhow::anyhow!("No active tab available for tmux gateway"))?;

        let tab = self
            .tab_manager
            .active_tab_mut()
            .ok_or_else(|| anyhow::anyhow!("No active tab available for tmux gateway"))?;

        // Write the command to the PTY
        // try_lock: intentional — initiate_tmux_gateway is user-initiated but called from
        // the sync event loop context. If the terminal is locked by the async PTY reader
        // the command cannot be sent. On miss: bails with an error so the caller can retry.
        if let Ok(term) = tab.terminal.try_write() {
            crate::debug_info!(
                "TMUX",
                "Writing gateway command to tab {}: {}",
                gateway_tab_id,
                cmd.trim()
            );
            term.write(cmd.as_bytes())?;
            // Enable tmux control mode parsing AFTER writing the command
            term.set_tmux_control_mode(true);
            crate::debug_info!(
                "TMUX",
                "Enabled tmux control mode parsing on tab {}",
                gateway_tab_id
            );
        } else {
            anyhow::bail!("Could not acquire terminal lock");
        }

        // Mark this tab as the gateway
        tab.tmux_gateway_active = true;

        // Store the gateway tab ID so we know where to send commands
        self.tmux_state.tmux_gateway_tab_id = Some(gateway_tab_id);
        crate::debug_info!(
            "TMUX",
            "Gateway tab set to {}, state: Initiating",
            gateway_tab_id
        );

        // Create session and set gateway state
        let mut session = TmuxSession::new();
        session.set_gateway_initiating();
        self.tmux_state.tmux_session = Some(session);

        // Show toast
        self.show_toast("tmux: Connecting...");

        Ok(())
    }

    /// Attach to an existing tmux session via gateway mode.
    ///
    /// This writes `tmux -CC attach -t session` to the active tab's PTY.
    pub fn attach_tmux_gateway(&mut self, session_name: &str) -> anyhow::Result<()> {
        if !self.config.tmux_enabled {
            anyhow::bail!("tmux integration is disabled");
        }

        if self.tmux_state.tmux_session.is_some() && self.is_tmux_connected() {
            anyhow::bail!("Already connected to a tmux session");
        }

        crate::debug_info!("TMUX", "Attaching to session via gateway: {}", session_name);

        // Generate the attach command
        let cmd = TmuxSession::create_attach_command(session_name);

        // Get the active tab ID and write the command to its PTY
        let gateway_tab_id = self
            .tab_manager
            .active_tab_id()
            .ok_or_else(|| anyhow::anyhow!("No active tab available for tmux gateway"))?;

        let tab = self
            .tab_manager
            .active_tab_mut()
            .ok_or_else(|| anyhow::anyhow!("No active tab available for tmux gateway"))?;

        // Write the command to the PTY
        // try_lock: intentional — same rationale as initiate_tmux_gateway. On miss: bails
        // so the user can retry the attach operation explicitly.
        if let Ok(term) = tab.terminal.try_write() {
            crate::debug_info!(
                "TMUX",
                "Writing attach command to tab {}: {}",
                gateway_tab_id,
                cmd.trim()
            );
            term.write(cmd.as_bytes())?;
            term.set_tmux_control_mode(true);
            crate::debug_info!(
                "TMUX",
                "Enabled tmux control mode parsing on tab {}",
                gateway_tab_id
            );
        } else {
            anyhow::bail!("Could not acquire terminal lock");
        }

        // Mark this tab as the gateway
        tab.tmux_gateway_active = true;

        // Store the gateway tab ID so we know where to send commands
        self.tmux_state.tmux_gateway_tab_id = Some(gateway_tab_id);
        crate::debug_info!(
            "TMUX",
            "Gateway tab set to {}, state: Initiating",
            gateway_tab_id
        );

        // Create session and set gateway state
        let mut session = TmuxSession::new();
        session.set_gateway_initiating();
        self.tmux_state.tmux_session = Some(session);

        // Show toast
        self.show_toast(format!("tmux: Attaching to '{}'...", session_name));

        Ok(())
    }

    /// Disconnect from the current tmux session
    pub fn disconnect_tmux_session(&mut self) {
        // Clear the gateway tab ID
        self.tmux_state.tmux_gateway_tab_id = None;

        // First, disable tmux control mode on any gateway tabs
        for tab in self.tab_manager.tabs_mut() {
            if tab.tmux_gateway_active {
                tab.tmux_gateway_active = false;
                // try_lock: intentional — disconnect is called from the sync event loop.
                // On miss: control mode stays on the terminal until the next frame; benign
                // since the session is already being torn down and no further output arrives.
                if let Ok(term) = tab.terminal.try_write() {
                    term.set_tmux_control_mode(false);
                }
            }
        }

        if let Some(mut session) = self.tmux_state.tmux_session.take() {
            crate::debug_info!("TMUX", "Disconnecting from tmux session");
            session.disconnect();
        }

        // Clear session name
        self.tmux_state.tmux_session_name = None;

        // Reset sync state
        self.tmux_state.tmux_sync = crate::tmux::TmuxSync::new();

        // Reset window title (now without tmux info)
        self.update_window_title_with_tmux();
    }

    /// Check if tmux session is active
    pub fn is_tmux_connected(&self) -> bool {
        self.tmux_state
            .tmux_session
            .as_ref()
            .is_some_and(|s| s.state() == SessionState::Connected)
    }

    /// Check if gateway mode is active (connected or connecting)
    pub fn is_gateway_active(&self) -> bool {
        self.tmux_state
            .tmux_session
            .as_ref()
            .is_some_and(|s| s.is_gateway_active())
    }

    /// Update the tmux focused pane when a native pane is focused
    ///
    /// This should be called when the user clicks on a pane to ensure
    /// input is routed to the correct tmux pane.
    pub fn set_tmux_focused_pane_from_native(&mut self, native_pane_id: crate::pane::PaneId) {
        if let Some(tmux_pane_id) = self
            .tmux_state
            .native_pane_to_tmux_pane
            .get(&native_pane_id)
            && let Some(session) = &mut self.tmux_state.tmux_session
        {
            crate::debug_info!(
                "TMUX",
                "Setting focused pane: native {} -> tmux %{}",
                native_pane_id,
                tmux_pane_id
            );
            session.set_focused_pane(Some(*tmux_pane_id));
        }
    }

    // =========================================================================
    // Gateway Mode Input Routing
    // =========================================================================

    /// Write a command to the gateway tab's terminal.
    ///
    /// The gateway tab is where the tmux control mode connection lives.
    /// All tmux commands must be written to this tab, not the active tab.
    pub(crate) fn write_to_gateway(&self, cmd: &str) -> bool {
        let gateway_tab_id = match self.tmux_state.tmux_gateway_tab_id {
            Some(id) => id,
            None => {
                crate::debug_trace!("TMUX", "No gateway tab ID set");
                return false;
            }
        };

        // try_lock: intentional — write_to_gateway is called from the sync event loop and
        // from input handlers. Blocking would stall the GUI or create deadlock risk.
        // On miss: the tmux command is silently dropped. For input this means a keypress
        // is lost; for control commands (resize, split) the caller should retry as needed.
        if let Some(tab) = self.tab_manager.get_tab(gateway_tab_id)
            && tab.tmux_gateway_active
            && let Ok(term) = tab.terminal.try_write()
            && term.write(cmd.as_bytes()).is_ok()
        {
            return true;
        }

        crate::debug_trace!("TMUX", "Failed to write to gateway tab");
        false
    }

    /// Split the current pane via tmux control mode.
    ///
    /// Writes split-window command to the gateway PTY.
    ///
    /// # Arguments
    /// * `vertical` - true for vertical split (side by side), false for horizontal (stacked)
    ///
    /// Returns true if the command was sent successfully.
    pub fn split_pane_via_tmux(&self, vertical: bool) -> bool {
        if !self.config.tmux_enabled || !self.is_tmux_connected() {
            return false;
        }

        let session = match &self.tmux_state.tmux_session {
            Some(s) => s,
            None => return false,
        };

        // Get the focused pane ID
        let pane_id = session.focused_pane();

        // Format the split command
        let cmd = if vertical {
            match pane_id {
                Some(id) => format!("split-window -h -t %{}\n", id),
                None => "split-window -h\n".to_string(),
            }
        } else {
            match pane_id {
                Some(id) => format!("split-window -v -t %{}\n", id),
                None => "split-window -v\n".to_string(),
            }
        };

        // Write to gateway tab
        if self.write_to_gateway(&cmd) {
            crate::debug_info!(
                "TMUX",
                "Sent {} split command via gateway",
                if vertical { "vertical" } else { "horizontal" }
            );
            return true;
        }

        false
    }

    /// Close the focused pane via tmux control mode.
    ///
    /// Writes kill-pane command to the gateway PTY.
    ///
    /// Returns true if the command was sent successfully.
    pub fn close_pane_via_tmux(&self) -> bool {
        if !self.config.tmux_enabled || !self.is_tmux_connected() {
            return false;
        }

        let session = match &self.tmux_state.tmux_session {
            Some(s) => s,
            None => return false,
        };

        // Get the focused pane ID
        let pane_id = match session.focused_pane() {
            Some(id) => id,
            None => {
                crate::debug_info!("TMUX", "No focused pane to close");
                return false;
            }
        };

        let cmd = format!("kill-pane -t %{}\n", pane_id);

        // Write to gateway tab
        if self.write_to_gateway(&cmd) {
            crate::debug_info!("TMUX", "Sent kill-pane command for pane %{}", pane_id);
            return true;
        }

        false
    }

    /// Sync clipboard content to tmux paste buffer.
    ///
    /// Writes set-buffer command to the gateway PTY.
    ///
    /// Returns true if the command was sent successfully.
    pub fn sync_clipboard_to_tmux(&self, content: &str) -> bool {
        // Check if clipboard sync is enabled
        if !self.config.tmux_clipboard_sync {
            return false;
        }

        if !self.config.tmux_enabled || !self.is_tmux_connected() {
            return false;
        }

        // Don't sync empty content
        if content.is_empty() {
            return false;
        }

        // Format the set-buffer command
        let escaped = content.replace('\'', "'\\''");
        let cmd = format!("set-buffer '{}'\n", escaped);

        // Write to gateway tab
        if self.write_to_gateway(&cmd) {
            crate::debug_trace!(
                "TMUX",
                "Synced {} chars to tmux paste buffer",
                content.len()
            );
            return true;
        }

        false
    }

    // =========================================================================
    // Pane Resize Sync
    // =========================================================================

    /// Sync pane resize to tmux after a divider drag.
    ///
    /// When the user resizes panes by dragging a divider in par-term, this
    /// sends the new pane sizes to tmux so external clients see the same layout.
    ///
    /// # Arguments
    /// * `is_horizontal_divider` - true if dragging a horizontal divider (changes heights),
    ///   false if dragging a vertical divider (changes widths)
    pub fn sync_pane_resize_to_tmux(&self, is_horizontal_divider: bool) {
        // Only sync if tmux gateway is active
        if !self.is_gateway_active() {
            return;
        }

        // Get cell dimensions from renderer
        let (cell_width, cell_height) = match &self.renderer {
            Some(r) => (r.cell_width(), r.cell_height()),
            None => return,
        };

        // Get pane sizes from active tab's pane manager
        let pane_sizes: Vec<(crate::tmux::TmuxPaneId, usize, usize)> = if let Some(tab) =
            self.tab_manager.active_tab()
            && let Some(pm) = tab.pane_manager()
        {
            pm.all_panes()
                .iter()
                .filter_map(|pane| {
                    // Get the tmux pane ID for this native pane
                    let tmux_pane_id = self.tmux_state.native_pane_to_tmux_pane.get(&pane.id)?;
                    // Calculate size in columns/rows
                    let cols = (pane.bounds.width / cell_width).floor() as usize;
                    let rows = (pane.bounds.height / cell_height).floor() as usize;
                    Some((*tmux_pane_id, cols.max(1), rows.max(1)))
                })
                .collect()
        } else {
            return;
        };

        // Send resize commands for each pane, but only for the dimension that changed
        // Horizontal divider: changes height (rows) - use -y
        // Vertical divider: changes width (cols) - use -x
        for (tmux_pane_id, cols, rows) in pane_sizes {
            let cmd = if is_horizontal_divider {
                format!("resize-pane -t %{} -y {}\n", tmux_pane_id, rows)
            } else {
                format!("resize-pane -t %{} -x {}\n", tmux_pane_id, cols)
            };
            if self.write_to_gateway(&cmd) {
                crate::debug_info!(
                    "TMUX",
                    "Synced pane %{} {} resize to {}",
                    tmux_pane_id,
                    if is_horizontal_divider {
                        "height"
                    } else {
                        "width"
                    },
                    if is_horizontal_divider { rows } else { cols }
                );
            }
        }
    }

    // =========================================================================
    // Prefix Key Handling
    // =========================================================================

    // =========================================================================
    // Profile Auto-Switching
    // =========================================================================

    /// Apply a profile based on tmux session name
    ///
    /// This checks for profiles that match the session name pattern and applies
    /// them to the gateway tab. Profile matching uses glob patterns (e.g., "work-*",
    /// "*-production").
    pub(crate) fn apply_tmux_session_profile(&mut self, session_name: &str) {
        // First, check if there's a fixed tmux_profile configured
        if let Some(ref profile_name) = self.config.tmux_profile {
            if let Some(profile) = self.overlay_ui.profile_manager.find_by_name(profile_name) {
                let profile_id = profile.id;
                let profile_display = profile.name.clone();
                crate::debug_info!(
                    "TMUX",
                    "Applying configured tmux_profile '{}' for session '{}'",
                    profile_display,
                    session_name
                );
                self.apply_profile_to_gateway_tab(profile_id, &profile_display);
                return;
            } else {
                crate::debug_info!(
                    "TMUX",
                    "Configured tmux_profile '{}' not found",
                    profile_name
                );
            }
        }

        // Then, check for pattern-based matching
        if let Some(profile) = self
            .overlay_ui
            .profile_manager
            .find_by_tmux_session(session_name)
        {
            let profile_id = profile.id;
            let profile_display = profile.name.clone();
            crate::debug_info!(
                "TMUX",
                "Auto-switching to profile '{}' for tmux session '{}'",
                profile_display,
                session_name
            );
            self.apply_profile_to_gateway_tab(profile_id, &profile_display);
        } else {
            crate::debug_info!(
                "TMUX",
                "No profile matches tmux session '{}' - consider adding tmux_session_patterns to a profile",
                session_name
            );
        }
    }

    /// Apply a profile to the tmux gateway tab
    fn apply_profile_to_gateway_tab(
        &mut self,
        profile_id: crate::profile::ProfileId,
        profile_name: &str,
    ) {
        // Extract profile settings before borrowing tab_manager
        let profile_settings = self.overlay_ui.profile_manager.get(&profile_id).map(|p| {
            (
                p.tab_name.clone(),
                p.icon.clone(),
                p.badge_text.clone(),
                p.command.clone(),
                p.command_args.clone(),
            )
        });

        if let Some(gateway_tab_id) = self.tmux_state.tmux_gateway_tab_id
            && let Some(tab) = self.tab_manager.get_tab_mut(gateway_tab_id)
        {
            // Mark the auto-applied profile
            tab.auto_applied_profile_id = Some(profile_id);

            if let Some((tab_name, icon, badge_text, command, command_args)) = profile_settings {
                // Apply profile icon
                tab.profile_icon = icon;

                // Save original title before overriding (only if not already saved)
                if tab.pre_profile_title.is_none() {
                    tab.pre_profile_title = Some(tab.title.clone());
                }
                // Apply profile tab name (fall back to profile name)
                tab.title = tab_name.unwrap_or_else(|| profile_name.to_string());

                // Apply badge text override if configured
                if let Some(badge_text) = badge_text {
                    tab.badge_override = Some(badge_text.clone());
                    crate::debug_info!(
                        "TMUX",
                        "Applied badge text '{}' from profile '{}'",
                        badge_text,
                        profile_name
                    );
                }

                // Execute profile command in the running shell if configured
                if let Some(cmd) = command {
                    let mut full_cmd = cmd;
                    if let Some(args) = command_args {
                        for arg in args {
                            full_cmd.push(' ');
                            full_cmd.push_str(&arg);
                        }
                    }
                    full_cmd.push('\n');

                    let terminal_clone = std::sync::Arc::clone(&tab.terminal);
                    self.runtime.spawn(async move {
                        let term = terminal_clone.write().await;
                        if let Err(e) = term.write(full_cmd.as_bytes()) {
                            log::error!("Failed to execute tmux profile command: {}", e);
                        }
                    });
                }
            }

            // Show notification about profile switch
            self.show_toast(format!("tmux: Profile '{}' applied", profile_name));
            log::info!(
                "Applied profile '{}' for tmux session (gateway tab {})",
                profile_name,
                gateway_tab_id
            );
        }

        // Apply profile badge settings (color, font, margins, etc.)
        if let Some(profile) = self.overlay_ui.profile_manager.get(&profile_id) {
            let profile_clone = profile.clone();
            self.apply_profile_badge(&profile_clone);
        }
    }
}

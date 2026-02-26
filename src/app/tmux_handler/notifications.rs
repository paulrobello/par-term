//! tmux notification handling — control-mode event routing.
//!
//! Handles all events received from the tmux control-mode parser:
//! session-changed, window-add/close/rename, layout-change, output,
//! pane-focus, session-ended, pause/continue, and sync-action dispatch.
//!
//! ## Gateway Mode
//!
//! Gateway mode writes `tmux -CC` commands to the existing terminal's PTY
//! instead of spawning a separate process. This is the iTerm2 approach and
//! provides reliable tmux integration.
//!
//! The flow is:
//! 1. User selects "Create Session" in picker
//! 2. We write `tmux -CC new-session -s name\n` to the active tab's PTY
//! 3. Enable tmux control mode parsing in the terminal
//! 4. Receive notifications via `%session-changed`, `%output`, etc.
//! 5. Route input via `send-keys` commands back to the same PTY

use crate::app::window_state::WindowState;
use crate::tmux::{
    ParserBridge, SyncAction, TmuxLayout, TmuxNotification, TmuxWindowId,
};

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
        let _session = match &self.tmux_session {
            Some(s) if s.is_gateway_active() => s,
            _ => return false,
        };

        // Get the gateway tab ID - this is where the tmux control connection lives
        let gateway_tab_id = match self.tmux_gateway_tab_id {
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
            if let Some(session) = &mut self.tmux_session
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

    /// Handle session started notification
    fn handle_tmux_session_started(&mut self, session_name: &str) {
        crate::debug_info!("TMUX", "Session started: {}", session_name);

        // Store the session name for later use (e.g., window title updates)
        self.tmux_session_name = Some(session_name.to_string());

        // Update window title with session name: "par-term - [tmux: session_name]"
        self.update_window_title_with_tmux();

        // Check for automatic profile switching based on tmux session name
        self.apply_tmux_session_profile(session_name);

        // Update the gateway tab's title to show tmux session
        if let Some(gateway_tab_id) = self.tmux_gateway_tab_id
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
        self.tmux_sync.enable();

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
    fn send_tmux_client_size(&self) {
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

    /// Request content refresh for specific panes
    ///
    /// After learning about panes from a layout change, we need to trigger
    /// each pane to send its content. tmux only sends %output for NEW content,
    /// not existing screen content when attaching.
    ///
    /// We use two approaches:
    /// 1. Send Ctrl+L (C-l) to each pane, which triggers shell screen redraw
    /// 2. Use capture-pane -p to get the current pane content (comes as command response)
    fn request_pane_refresh(&self, pane_ids: &[crate::tmux::TmuxPaneId]) {
        for pane_id in pane_ids {
            // Approach 1: Send Ctrl+L (screen redraw signal) to trigger shell to repaint
            // This works for interactive shells that respond to SIGWINCH-like events
            let cmd = format!("send-keys -t %{} C-l\n", pane_id);
            if self.write_to_gateway(&cmd) {
                crate::debug_trace!("TMUX", "Sent C-l to pane %{} for refresh", pane_id);
            }
        }

        // Request client refresh which may help with layout sync
        let refresh_cmd = "refresh-client\n";
        if self.write_to_gateway(refresh_cmd) {
            crate::debug_info!(
                "TMUX",
                "Requested client refresh for {} panes",
                pane_ids.len()
            );
        }
    }

    /// Update window title with tmux session info
    /// Format: "window_title - [tmux: session_name]"
    pub(crate) fn update_window_title_with_tmux(&self) {
        if let Some(window) = &self.window {
            let title = if let Some(session_name) = &self.tmux_session_name {
                format!("{} - [tmux: {}]", self.config.window_title, session_name)
            } else {
                self.config.window_title.clone()
            };
            window.set_title(&self.format_title(&title));
        }
    }

    /// Handle session renamed notification
    fn handle_tmux_session_renamed(&mut self, session_name: &str) {
        crate::debug_info!("TMUX", "Session renamed to: {}", session_name);

        // Update stored session name
        self.tmux_session_name = Some(session_name.to_string());

        // Update window title with new session name
        self.update_window_title_with_tmux();
    }

    /// Handle window add notification - creates a new tab
    fn handle_tmux_window_add(&mut self, window_id: TmuxWindowId) {
        crate::debug_info!("TMUX", "Window added: @{}", window_id);

        // Check max tabs limit
        if self.config.max_tabs > 0 && self.tab_manager.tab_count() >= self.config.max_tabs {
            crate::debug_error!(
                "TMUX",
                "Cannot create tab for tmux window @{}: max_tabs limit ({}) reached",
                window_id,
                self.config.max_tabs
            );
            return;
        }

        // Get current grid size from renderer
        let grid_size = self.renderer.as_ref().map(|r| r.grid_size());

        // Create a new tab for this tmux window
        match self.tab_manager.new_tab(
            &self.config,
            std::sync::Arc::clone(&self.runtime),
            false, // Don't inherit CWD from active tab for tmux
            grid_size,
        ) {
            Ok(tab_id) => {
                crate::debug_info!(
                    "TMUX",
                    "Created tab {} for tmux window @{}",
                    tab_id,
                    window_id
                );

                // Register the mapping
                self.tmux_sync.map_window(window_id, tab_id);

                // Set initial title based on tmux window ID
                // Note: These tabs are for displaying tmux windows, but the gateway tab
                // is where the actual tmux control connection lives. We store the tmux pane ID
                // on the tab so we know which pane to route input to.
                if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
                    tab.set_title(&format!("tmux @{}", window_id));
                    // Note: Don't set tmux_gateway_active here - only the gateway tab is the gateway

                    // Start refresh task for the new tab
                    if let Some(window) = &self.window {
                        tab.start_refresh_task(
                            std::sync::Arc::clone(&self.runtime),
                            std::sync::Arc::clone(window),
                            self.config.max_fps,
                            self.config.inactive_tab_fps,
                        );
                    }

                    // Resize terminal to match current renderer dimensions
                    // try_lock: intentional — called during window-add handling in the sync event
                    // loop. On miss: the new tmux tab's terminal is not resized this frame; the
                    // size will be corrected on the next Resized event.
                    if let Some(renderer) = &self.renderer
                        && let Ok(mut term) = tab.terminal.try_lock()
                    {
                        let (cols, rows) = renderer.grid_size();
                        let size = renderer.size();
                        let width_px = size.width as usize;
                        let height_px = size.height as usize;

                        term.set_cell_dimensions(
                            renderer.cell_width() as u32,
                            renderer.cell_height() as u32,
                        );
                        let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
                        crate::debug_info!(
                            "TMUX",
                            "Resized tmux tab {} terminal to {}x{}",
                            tab_id,
                            cols,
                            rows
                        );
                    }
                }
            }
            Err(e) => {
                crate::debug_error!(
                    "TMUX",
                    "Failed to create tab for tmux window @{}: {}",
                    window_id,
                    e
                );
            }
        }
    }

    /// Handle window close notification - closes corresponding tab
    fn handle_tmux_window_close(&mut self, window_id: TmuxWindowId) {
        crate::debug_info!("TMUX", "Window closed: @{}", window_id);

        // Find the corresponding tab
        if let Some(tab_id) = self.tmux_sync.get_tab(window_id) {
            crate::debug_info!(
                "TMUX",
                "Closing tab {} for tmux window @{}",
                tab_id,
                window_id
            );

            // Close the tab
            let was_last = self.tab_manager.close_tab(tab_id);

            // Remove the mapping
            self.tmux_sync.unmap_window(window_id);

            if was_last {
                // Last tab closed - trigger session end handling
                crate::debug_info!("TMUX", "Last tmux window closed, session ending");
                self.handle_tmux_session_ended();
            }
        } else {
            crate::debug_info!(
                "TMUX",
                "No tab mapping found for tmux window @{} (may have been created before attach)",
                window_id
            );
        }
    }

    /// Handle window renamed notification
    fn handle_tmux_window_renamed(&mut self, window_id: TmuxWindowId, name: &str) {
        crate::debug_info!("TMUX", "Window @{} renamed to: {}", window_id, name);

        // Find the corresponding tab and update its title
        if let Some(tab_id) = self.tmux_sync.get_tab(window_id) {
            if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
                tab.set_title(name);
                crate::debug_info!("TMUX", "Updated tab {} title to '{}'", tab_id, name);
            }
        } else {
            crate::debug_info!(
                "TMUX",
                "No tab mapping found for tmux window @{} rename",
                window_id
            );
        }
    }

    /// Handle layout change notification - updates pane arrangement
    fn handle_tmux_layout_change(&mut self, window_id: TmuxWindowId, layout_str: &str) {
        crate::debug_info!(
            "TMUX",
            "Layout changed for window @{}: {}",
            window_id,
            layout_str
        );

        // Parse the layout string
        let parsed_layout = match TmuxLayout::parse(layout_str) {
            Some(layout) => layout,
            None => {
                crate::debug_error!(
                    "TMUX",
                    "Failed to parse layout string for window @{}: {}",
                    window_id,
                    layout_str
                );
                return;
            }
        };

        // Log the parsed layout structure
        let pane_ids = parsed_layout.pane_ids();
        crate::debug_info!(
            "TMUX",
            "Parsed layout for window @{}: {} panes (IDs: {:?})",
            window_id,
            pane_ids.len(),
            pane_ids
        );

        // Log the layout structure for debugging
        Self::log_layout_node(&parsed_layout.root, 0);

        // Update focused pane in session if we have one
        if !pane_ids.is_empty()
            && let Some(session) = &mut self.tmux_session
        {
            // Default to first pane if no focused pane set
            if session.focused_pane().is_none() {
                session.set_focused_pane(Some(pane_ids[0]));
            }
        }

        // Find the corresponding tab and create window mapping if needed
        let tab_id = if let Some(id) = self.tmux_sync.get_tab(window_id) {
            Some(id)
        } else {
            // No window mapping exists - try to find a tab that has one of our panes
            // This happens when we connect to an existing session and receive layout before window-add
            let mut found_tab_id = None;
            for pane_id in &pane_ids {
                // Check if any tab has this tmux_pane_id set
                for tab in self.tab_manager.tabs() {
                    if tab.tmux_pane_id == Some(*pane_id) {
                        found_tab_id = Some(tab.id);
                        crate::debug_info!(
                            "TMUX",
                            "Found existing tab {} with pane %{} for window @{}",
                            tab.id,
                            pane_id,
                            window_id
                        );
                        break;
                    }
                }
                if found_tab_id.is_some() {
                    break;
                }
            }

            // If we found a tab, create the window mapping
            if let Some(tid) = found_tab_id {
                self.tmux_sync.map_window(window_id, tid);
                crate::debug_info!(
                    "TMUX",
                    "Created window mapping: @{} -> tab {}",
                    window_id,
                    tid
                );
            }

            found_tab_id
        };

        // Get bounds info from renderer for proper pane sizing (needed for both paths)
        // Calculate status bar height for proper content area
        let is_tmux_connected = self.is_tmux_connected();
        let status_bar_height =
            crate::tmux_status_bar_ui::TmuxStatusBarUI::height(&self.config, is_tmux_connected);
        let custom_status_bar_height = self.status_bar_ui.height(&self.config, self.is_fullscreen);

        let bounds_info = self.renderer.as_ref().map(|r| {
            let size = r.size();
            let padding = r.window_padding();
            let content_offset_y = r.content_offset_y();
            let content_inset_right = r.content_inset_right();
            let cell_width = r.cell_width();
            let cell_height = r.cell_height();
            // Scale status_bar_height from logical to physical pixels
            let physical_status_bar_height =
                (status_bar_height + custom_status_bar_height) * r.scale_factor();
            (
                size,
                padding,
                content_offset_y,
                content_inset_right,
                cell_width,
                cell_height,
                physical_status_bar_height,
            )
        });

        if let Some(tab_id) = tab_id {
            crate::debug_info!(
                "TMUX",
                "Layout change for window @{} on tab {} - {} panes: {:?}",
                window_id,
                tab_id,
                pane_ids.len(),
                pane_ids
            );

            // Apply the tmux layout to native pane rendering
            if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
                // Initialize pane manager if needed
                tab.init_pane_manager();

                // Set pane bounds before applying layout
                if let Some((
                    size,
                    padding,
                    content_offset_y,
                    content_inset_right,
                    _cell_width,
                    _cell_height,
                    status_bar_height,
                )) = bounds_info
                    && let Some(pm) = tab.pane_manager_mut()
                {
                    // Tmux layouts always have multiple panes; hide window padding if configured
                    let effective_padding = if self.config.hide_window_padding_on_split {
                        0.0
                    } else {
                        padding
                    };
                    let content_width =
                        size.width as f32 - effective_padding * 2.0 - content_inset_right;
                    let content_height = size.height as f32
                        - content_offset_y
                        - effective_padding
                        - status_bar_height;
                    let bounds = crate::pane::PaneBounds::new(
                        effective_padding,
                        content_offset_y,
                        content_width,
                        content_height,
                    );
                    pm.set_bounds(bounds);
                    crate::debug_info!(
                        "TMUX",
                        "Set pane manager bounds: {}x{} at ({}, {})",
                        content_width,
                        content_height,
                        effective_padding,
                        content_offset_y
                    );
                }

                // Check if we already have mappings for these exact tmux pane IDs
                // If so, we should preserve the existing native panes/terminals
                let existing_tmux_ids: std::collections::HashSet<_> =
                    self.tmux_pane_to_native_pane.keys().copied().collect();
                let new_tmux_ids: std::collections::HashSet<_> = pane_ids.iter().copied().collect();

                if existing_tmux_ids == new_tmux_ids && !existing_tmux_ids.is_empty() {
                    // Same panes - preserve terminals but update layout structure
                    crate::debug_info!(
                        "TMUX",
                        "Layout change with same panes ({:?}) - preserving terminals, updating layout",
                        pane_ids
                    );

                    // Update the pane tree structure from the new layout without recreating terminals
                    if let Some(pm) = tab.pane_manager_mut() {
                        // Update layout structure (ratios, positions) from tmux layout
                        pm.update_layout_from_tmux(&parsed_layout, &self.tmux_pane_to_native_pane);
                        pm.recalculate_bounds();

                        // Resize terminals to match new bounds
                        // No padding in tmux mode - tmux controls the layout
                        if let Some((_, _, _, _, cell_width, cell_height, _)) = bounds_info {
                            pm.resize_all_terminals(cell_width, cell_height);
                        }
                    }

                    self.needs_redraw = true;
                    return; // Early return - don't recreate panes
                }

                // Check if new panes are a SUBSET of existing (panes were closed)
                // or if there's overlap (some panes closed, some remain)
                let panes_to_keep: std::collections::HashSet<_> = existing_tmux_ids
                    .intersection(&new_tmux_ids)
                    .copied()
                    .collect();
                let panes_to_remove: Vec<_> = existing_tmux_ids
                    .difference(&new_tmux_ids)
                    .copied()
                    .collect();
                let panes_to_add: Vec<_> = new_tmux_ids
                    .difference(&existing_tmux_ids)
                    .copied()
                    .collect();

                // If we have panes to keep and panes to remove, handle incrementally
                if !panes_to_keep.is_empty()
                    && !panes_to_remove.is_empty()
                    && panes_to_add.is_empty()
                {
                    crate::debug_info!(
                        "TMUX",
                        "Layout change: keeping {:?}, removing {:?}",
                        panes_to_keep,
                        panes_to_remove
                    );

                    // Check if any of the removed panes was the focused pane
                    let current_focused = self.tmux_session.as_ref().and_then(|s| s.focused_pane());
                    let focused_pane_removed = current_focused
                        .map(|fp| panes_to_remove.contains(&fp))
                        .unwrap_or(false);

                    // Remove the closed panes from our native pane tree
                    if let Some(pm) = tab.pane_manager_mut() {
                        for tmux_pane_id in &panes_to_remove {
                            if let Some(native_pane_id) =
                                self.tmux_pane_to_native_pane.get(tmux_pane_id)
                            {
                                crate::debug_info!(
                                    "TMUX",
                                    "Removing native pane {} for closed tmux pane %{}",
                                    native_pane_id,
                                    tmux_pane_id
                                );
                                pm.close_pane(*native_pane_id);
                            }
                        }

                        // Update layout structure for remaining panes
                        // Build new mappings with only the kept panes
                        let kept_mappings: std::collections::HashMap<_, _> = self
                            .tmux_pane_to_native_pane
                            .iter()
                            .filter(|(tmux_id, _)| panes_to_keep.contains(tmux_id))
                            .map(|(k, v)| (*k, *v))
                            .collect();

                        pm.update_layout_from_tmux(&parsed_layout, &kept_mappings);
                        pm.recalculate_bounds();

                        // Resize terminals to match new bounds
                        if let Some((_, _, _, _, cell_width, cell_height, _)) = bounds_info {
                            pm.resize_all_terminals(cell_width, cell_height);
                        }
                    }

                    // Update mappings - remove closed panes
                    for tmux_pane_id in &panes_to_remove {
                        if let Some(native_id) = self.tmux_pane_to_native_pane.remove(tmux_pane_id)
                        {
                            self.native_pane_to_tmux_pane.remove(&native_id);
                        }
                    }

                    // If the focused pane was removed, update tmux session focus to first remaining pane
                    if focused_pane_removed
                        && let Some(new_focus) = panes_to_keep.iter().next().copied()
                    {
                        crate::debug_info!(
                            "TMUX",
                            "Focused pane was removed, updating tmux session focus to %{}",
                            new_focus
                        );
                        if let Some(session) = &mut self.tmux_session {
                            session.set_focused_pane(Some(new_focus));
                        }
                    }

                    crate::debug_info!(
                        "TMUX",
                        "After pane removal, mappings: {:?}",
                        self.tmux_pane_to_native_pane
                    );

                    self.needs_redraw = true;
                    self.request_redraw();
                    return; // Early return - don't recreate remaining panes
                }

                // Handle case where panes are ADDED (split) while keeping existing ones
                if !panes_to_keep.is_empty()
                    && !panes_to_add.is_empty()
                    && panes_to_remove.is_empty()
                {
                    crate::debug_info!(
                        "TMUX",
                        "Layout change: keeping {:?}, adding {:?}",
                        panes_to_keep,
                        panes_to_add
                    );

                    // Rebuild the entire tree structure from the tmux layout
                    // This preserves existing pane terminals while creating correct structure
                    if let Some(pm) = tab.pane_manager_mut() {
                        // Create a mapping of tmux pane IDs to keep -> their native IDs
                        let existing_mappings: std::collections::HashMap<_, _> = panes_to_keep
                            .iter()
                            .filter_map(|tmux_id| {
                                self.tmux_pane_to_native_pane
                                    .get(tmux_id)
                                    .map(|native_id| (*tmux_id, *native_id))
                            })
                            .collect();

                        match pm.rebuild_from_tmux_layout(
                            &parsed_layout,
                            &existing_mappings,
                            &panes_to_add,
                            &self.config,
                            std::sync::Arc::clone(&self.runtime),
                        ) {
                            Ok(new_mappings) => {
                                // Update our mappings with the new ones
                                self.tmux_pane_to_native_pane = new_mappings.clone();
                                self.native_pane_to_tmux_pane = new_mappings
                                    .iter()
                                    .map(|(tmux_id, native_id)| (*native_id, *tmux_id))
                                    .collect();

                                crate::debug_info!(
                                    "TMUX",
                                    "Rebuilt layout with {} panes: {:?}",
                                    new_mappings.len(),
                                    new_mappings
                                );

                                // Resize terminals to match new bounds
                                if let Some((_, _, _, _, cell_width, cell_height, _)) = bounds_info
                                {
                                    pm.resize_all_terminals(cell_width, cell_height);
                                }
                            }
                            Err(e) => {
                                crate::debug_error!("TMUX", "Failed to rebuild layout: {}", e);
                            }
                        }
                    }

                    // Request content for the new panes only
                    self.request_pane_refresh(&panes_to_add);

                    crate::debug_info!(
                        "TMUX",
                        "After pane addition, mappings: {:?}",
                        self.tmux_pane_to_native_pane
                    );

                    self.needs_redraw = true;
                    self.request_redraw();
                    return; // Early return - don't recreate all panes
                }

                // Full layout recreation needed (complete replacement or complex changes)
                if let Some(pm) = tab.pane_manager_mut() {
                    crate::debug_info!(
                        "TMUX",
                        "Full layout recreation: existing={:?}, new={:?}",
                        existing_tmux_ids,
                        new_tmux_ids
                    );

                    match pm.set_from_tmux_layout(
                        &parsed_layout,
                        &self.config,
                        std::sync::Arc::clone(&self.runtime),
                    ) {
                        Ok(pane_mappings) => {
                            // Store the pane mappings for output routing
                            crate::debug_info!(
                                "TMUX",
                                "Storing pane mappings: {:?}",
                                pane_mappings
                            );
                            // Store both forward and reverse mappings
                            self.tmux_pane_to_native_pane = pane_mappings.clone();
                            self.native_pane_to_tmux_pane = pane_mappings
                                .iter()
                                .map(|(tmux_id, native_id)| (*native_id, *tmux_id))
                                .collect();

                            crate::debug_info!(
                                "TMUX",
                                "Applied tmux layout to tab {}: {} pane mappings created",
                                tab_id,
                                pane_mappings.len()
                            );

                            // Set tab's tmux_pane_id to first pane for legacy output routing
                            if !pane_ids.is_empty() && tab.tmux_pane_id.is_none() {
                                tab.tmux_pane_id = Some(pane_ids[0]);
                            }

                            // Request content refresh for all panes
                            // tmux doesn't send existing content on attach
                            self.request_pane_refresh(&pane_ids);

                            self.needs_redraw = true;
                        }
                        Err(e) => {
                            crate::debug_error!(
                                "TMUX",
                                "Failed to apply tmux layout to tab {}: {}",
                                tab_id,
                                e
                            );
                            // Fall back to legacy routing
                            if !pane_ids.is_empty() && tab.tmux_pane_id.is_none() {
                                tab.tmux_pane_id = Some(pane_ids[0]);
                            }
                        }
                    }
                } else {
                    // No pane manager - use legacy routing
                    if !pane_ids.is_empty() && tab.tmux_pane_id.is_none() {
                        tab.tmux_pane_id = Some(pane_ids[0]);
                        crate::debug_info!(
                            "TMUX",
                            "Set tab {} tmux_pane_id to %{} for output routing (no pane manager)",
                            tab_id,
                            pane_ids[0]
                        );
                    }
                }
            }
        } else {
            // No tab mapping found - create a new tab for this tmux window
            crate::debug_info!(
                "TMUX",
                "No tab mapping for window @{}, creating new tab for layout",
                window_id
            );

            // Create a new tab for this tmux window
            if self.config.max_tabs == 0 || self.tab_manager.tab_count() < self.config.max_tabs {
                let grid_size = self.renderer.as_ref().map(|r| r.grid_size());
                match self.tab_manager.new_tab(
                    &self.config,
                    std::sync::Arc::clone(&self.runtime),
                    false,
                    grid_size,
                ) {
                    Ok(new_tab_id) => {
                        crate::debug_info!(
                            "TMUX",
                            "Created tab {} for tmux window @{}",
                            new_tab_id,
                            window_id
                        );

                        // Register the window mapping
                        self.tmux_sync.map_window(window_id, new_tab_id);

                        // Now apply the layout to this tab
                        if let Some(tab) = self.tab_manager.get_tab_mut(new_tab_id) {
                            tab.init_pane_manager();
                            tab.set_title(&format!("tmux @{}", window_id));

                            // Start refresh task
                            if let Some(window) = &self.window {
                                tab.start_refresh_task(
                                    std::sync::Arc::clone(&self.runtime),
                                    std::sync::Arc::clone(window),
                                    self.config.max_fps,
                                    self.config.inactive_tab_fps,
                                );
                            }

                            // Set pane bounds
                            if let Some((
                                size,
                                padding,
                                content_offset_y,
                                content_inset_right,
                                _cell_width,
                                _cell_height,
                                status_bar_height,
                            )) = bounds_info
                                && let Some(pm) = tab.pane_manager_mut()
                            {
                                let content_width =
                                    size.width as f32 - padding * 2.0 - content_inset_right;
                                let content_height = size.height as f32
                                    - content_offset_y
                                    - padding
                                    - status_bar_height;
                                let bounds = crate::pane::PaneBounds::new(
                                    padding,
                                    content_offset_y,
                                    content_width,
                                    content_height,
                                );
                                pm.set_bounds(bounds);
                            }

                            // Apply the tmux layout
                            if let Some(pm) = tab.pane_manager_mut() {
                                match pm.set_from_tmux_layout(
                                    &parsed_layout,
                                    &self.config,
                                    std::sync::Arc::clone(&self.runtime),
                                ) {
                                    Ok(pane_mappings) => {
                                        crate::debug_info!(
                                            "TMUX",
                                            "Storing pane mappings for new tab: {:?}",
                                            pane_mappings
                                        );
                                        // Store both forward and reverse mappings
                                        self.native_pane_to_tmux_pane = pane_mappings
                                            .iter()
                                            .map(|(tmux_id, native_id)| (*native_id, *tmux_id))
                                            .collect();
                                        self.tmux_pane_to_native_pane = pane_mappings;

                                        // Set tab's tmux_pane_id to first pane
                                        if !pane_ids.is_empty() {
                                            tab.tmux_pane_id = Some(pane_ids[0]);
                                        }

                                        // Request content refresh for all panes
                                        self.request_pane_refresh(&pane_ids);

                                        self.needs_redraw = true;
                                    }
                                    Err(e) => {
                                        crate::debug_error!(
                                            "TMUX",
                                            "Failed to apply tmux layout to new tab: {}",
                                            e
                                        );
                                    }
                                }
                            }
                        }

                        // Switch to the new tab
                        self.tab_manager.switch_to(new_tab_id);
                    }
                    Err(e) => {
                        crate::debug_error!(
                            "TMUX",
                            "Failed to create tab for tmux window @{}: {}",
                            window_id,
                            e
                        );
                    }
                }
            }
        }
    }

    /// Log a layout node and its children recursively for debugging
    fn log_layout_node(node: &crate::tmux::LayoutNode, depth: usize) {
        let indent = "  ".repeat(depth);
        match node {
            crate::tmux::LayoutNode::Pane {
                id,
                width,
                height,
                x,
                y,
            } => {
                crate::debug_trace!(
                    "TMUX",
                    "{}Pane %{}: {}x{} at ({}, {})",
                    indent,
                    id,
                    width,
                    height,
                    x,
                    y
                );
            }
            crate::tmux::LayoutNode::VerticalSplit {
                width,
                height,
                x,
                y,
                children,
            } => {
                crate::debug_trace!(
                    "TMUX",
                    "{}VerticalSplit: {}x{} at ({}, {}) with {} children",
                    indent,
                    width,
                    height,
                    x,
                    y,
                    children.len()
                );
                for child in children {
                    Self::log_layout_node(child, depth + 1);
                }
            }
            crate::tmux::LayoutNode::HorizontalSplit {
                width,
                height,
                x,
                y,
                children,
            } => {
                crate::debug_trace!(
                    "TMUX",
                    "{}HorizontalSplit: {}x{} at ({}, {}) with {} children",
                    indent,
                    width,
                    height,
                    x,
                    y,
                    children.len()
                );
                for child in children {
                    Self::log_layout_node(child, depth + 1);
                }
            }
        }
    }

    /// Handle pane output notification - routes to correct terminal
    fn handle_tmux_output(&mut self, pane_id: crate::tmux::TmuxPaneId, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        crate::debug_trace!(
            "TMUX",
            "Output from pane %{}: {} bytes",
            pane_id,
            data.len()
        );

        // Log first few bytes for debugging space issue
        if data.len() <= 20 {
            crate::debug_trace!(
                "TMUX",
                "Output data: {:?} (hex: {:02x?})",
                String::from_utf8_lossy(data),
                data
            );
        }

        // Check if output is paused - buffer if so
        if self.tmux_sync.buffer_output(pane_id, data) {
            crate::debug_trace!(
                "TMUX",
                "Buffered {} bytes for pane %{} (paused)",
                data.len(),
                pane_id
            );
            return;
        }

        // Debug: log the current mapping state
        crate::debug_trace!("TMUX", "Pane mappings: {:?}", self.tmux_pane_to_native_pane);

        // First, try to find a native pane mapping (for split panes)
        // Check our direct mapping first, then fall back to tmux_sync
        let native_pane_id = self
            .tmux_pane_to_native_pane
            .get(&pane_id)
            .copied()
            .or_else(|| self.tmux_sync.get_native_pane(pane_id));

        if let Some(native_pane_id) = native_pane_id {
            // Find the pane across all tabs and route output to it
            for tab in self.tab_manager.tabs_mut() {
                // try_lock: intentional — output routing is called from the sync event loop.
                // On miss: this chunk of tmux output is dropped for this pane. Acceptable
                // because tmux re-sends content via pane refresh (Ctrl+L) on next connect.
                if let Some(pane_manager) = tab.pane_manager_mut()
                    && let Some(pane) = pane_manager.get_pane_mut(native_pane_id)
                    && let Ok(term) = pane.terminal.try_lock()
                {
                    // Route the data to this pane's terminal
                    term.process_data(data);
                    crate::debug_trace!(
                        "TMUX",
                        "Routed {} bytes to pane {} (tmux %{})",
                        data.len(),
                        native_pane_id,
                        pane_id
                    );
                    return;
                }
            }
        }

        // No native pane mapping - check for tab-level tmux pane mapping
        // (This is used when we create tabs for tmux panes without split pane manager)
        for tab in self.tab_manager.tabs_mut() {
            // try_lock: intentional — output routing from the sync event loop; must not block.
            // On miss: this tmux output chunk is dropped for this tab. Low risk as tmux
            // provides its own backpressure (%pause / %continue protocol).
            if tab.tmux_pane_id == Some(pane_id)
                && let Ok(term) = tab.terminal.try_lock()
            {
                term.process_data(data);
                crate::debug_trace!(
                    "TMUX",
                    "Routed {} bytes to tab terminal (tmux %{})",
                    data.len(),
                    pane_id
                );
                return;
            }
        }

        // No direct mapping for this pane - try to find an existing tmux tab to route to
        // This handles the case where tmux has multiple panes but we don't have native
        // split pane rendering yet. Route all output to the first tmux-connected tab.
        crate::debug_trace!(
            "TMUX",
            "No direct mapping for tmux pane %{}, looking for existing tmux tab",
            pane_id
        );

        // First, try to find any tab with a tmux_pane_id set (existing tmux display)
        for tab in self.tab_manager.tabs_mut() {
            // try_lock: intentional — fallback output routing in the sync event loop.
            // On miss: this chunk of pane output is dropped. Acceptable for the same
            // reason as above — tmux backpressure and pane refresh handle recovery.
            if tab.tmux_pane_id.is_some()
                && !tab.tmux_gateway_active
                && let Ok(term) = tab.terminal.try_lock()
            {
                term.process_data(data);
                crate::debug_trace!(
                    "TMUX",
                    "Routed {} bytes from pane %{} to existing tmux tab (pane %{:?})",
                    data.len(),
                    pane_id,
                    tab.tmux_pane_id
                );
                return;
            }
        }

        // No existing tmux tab found - create one
        crate::debug_info!(
            "TMUX",
            "No existing tmux tab found, creating new tab for pane %{}",
            pane_id
        );

        // Don't route to the gateway tab - that shows raw protocol
        // Instead, create a new tab for this tmux pane
        if self.tmux_gateway_tab_id.is_some() {
            // Check if we can create a new tab
            if self.config.max_tabs == 0 || self.tab_manager.tab_count() < self.config.max_tabs {
                let grid_size = self.renderer.as_ref().map(|r| r.grid_size());
                match self.tab_manager.new_tab(
                    &self.config,
                    std::sync::Arc::clone(&self.runtime),
                    false,
                    grid_size,
                ) {
                    Ok(new_tab_id) => {
                        crate::debug_info!(
                            "TMUX",
                            "Created tab {} for tmux pane %{}",
                            new_tab_id,
                            pane_id
                        );

                        // Set the focused pane if not already set
                        if let Some(session) = &mut self.tmux_session
                            && session.focused_pane().is_none()
                        {
                            session.set_focused_pane(Some(pane_id));
                        }

                        // Configure the new tab for this tmux pane
                        if let Some(tab) = self.tab_manager.get_tab_mut(new_tab_id) {
                            // Associate this tab with the tmux pane
                            tab.tmux_pane_id = Some(pane_id);
                            tab.set_title(&format!("tmux %{}", pane_id));

                            // Start refresh task
                            if let Some(window) = &self.window {
                                tab.start_refresh_task(
                                    std::sync::Arc::clone(&self.runtime),
                                    std::sync::Arc::clone(window),
                                    self.config.max_fps,
                                    self.config.inactive_tab_fps,
                                );
                            }

                            // Route the data to the new tab's terminal
                            // try_lock: intentional — the tab was just created so contention is
                            // extremely unlikely. On miss: the very first chunk of pane output
                            // is dropped; subsequent output arrives normally.
                            if let Ok(term) = tab.terminal.try_lock() {
                                term.process_data(data);
                            }
                        }

                        // Switch to the new tab (away from gateway tab)
                        self.tab_manager.switch_to(new_tab_id);
                    }
                    Err(e) => {
                        crate::debug_error!(
                            "TMUX",
                            "Failed to create tab for tmux pane %{}: {}",
                            pane_id,
                            e
                        );
                    }
                }
            } else {
                crate::debug_error!(
                    "TMUX",
                    "Cannot create tab for tmux pane %{}: max tabs reached",
                    pane_id
                );
            }
        }
    }

    /// Handle pane focus changed notification from external tmux
    fn handle_tmux_pane_focus_changed(&mut self, tmux_pane_id: crate::tmux::TmuxPaneId) {
        crate::debug_info!("TMUX", "Pane focus changed to %{}", tmux_pane_id);

        // Update the tmux session's focused pane
        if let Some(session) = &mut self.tmux_session {
            session.set_focused_pane(Some(tmux_pane_id));
        }

        // Update the native pane focus to match
        if let Some(native_pane_id) = self.tmux_pane_to_native_pane.get(&tmux_pane_id) {
            // Find the tab containing this pane and update its focus
            if let Some(tab) = self.tab_manager.active_tab_mut()
                && let Some(pm) = tab.pane_manager_mut()
            {
                pm.focus_pane(*native_pane_id);
                crate::debug_info!(
                    "TMUX",
                    "Updated native pane focus: tmux %{} -> native {}",
                    tmux_pane_id,
                    native_pane_id
                );
            }
        }
    }

    /// Handle session ended notification
    fn handle_tmux_session_ended(&mut self) {
        crate::debug_info!("TMUX", "Session ended");

        // Collect tmux display tabs to close (tabs with tmux_pane_id set, excluding gateway)
        let gateway_tab_id = self.tmux_gateway_tab_id;
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
        if let Some(gateway_tab_id) = self.tmux_gateway_tab_id
            && let Some(tab) = self.tab_manager.get_tab_mut(gateway_tab_id)
            && tab.tmux_gateway_active
        {
            tab.tmux_gateway_active = false;
            tab.tmux_pane_id = None;
            tab.clear_auto_profile(); // Clear tmux session profile
            // try_lock: intentional — session-ended cleanup runs from the sync event loop.
            // On miss: tmux control mode stays enabled on the terminal temporarily; the
            // terminal will stop receiving control-mode notifications since the session is
            // disconnected and the flag will be cleared on the next unlock opportunity.
            // FIXME: If lock is never acquired, the terminal remains in tmux control mode
            // indefinitely. Consider a deferred cleanup mechanism if this proves problematic.
            if let Ok(term) = tab.terminal.try_lock() {
                term.set_tmux_control_mode(false);
            }
        }
        self.tmux_gateway_tab_id = None;

        // Clean up tmux session state
        if let Some(mut session) = self.tmux_session.take() {
            session.disconnect();
        }
        self.tmux_session_name = None;

        // Clear pane mappings
        self.tmux_pane_to_native_pane.clear();
        self.native_pane_to_tmux_pane.clear();

        // Reset window title (now without tmux info)
        self.update_window_title_with_tmux();

        // Clear sync state
        self.tmux_sync = crate::tmux::TmuxSync::new();

        // Show toast
        self.show_toast("tmux: Session ended");
    }

    /// Handle error notification
    fn handle_tmux_error(&mut self, msg: &str) {
        crate::debug_error!("TMUX", "Error from tmux: {}", msg);

        // Show notification to user
        self.deliver_notification("tmux Error", msg);
    }

    /// Handle pause notification (for slow connections)
    fn handle_tmux_pause(&mut self) {
        crate::debug_info!("TMUX", "Received pause notification - buffering output");

        // Set paused state in sync manager
        self.tmux_sync.pause();

        // Show toast notification to user
        self.show_toast("tmux: Output paused (slow connection)");
    }

    /// Handle continue notification (resume after pause)
    fn handle_tmux_continue(&mut self) {
        crate::debug_info!("TMUX", "Received continue notification - resuming output");

        // Get and flush buffered output
        let buffered = self.tmux_sync.resume();

        // Flush buffered data to each pane
        for (tmux_pane_id, data) in buffered {
            if !data.is_empty() {
                crate::debug_info!(
                    "TMUX",
                    "Flushing {} buffered bytes to pane %{}",
                    data.len(),
                    tmux_pane_id
                );

                // Find the native pane and send the buffered data
                if let Some(native_pane_id) = self.tmux_sync.get_native_pane(tmux_pane_id) {
                    // Find the pane across all tabs
                    for tab in self.tab_manager.tabs_mut() {
                        if let Some(pane_manager) = tab.pane_manager_mut()
                            && let Some(pane) = pane_manager.get_pane_mut(native_pane_id)
                        {
                            // try_lock: intentional — flushing buffered output in the sync
                            // event loop. On miss: this buffered chunk is lost. Low risk:
                            // the tmux %continue state means output has resumed and fresh
                            // data will arrive shortly to fill in any gap.
                            if let Ok(term) = pane.terminal.try_lock() {
                                term.process_data(&data);
                            }
                            break;
                        }
                    }
                }
            }
        }

        // Show toast notification to user
        self.show_toast("tmux: Output resumed");
    }

    /// Process sync actions generated by TmuxSync
    /// TODO: Wire this up when TmuxSync is fully integrated
    #[allow(dead_code)] // Planned for TmuxSync integration
    fn process_sync_actions(&mut self, actions: Vec<SyncAction>) {
        for action in actions {
            match action {
                SyncAction::CreateTab { window_id } => {
                    crate::debug_info!("TMUX", "Sync: Create tab for window @{}", window_id);
                }
                SyncAction::CloseTab { tab_id } => {
                    crate::debug_info!("TMUX", "Sync: Close tab {}", tab_id);
                }
                SyncAction::RenameTab { tab_id, name } => {
                    crate::debug_info!("TMUX", "Sync: Rename tab {} to '{}'", tab_id, name);
                }
                SyncAction::UpdateLayout { tab_id, layout: _ } => {
                    crate::debug_info!("TMUX", "Sync: Update layout for tab {}", tab_id);
                }
                SyncAction::PaneOutput { pane_id, data } => {
                    crate::debug_trace!(
                        "TMUX",
                        "Sync: Route {} bytes to pane {}",
                        data.len(),
                        pane_id
                    );
                }
                SyncAction::SessionEnded => {
                    crate::debug_info!("TMUX", "Sync: Session ended");
                    self.handle_tmux_session_ended();
                }
                SyncAction::Pause => {
                    crate::debug_info!("TMUX", "Sync: Pause");
                    self.handle_tmux_pause();
                }
                SyncAction::Continue => {
                    crate::debug_info!("TMUX", "Sync: Continue");
                    self.handle_tmux_continue();
                }
            }
        }
    }
}

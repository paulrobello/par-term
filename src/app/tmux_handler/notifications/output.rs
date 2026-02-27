//! Tmux output routing — `handle_tmux_output`.
//!
//! Routes `%output` notifications from the tmux control-mode session to the
//! correct native pane terminal. Routing priority:
//! 1. Direct native-pane mapping (split-pane rendering).
//! 2. Tab-level tmux pane ID (legacy single-pane per tab).
//! 3. Any existing tmux tab (fallback).
//! 4. Create a new tab on-demand.

use crate::app::window_state::WindowState;

impl WindowState {
    /// Handle pane output notification - routes to correct terminal
    pub(super) fn handle_tmux_output(&mut self, pane_id: crate::tmux::TmuxPaneId, data: &[u8]) {
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
        if self.tmux_state.tmux_sync.buffer_output(pane_id, data) {
            crate::debug_trace!(
                "TMUX",
                "Buffered {} bytes for pane %{} (paused)",
                data.len(),
                pane_id
            );
            return;
        }

        // Debug: log the current mapping state
        crate::debug_trace!(
            "TMUX",
            "Pane mappings: {:?}",
            self.tmux_state.tmux_pane_to_native_pane
        );

        // First, try to find a native pane mapping (for split panes)
        // Check our direct mapping first, then fall back to tmux_sync
        let native_pane_id = self
            .tmux_state
            .tmux_pane_to_native_pane
            .get(&pane_id)
            .copied()
            .or_else(|| self.tmux_state.tmux_sync.get_native_pane(pane_id));

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
        if self.tmux_state.tmux_gateway_tab_id.is_some() {
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
                        if let Some(session) = &mut self.tmux_state.tmux_session
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
}

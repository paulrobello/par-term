//! Tmux output routing — `handle_tmux_output`.
//!
//! Routes `%output` notifications from the tmux control-mode session to the
//! correct native pane terminal. Routing priority:
//! 1. Direct native-pane mapping (split-pane rendering).
//! 2. Tab-level tmux pane ID (legacy single-pane per tab).
//! 3. Any existing tmux tab (fallback).
//! 4. Create a new tab on-demand.

use crate::app::window_state::WindowState;

/// Per-pane cap on lock-miss output backlog (`mux_output_backlog`): past
/// this, the oldest bytes are dropped so the newest repaint data survives.
/// The daemon never re-sends pane output, so this is a pressure valve, not
/// a correctness mechanism.
#[cfg(feature = "mux")]
const MAX_OUTPUT_BACKLOG_BYTES: usize = 1024 * 1024;

impl WindowState {
    /// Buffer an output chunk whose terminal lock was contended. Mux only:
    /// the par-mux daemon never re-sends pane output, so a dropped chunk is
    /// a permanent hole (corrupted repaints, truncated OSC 52). Plain tmux
    /// keeps its drop discipline — tmux re-sends content via pane refresh.
    #[cfg(feature = "mux")]
    fn buffer_output_backlog(&mut self, pane_id: crate::tmux::TmuxPaneId, data: &[u8]) {
        let entry = self
            .tmux_state
            .mux_output_backlog
            .entry(pane_id)
            .or_default();
        entry.extend_from_slice(data);
        let overflow = entry.len().saturating_sub(MAX_OUTPUT_BACKLOG_BYTES);
        if overflow > 0 {
            crate::debug_info!(
                "TMUX",
                "Output backlog for pane %{} over cap, dropping {} oldest bytes",
                pane_id,
                overflow
            );
            entry.drain(0..overflow);
        }
    }

    /// Replay output buffered while the pane's terminal lock was contended.
    /// Called on the pane's next `%output` before the live chunk routes, so
    /// stream order is preserved; under plain tmux this is a no-op.
    #[cfg(feature = "mux")]
    fn replay_output_backlog(&mut self, pane_id: crate::tmux::TmuxPaneId) {
        let Some(buffered) = self.tmux_state.mux_output_backlog.remove(&pane_id) else {
            return;
        };
        if !self.route_tmux_output_to_existing(pane_id, &buffered) {
            // Still unmapped: restore and wait for the mapping (the live
            // chunk's flow below may create the tab).
            self.tmux_state.mux_output_backlog.insert(pane_id, buffered);
        }
    }

    /// Routes through the four existing-mapping paths. Returns `true` when
    /// the chunk was delivered or (mux) buffered behind a contended lock;
    /// `false` only when the pane has no mapping anywhere, which sends the
    /// caller to the create-tab path. Plain tmux keeps the tmux-era drop
    /// discipline on lock misses (falls through as it always did).
    fn route_tmux_output_to_existing(
        &mut self,
        pane_id: crate::tmux::TmuxPaneId,
        data: &[u8],
    ) -> bool {
        // Direct per-tab mapping (split panes). Native pane ids restart at
        // 1 in every tab, so the pane resolves inside its owning tab — an
        // unscoped scan could land this output in another window's pane.
        if let Some((owner_tab_id, native_pane_id)) = self.tmux_state.tmux_pane_owner(pane_id) {
            // Seed-before-live causality (the blank/partial mux render): a
            // pending reattach seed is OLDER than this chunk, so it must
            // land first — cursor-addressed TUI updates arriving after
            // delivery land on the seeded baseline instead of atop a stale
            // full-screen paste over them. No-op without the mux feature.
            #[cfg(feature = "mux")]
            self.deliver_pending_mux_seed(pane_id);
            // try_lock: intentional — output routing is called from the sync
            // event loop. On a lock miss, mux buffers for replay (the daemon
            // never re-sends); plain tmux falls through to the next path.
            if let Some(tab) = self.tab_manager.get_tab_mut(owner_tab_id)
                && let Some(pane_manager) = tab.pane_manager_mut()
                && let Some(pane) = pane_manager.get_pane_mut(native_pane_id)
                && let Ok(term) = pane.terminal.try_read()
            {
                term.process_mux_output(data);
                crate::debug_trace!(
                    "TMUX",
                    "Routed {} bytes to pane {} (tmux %{})",
                    data.len(),
                    native_pane_id,
                    pane_id
                );
                return true;
            }
            #[cfg(feature = "mux")]
            {
                self.buffer_output_backlog(pane_id, data);
                return true;
            }
        }

        // Legacy single-pane fallback: the sync map carries no owning tab,
        // so its panes resolve by scanning.
        if let Some(native_pane_id) = self.tmux_state.tmux_sync.get_native_pane(pane_id) {
            for tab in self.tab_manager.tabs_mut() {
                if let Some(pane_manager) = tab.pane_manager_mut()
                    && let Some(pane) = pane_manager.get_pane_mut(native_pane_id)
                    && let Ok(term) = pane.terminal.try_read()
                {
                    term.process_mux_output(data);
                    crate::debug_trace!(
                        "TMUX",
                        "Routed {} bytes to pane {} (tmux %{}, sync fallback)",
                        data.len(),
                        native_pane_id,
                        pane_id
                    );
                    return true;
                }
            }
            #[cfg(feature = "mux")]
            {
                self.buffer_output_backlog(pane_id, data);
                return true;
            }
        }

        // No native pane mapping - check for tab-level tmux pane mapping
        // (This is used when we create tabs for tmux panes without split pane manager)
        for tab in self.tab_manager.tabs_mut() {
            // try_lock: intentional — output routing from the sync event loop; must not block.
            if tab.tmux.tmux_pane_id == Some(pane_id)
                && let Ok(term) = tab.terminal.try_read()
            {
                term.process_mux_output(data);
                crate::debug_trace!(
                    "TMUX",
                    "Routed {} bytes to tab terminal (tmux %{})",
                    data.len(),
                    pane_id
                );
                return true;
            }
        }

        // First, try to find any tab with a tmux_pane_id set (existing tmux display)
        for tab in self.tab_manager.tabs_mut() {
            // try_lock: intentional — fallback output routing in the sync event loop.
            if tab.tmux.tmux_pane_id.is_some()
                && !tab.tmux.tmux_gateway_active
                && let Ok(term) = tab.terminal.try_read()
            {
                term.process_mux_output(data);
                crate::debug_trace!(
                    "TMUX",
                    "Routed {} bytes from pane %{} to existing tmux tab (pane %{:?})",
                    data.len(),
                    pane_id,
                    tab.tmux.tmux_pane_id
                );
                return true;
            }
        }

        false
    }

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

        // A screen seed requested off-loop is still in flight: it clears
        // the mirror and paints a snapshot, so output applied now would be
        // erased when it lands. Hold it; the seed replays it afterwards.
        #[cfg(feature = "mux")]
        if self.hold_output_behind_seed(pane_id, data) {
            return;
        }

        // Mux only: replay output buffered while the pane's terminal lock
        // was contended, before the pause check — backlog bytes are older
        // than anything buffered during a pause, so this keeps stream order.
        #[cfg(feature = "mux")]
        self.replay_output_backlog(pane_id);

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
            self.tmux_state.tmux_pane_owners
        );

        if self.route_tmux_output_to_existing(pane_id, data) {
            return;
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
            if self.config.load().tabs.max_tabs == 0
                || self.tab_manager.tab_count() < self.config.load().tabs.max_tabs
            {
                let grid_size = self.renderer.as_ref().map(|r| r.grid_size());
                match self.tab_manager.new_tab(
                    &self.config.load(),
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
                            tab.tmux.tmux_pane_id = Some(pane_id);
                            // UX.md T8: the placeholder title must not say
                            // "tmux" when the pane belongs to the daemon.
                            let prefix = if self.tmux_state.is_mux_attached() {
                                "par-mux"
                            } else {
                                "tmux"
                            };
                            tab.set_title(&format!("{} %{}", prefix, pane_id));

                            // Start refresh task
                            if let Some(window) = &self.window {
                                tab.start_refresh_task(
                                    std::sync::Arc::clone(&self.runtime),
                                    std::sync::Arc::clone(window),
                                    self.config.load().rendering.max_fps,
                                    self.config.load().power.inactive_tab_fps,
                                );
                            }

                            // Route the data to the new tab's terminal
                            // try_lock: intentional — the tab was just created so contention is
                            // extremely unlikely. On a lock miss, mux buffers the very first
                            // chunk for replay (the daemon never re-sends); plain tmux keeps
                            // its drop discipline (tmux re-sends via pane refresh).
                            if let Ok(term) = tab.terminal.try_read() {
                                term.process_mux_output(data);
                            } else {
                                #[cfg(feature = "mux")]
                                self.buffer_output_backlog(pane_id, data);
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

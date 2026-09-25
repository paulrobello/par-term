//! par-mux notification drain: pulls the daemon transport, partitions
//! agent-roster pushes out for the roster cache, dispatches the rest through
//! the same grouped `TmuxSync` path as the tmux gateway, and delivers pending
//! reattach screen seeds. Split from `mux.rs`, which owns the transport and
//! the attach/detach wiring.

use crate::app::window_state::WindowState;
use crate::tmux::{ParserBridge, TmuxNotification};
use par_term_mux::AgentEntry;
use par_term_tmux::TmuxPaneId;

impl WindowState {
    /// Apply `%agent-state-changed` pushes to the roster cache. Returns
    /// whether any push landed (a roster surface may need to re-render).
    ///
    /// Extracted from [`Self::check_mux_notifications`] so the manners rule
    /// (REPORT.md E3: an agent needing attention joins a list and the
    /// indicator glows — it never steals focus) is testable without a live
    /// daemon. Everything this does beyond `AgentRoster::apply_push` is
    /// return the redraw flag; introducing a focus call, window raise,
    /// notification, or palette open here is the interruption E3 forbids,
    /// and the test below is the alarm.
    pub(super) fn apply_agent_pushes(
        &mut self,
        pushes: Vec<par_term_emu_core_rust::tmux_control::TmuxNotification>,
    ) -> bool {
        let mut needs_redraw = false;
        for push in pushes {
            if let par_term_emu_core_rust::tmux_control::TmuxNotification::AgentStateChanged {
                pane_id,
                agent,
                state,
                source,
            } = push
            {
                match AgentEntry::from_push(&pane_id, &agent, &state, &source) {
                    Some(entry) => {
                        // Explicit field reads (reason included) keep the
                        // entry honest in the log while the wire cannot
                        // carry a reason yet.
                        crate::debug_info!(
                            "MUX",
                            "agent roster push: %{} {} {} source={:?} reason={:?}",
                            entry.pane,
                            entry.agent,
                            entry.state,
                            entry.source,
                            entry.reason
                        );
                        self.tmux_state.agent_roster.apply_push(entry);
                        // Roster surfaces (A2b tasks 2/3) render from this
                        // cache, so a push is a potential visual change.
                        needs_redraw = true;
                    }
                    None => crate::debug_log!(
                        "MUX",
                        "dropped unattributed agent push: {pane_id} {agent} {state}"
                    ),
                }
            }
        }
        needs_redraw
    }

    /// Apply `%agent-released` pushes to the roster cache. Same manners
    /// rule as [`Self::apply_agent_pushes`]: a release may set the redraw
    /// flag (a roster row disappearing) and nothing else.
    pub(super) fn apply_agent_releases(
        &mut self,
        releases: Vec<par_term_emu_core_rust::tmux_control::TmuxNotification>,
    ) -> bool {
        let mut needs_redraw = false;
        for release in releases {
            if let par_term_emu_core_rust::tmux_control::TmuxNotification::AgentReleased {
                pane_id,
                agent,
            } = release
                && let Some(pane) = pane_id.strip_prefix('%').and_then(|id| id.parse().ok())
            {
                crate::debug_info!("MUX", "agent release push: %{pane} {agent}");
                self.tmux_state.agent_roster.apply_release(pane, &agent);
                needs_redraw = true;
            }
        }
        needs_redraw
    }

    /// Apply `%pane-title-changed` pushes: the daemon emits these ONLY for
    /// user `-T` titles (set from par-term's Rename Pane, a shell via
    /// `par-mux -c`, or another client), so a push always carries user
    /// semantics — non-empty re-marks the pane user-named, empty (the
    /// clear operation) reverts it to automatic titles. A pane with no
    /// native mapping yet waits in `mux_pane_titles` for the layout
    /// consumer, the same pending-seed pattern as reattach screens.
    pub(super) fn apply_pane_title_pushes(
        &mut self,
        pushes: Vec<par_term_emu_core_rust::tmux_control::TmuxNotification>,
    ) -> bool {
        let mut needs_redraw = false;
        for push in pushes {
            if let par_term_emu_core_rust::tmux_control::TmuxNotification::PaneTitleChanged {
                pane_id,
                title,
            } = push
                && let Some(pane) = pane_id.strip_prefix('%').and_then(|id| id.parse().ok())
            {
                crate::debug_info!(
                    "MUX",
                    "pane title push: %{pane} {:?} (user semantics)",
                    title
                );
                self.tmux_state.mux_pane_titles.insert(pane, (title, true));
                self.apply_pending_mux_pane_titles();
                needs_redraw = true;
            }
        }
        needs_redraw
    }

    /// Drain the par-mux transport and dispatch through the same grouped
    /// consumer path as `check_tmux_notifications`: session/window
    /// structure before layout before output. Called from the shared poll
    /// loop while a transport is installed.
    pub(crate) fn check_mux_notifications(&mut self) -> bool {
        let (core_notifications, disconnected) = match &self.tmux_state.transport {
            Some(transport) => transport.drain(),
            None => return false,
        };
        if disconnected {
            // The daemon died without `%exit`. Drop the dead socket; the
            // synthesized `SessionEnded` below then runs the shared
            // end-of-session cleanup exactly once.
            let _ = self.tmux_state.transport.take();
            self.tmux_state.mux_focused_pane = None;
        }
        // A delayed mux paste sends its due chunk here — before the
        // empty-drain early return below, or a quiet daemon would stall
        // the paste mid-line.
        let paste_sent = self.tick_pending_mux_paste();
        if core_notifications.is_empty() && !disconnected {
            self.apply_pending_mux_screen_seeds();
            self.apply_pending_mux_pane_titles();
            return paste_sent;
        }

        // The roster push, the agent release, and the daemon pane-title push
        // are core variants the ParserBridge deliberately drops (a named arm
        // there cannot compile against the published pin), so partition them
        // out here — the roster cache and the pane-title applier are their
        // consumers. This destructure is the single push call site for all.
        let (agent_pushes, core_notifications): (Vec<_>, Vec<_>) =
            core_notifications.into_iter().partition(|n| {
                matches!(
                    n,
                    par_term_emu_core_rust::tmux_control::TmuxNotification::AgentStateChanged { .. }
                )
            });
        let (agent_releases, core_notifications): (Vec<_>, Vec<_>) =
            core_notifications.into_iter().partition(|n| {
                matches!(
                    n,
                    par_term_emu_core_rust::tmux_control::TmuxNotification::AgentReleased { .. }
                )
            });
        let (title_pushes, core_notifications): (Vec<_>, Vec<_>) =
            core_notifications.into_iter().partition(|n| {
                matches!(
                    n,
                    par_term_emu_core_rust::tmux_control::TmuxNotification::PaneTitleChanged { .. }
                )
            });

        let mut notifications = ParserBridge::convert_all(core_notifications);
        if disconnected
            && !notifications
                .iter()
                .any(|n| matches!(n, TmuxNotification::SessionEnded))
        {
            notifications.push(TmuxNotification::SessionEnded);
        }

        // The roster must not outlive its daemon: clear it on an abrupt
        // death (the flag) or a graceful end (the notification), before
        // any surface reads a ghost.
        if disconnected
            || notifications
                .iter()
                .any(|n| matches!(n, TmuxNotification::SessionEnded))
        {
            self.tmux_state.agent_roster.clear();
            self.tmux_state.mux_pane_titles.clear();
        }

        crate::debug_info!("MUX", "Processing {} notifications", notifications.len());

        let mut needs_redraw = paste_sent;
        needs_redraw |= self.apply_agent_pushes(agent_pushes);
        needs_redraw |= self.apply_agent_releases(agent_releases);
        needs_redraw |= self.apply_pane_title_pushes(title_pushes);

        // Same bucket split as polling.rs — direct handlers TmuxSync cannot
        // translate, then the sync groups in dependency order.
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
        // Focus pushes are deferred until after the layout groups: a split's
        // `%window-pane-changed` for the NEW pane arrives in the same batch
        // as the `%layout-change` that creates it, so applying it here (the
        // pane not yet mapped) lost it, and native focus — the only routing
        // source of truth — stayed on the old pane.
        let mut deferred_focus: Option<TmuxPaneId> = None;
        for notification in direct_notifications {
            match notification {
                TmuxNotification::SessionStarted(session_name) => {
                    self.handle_mux_session_started(&session_name);
                    needs_redraw = true;
                }
                TmuxNotification::SessionRenamed(session_name) => {
                    self.handle_tmux_session_renamed(&session_name);
                    needs_redraw = true;
                }
                TmuxNotification::PaneFocusChanged { pane_id } => {
                    deferred_focus = Some(pane_id);
                    needs_redraw = true;
                }
                TmuxNotification::Error(msg) => {
                    self.handle_tmux_error(&msg);
                }
                TmuxNotification::ControlModeStarted => {
                    crate::debug_info!("MUX", "Control mode started");
                }
                _ => {}
            }
        }

        // --- TmuxSync dispatch: group 1 — session/window structure ---
        let session_actions = self
            .tmux_state
            .tmux_sync
            .process_notifications(&session_sync);
        needs_redraw |= self.process_sync_actions(session_actions);

        // --- TmuxSync dispatch: group 2 — layout changes ---
        let layout_actions = self
            .tmux_state
            .tmux_sync
            .process_notifications(&layout_sync);
        needs_redraw |= self.process_sync_actions(layout_actions);

        // Fallback: layouts for windows not yet mapped (on-the-fly mapping).
        for notification in &layout_sync {
            if let TmuxNotification::LayoutChange { window_id, layout } = notification
                && self.tmux_state.tmux_sync.get_tab(*window_id).is_none()
            {
                self.handle_tmux_layout_change(*window_id, layout);
                needs_redraw = true;
            }
        }

        // Deferred focus push: every pane this batch created is mapped now.
        if let Some(pane_id) = deferred_focus {
            self.tmux_state.mux_focused_pane = Some(pane_id);
            self.handle_tmux_pane_focus_changed(pane_id);
        }

        // --- TmuxSync dispatch: group 3 — pane output ---
        let output_actions = self
            .tmux_state
            .tmux_sync
            .process_notifications(&output_sync);
        needs_redraw |= self.process_sync_actions(output_actions);

        // Fallback: output for panes not yet mapped.
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

        self.apply_pending_mux_screen_seeds();
        self.apply_pending_mux_pane_titles();

        needs_redraw
    }

    /// Feed replayed screens to panes once their mappings exist — panes are
    /// created by the layout consumers on a later poll, so seeds from
    /// `attach_sequence` wait here until `get_native_pane` resolves.
    /// Delivery is the same `process_data` call the PaneOutput consumer
    /// uses; a seed is consumed only once delivered (a locked terminal
    /// retries next frame).
    fn apply_pending_mux_screen_seeds(&mut self) {
        if self.tmux_state.mux_screen_seeds.is_empty() {
            return;
        }
        let ready: Vec<TmuxPaneId> = self.tmux_state.mux_screen_seeds.keys().copied().collect();
        for pane in ready {
            self.deliver_pending_mux_seed(pane);
        }
    }

    /// Deliver daemon pane titles (reattach restore + pre-mapping pushes)
    /// to native panes once their mappings exist — consumed once
    /// delivered, mirroring the seed sweep. An entry for a pane that was
    /// closed before mapping lingers until detach clears the map (bounded
    /// by session lifetime, one string per closed pane).
    fn apply_pending_mux_pane_titles(&mut self) {
        if self.tmux_state.mux_pane_titles.is_empty() {
            return;
        }
        let ready: Vec<(TmuxPaneId, String, bool)> = self
            .tmux_state
            .mux_pane_titles
            .iter()
            .map(|(pane, (title, is_user))| (*pane, title.clone(), *is_user))
            .collect();
        for (pane, title, is_user) in ready {
            self.apply_daemon_pane_title(pane, &title, is_user);
        }
    }

    /// Set one pane's title from the daemon on its mapped native pane —
    /// the single delivery site for daemon pane titles. User titles
    /// re-mark the pane user-named (auto-title updates stop overwriting
    /// it); an OSC-sourced title (the reattach read-back) is only the
    /// initial title, so live OSC updates keep flowing. A clear (empty
    /// user title) reverts the pane to automatic titles. Returns whether
    /// the title landed (a pane without a mapping stays pending).
    fn apply_daemon_pane_title(
        &mut self,
        tmux_pane: TmuxPaneId,
        title: &str,
        is_user: bool,
    ) -> bool {
        fn apply_title(pane_obj: &mut crate::pane::Pane, title: &str, is_user: bool) {
            if is_user {
                if title.is_empty() {
                    // The daemon's clear: back to automatic titles,
                    // mirroring Rename Tab's blank branch.
                    pane_obj.user_named = false;
                    pane_obj.title = String::new();
                    pane_obj.has_default_title = true;
                } else {
                    pane_obj.title = title.to_string();
                    pane_obj.has_default_title = false;
                    pane_obj.user_named = true;
                }
            } else if !title.is_empty() {
                // OSC-sourced read-back: seed the title without
                // freezing it — the auto-title loop may refine it.
                pane_obj.title = title.to_string();
                pane_obj.has_default_title = false;
            }
        }

        // Per-tab mapping first — native pane ids restart at 1 in every
        // tab, so the owning tab must resolve the pane or another
        // window's pane takes the title.
        if let Some((owner_tab_id, native)) = self.tmux_state.tmux_pane_owner(tmux_pane) {
            if let Some(tab) = self.tab_manager.get_tab_mut(owner_tab_id)
                && let Some(pane_manager) = tab.pane_manager_mut()
                && let Some(pane_obj) = pane_manager.get_pane_mut(native)
            {
                apply_title(pane_obj, title, is_user);
                self.tmux_state.mux_pane_titles.remove(&tmux_pane);
                crate::debug_info!(
                    "MUX",
                    "applied daemon pane title for %{tmux_pane}: {:?} (user={is_user})",
                    title
                );
                return true;
            }
            return false;
        }

        // Legacy fallback: the sync map carries no owning tab.
        let Some(native) = self.tmux_state.tmux_sync.get_native_pane(tmux_pane) else {
            return false;
        };
        for tab in self.tab_manager.tabs_mut() {
            if let Some(pane_manager) = tab.pane_manager_mut()
                && let Some(pane_obj) = pane_manager.get_pane_mut(native)
            {
                apply_title(pane_obj, title, is_user);
                self.tmux_state.mux_pane_titles.remove(&tmux_pane);
                crate::debug_info!(
                    "MUX",
                    "applied daemon pane title for %{tmux_pane}: {:?} (user={is_user})",
                    title
                );
                return true;
            }
        }
        false
    }

    /// Feed one pane's pending reattach seed to its mapped native pane —
    /// the single delivery site, called from the end-of-poll sweep
    /// ([`Self::apply_pending_mux_screen_seeds`]) and, before newer live
    /// output, from `handle_tmux_output`. A seed is consumed only once
    /// delivered; a locked terminal retries on the next attempt (the same
    /// try_lock discipline as the PaneOutput consumer).
    ///
    /// The native pane resolves through the SAME lookup output routing
    /// uses — the per-tab app map first, the sync map as a legacy
    /// fallback. Resolving through `tmux_sync.get_native_pane` alone (an
    /// earlier form) always missed in production because nothing in the
    /// app populates the sync map, so every seed sat pending forever and
    /// panes reattached blank.
    pub(super) fn deliver_pending_mux_seed(&mut self, tmux_pane: TmuxPaneId) -> bool {
        let Some(data) = self.tmux_state.mux_screen_seeds.get(&tmux_pane).cloned() else {
            return false;
        };

        // Per-tab mapping — the owning tab resolves the pane (native ids
        // restart at 1 per tab).
        if let Some((owner_tab_id, native)) = self.tmux_state.tmux_pane_owner(tmux_pane) {
            if let Some(tab) = self.tab_manager.get_tab_mut(owner_tab_id)
                && let Some(pane_manager) = tab.pane_manager_mut()
                && let Some(pane_obj) = pane_manager.get_pane_mut(native)
                && let Ok(term) = pane_obj.terminal.try_read()
            {
                term.process_data(&data);
                self.tmux_state.mux_screen_seeds.remove(&tmux_pane);
                crate::debug_info!(
                    "MUX",
                    "delivered pending mux seed before live output for %{}",
                    tmux_pane
                );
                return true;
            }
            return false;
        }

        // Legacy fallback: the sync map carries no owning tab.
        let Some(native) = self.tmux_state.tmux_sync.get_native_pane(tmux_pane) else {
            return false;
        };
        for tab in self.tab_manager.tabs_mut() {
            if let Some(pane_manager) = tab.pane_manager_mut()
                && let Some(pane_obj) = pane_manager.get_pane_mut(native)
                && let Ok(term) = pane_obj.terminal.try_read()
            {
                term.process_data(&data);
                self.tmux_state.mux_screen_seeds.remove(&tmux_pane);
                crate::debug_info!(
                    "MUX",
                    "delivered pending mux seed before live output for %{}",
                    tmux_pane
                );
                return true;
            }
        }
        false
    }
}

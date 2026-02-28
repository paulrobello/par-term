//! Bidirectional state synchronization between tmux and par-term
//!
//! This module handles:
//! - Mapping tmux windows to par-term tabs
//! - Mapping tmux panes to par-term split panes
//! - Syncing layout changes
//! - Routing input/output to correct panes

use crate::session::TmuxNotification;
use crate::types::{TmuxPaneId, TmuxWindowId};
use par_term_config::{PaneId, TabId};
use std::collections::HashMap;

/// Synchronizes state between tmux and par-term
pub struct TmuxSync {
    /// Mapping from tmux window IDs to par-term tab IDs
    window_to_tab: HashMap<TmuxWindowId, TabId>,
    /// Reverse mapping from tab IDs to tmux window IDs
    tab_to_window: HashMap<TabId, TmuxWindowId>,
    /// Mapping from tmux pane IDs to par-term pane IDs
    pane_to_native: HashMap<TmuxPaneId, PaneId>,
    /// Reverse mapping from native pane IDs to tmux pane IDs
    native_to_pane: HashMap<PaneId, TmuxPaneId>,
    /// Whether sync is enabled
    enabled: bool,
    /// Whether output is paused (for slow connections)
    paused: bool,
    /// Buffered output during pause, keyed by pane ID
    pause_buffer: HashMap<TmuxPaneId, Vec<u8>>,
}

impl TmuxSync {
    /// Create a new sync manager
    pub fn new() -> Self {
        Self {
            window_to_tab: HashMap::new(),
            tab_to_window: HashMap::new(),
            pane_to_native: HashMap::new(),
            native_to_pane: HashMap::new(),
            enabled: false,
            paused: false,
            pause_buffer: HashMap::new(),
        }
    }

    /// Enable synchronization
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable synchronization
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Check if sync is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Map a tmux window to a par-term tab
    pub fn map_window(&mut self, window_id: TmuxWindowId, tab_id: TabId) {
        self.window_to_tab.insert(window_id, tab_id);
        self.tab_to_window.insert(tab_id, window_id);
    }

    /// Unmap a tmux window
    pub fn unmap_window(&mut self, window_id: TmuxWindowId) {
        if let Some(tab_id) = self.window_to_tab.remove(&window_id) {
            self.tab_to_window.remove(&tab_id);
        }
    }

    /// Get the tab ID for a tmux window
    pub fn get_tab(&self, window_id: TmuxWindowId) -> Option<TabId> {
        self.window_to_tab.get(&window_id).copied()
    }

    /// Get the tmux window ID for a tab
    pub fn get_window(&self, tab_id: TabId) -> Option<TmuxWindowId> {
        self.tab_to_window.get(&tab_id).copied()
    }

    /// Map a tmux pane to a native pane
    pub fn map_pane(&mut self, tmux_pane_id: TmuxPaneId, native_pane_id: PaneId) {
        self.pane_to_native.insert(tmux_pane_id, native_pane_id);
        self.native_to_pane.insert(native_pane_id, tmux_pane_id);
    }

    /// Unmap a tmux pane
    pub fn unmap_pane(&mut self, tmux_pane_id: TmuxPaneId) {
        if let Some(native_id) = self.pane_to_native.remove(&tmux_pane_id) {
            self.native_to_pane.remove(&native_id);
        }
    }

    /// Get the native pane ID for a tmux pane
    pub fn get_native_pane(&self, tmux_pane_id: TmuxPaneId) -> Option<PaneId> {
        self.pane_to_native.get(&tmux_pane_id).copied()
    }

    /// Get the tmux pane ID for a native pane
    pub fn get_tmux_pane(&self, native_pane_id: PaneId) -> Option<TmuxPaneId> {
        self.native_to_pane.get(&native_pane_id).copied()
    }

    // =========================================================================
    // Pause/Continue Handling (for slow connections)
    // =========================================================================

    /// Check if output is currently paused
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Enter paused state - output will be buffered until continue
    pub fn pause(&mut self) {
        self.paused = true;
        log::info!("tmux output paused (slow connection)");
    }

    /// Exit paused state and return buffered output
    ///
    /// Returns a map of pane ID -> buffered data
    pub fn resume(&mut self) -> HashMap<TmuxPaneId, Vec<u8>> {
        self.paused = false;
        let buffered = std::mem::take(&mut self.pause_buffer);
        log::info!(
            "tmux output resumed, flushing {} panes with buffered data",
            buffered.len()
        );
        buffered
    }

    /// Buffer output for a pane during pause
    ///
    /// Returns true if data was buffered, false if not paused
    pub fn buffer_output(&mut self, pane_id: TmuxPaneId, data: &[u8]) -> bool {
        if !self.paused {
            return false;
        }

        self.pause_buffer
            .entry(pane_id)
            .or_default()
            .extend_from_slice(data);
        true
    }

    /// Get the total size of buffered data
    pub fn buffered_size(&self) -> usize {
        self.pause_buffer.values().map(|v| v.len()).sum()
    }

    /// Process notifications and generate sync actions
    ///
    /// Returns a list of actions to perform on the par-term side.
    /// In gateway mode, notifications come from `TerminalManager::drain_tmux_notifications()`.
    pub fn process_notifications(&mut self, notifications: &[TmuxNotification]) -> Vec<SyncAction> {
        let mut actions = Vec::new();

        for notif in notifications {
            match notif {
                TmuxNotification::WindowAdd(window_id) => {
                    actions.push(SyncAction::CreateTab {
                        window_id: *window_id,
                    });
                }
                TmuxNotification::WindowClose(window_id) => {
                    if let Some(tab_id) = self.get_tab(*window_id) {
                        actions.push(SyncAction::CloseTab { tab_id });
                        self.unmap_window(*window_id);
                    }
                }
                TmuxNotification::WindowRenamed { id, name } => {
                    if let Some(tab_id) = self.get_tab(*id) {
                        actions.push(SyncAction::RenameTab {
                            tab_id,
                            name: name.clone(),
                        });
                    }
                }
                TmuxNotification::LayoutChange { window_id, layout } => {
                    if let Some(tab_id) = self.get_tab(*window_id) {
                        actions.push(SyncAction::UpdateLayout {
                            tab_id,
                            layout: layout.clone(),
                        });
                    }
                }
                TmuxNotification::Output { pane_id, data } => {
                    if let Some(native_id) = self.get_native_pane(*pane_id) {
                        actions.push(SyncAction::PaneOutput {
                            pane_id: native_id,
                            data: data.clone(),
                        });
                    }
                }
                TmuxNotification::SessionEnded => {
                    actions.push(SyncAction::SessionEnded);
                }
                TmuxNotification::Pause => {
                    actions.push(SyncAction::Pause);
                }
                TmuxNotification::Continue => {
                    actions.push(SyncAction::Continue);
                }
                TmuxNotification::ControlModeStarted
                | TmuxNotification::SessionStarted(_)
                | TmuxNotification::SessionRenamed(_)
                | TmuxNotification::Error(_)
                | TmuxNotification::PaneFocusChanged { .. } => {
                    // These are handled elsewhere (directly in tmux_handler.rs)
                }
            }
        }

        actions
    }

    /// Clear all mappings
    pub fn clear(&mut self) {
        self.window_to_tab.clear();
        self.tab_to_window.clear();
        self.pane_to_native.clear();
        self.native_to_pane.clear();
    }
}

impl Default for TmuxSync {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::TmuxNotification;

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    fn make_sync_with_window(window_id: TmuxWindowId, tab_id: TabId) -> TmuxSync {
        let mut sync = TmuxSync::new();
        sync.map_window(window_id, tab_id);
        sync
    }

    fn make_sync_with_pane(tmux_pane_id: TmuxPaneId, native_pane_id: PaneId) -> TmuxSync {
        let mut sync = TmuxSync::new();
        sync.map_pane(tmux_pane_id, native_pane_id);
        sync
    }

    // -------------------------------------------------------------------------
    // CreateTab — WindowAdd always produces an action (no mapping required)
    // -------------------------------------------------------------------------

    #[test]
    fn window_add_produces_create_tab() {
        let mut sync = TmuxSync::new();
        let notifs = vec![TmuxNotification::WindowAdd(42)];
        let actions = sync.process_notifications(&notifs);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::CreateTab { window_id } => assert_eq!(*window_id, 42),
            other => panic!("expected CreateTab, got {:?}", other),
        }
    }

    // -------------------------------------------------------------------------
    // CloseTab — WindowClose only produces an action when the window is mapped
    // -------------------------------------------------------------------------

    #[test]
    fn window_close_produces_close_tab_when_mapped() {
        let mut sync = make_sync_with_window(7, 100);
        let notifs = vec![TmuxNotification::WindowClose(7)];
        let actions = sync.process_notifications(&notifs);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::CloseTab { tab_id } => assert_eq!(*tab_id, 100),
            other => panic!("expected CloseTab, got {:?}", other),
        }
        // Mapping must be removed so the fallback path won't double-process it.
        assert!(
            sync.get_tab(7).is_none(),
            "window mapping should be removed after close"
        );
    }

    #[test]
    fn window_close_produces_no_action_when_unmapped() {
        let mut sync = TmuxSync::new();
        let notifs = vec![TmuxNotification::WindowClose(99)];
        let actions = sync.process_notifications(&notifs);
        assert!(
            actions.is_empty(),
            "unmapped window close should produce no action"
        );
    }

    // -------------------------------------------------------------------------
    // RenameTab — WindowRenamed only produces an action when the window is mapped
    // -------------------------------------------------------------------------

    #[test]
    fn window_renamed_produces_rename_tab_when_mapped() {
        let mut sync = make_sync_with_window(3, 50);
        let notifs = vec![TmuxNotification::WindowRenamed {
            id: 3,
            name: "my-shell".into(),
        }];
        let actions = sync.process_notifications(&notifs);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::RenameTab { tab_id, name } => {
                assert_eq!(*tab_id, 50);
                assert_eq!(name, "my-shell");
            }
            other => panic!("expected RenameTab, got {:?}", other),
        }
    }

    #[test]
    fn window_renamed_produces_no_action_when_unmapped() {
        let mut sync = TmuxSync::new();
        let notifs = vec![TmuxNotification::WindowRenamed {
            id: 5,
            name: "irrelevant".into(),
        }];
        let actions = sync.process_notifications(&notifs);
        assert!(actions.is_empty());
    }

    // -------------------------------------------------------------------------
    // UpdateLayout — LayoutChange only produces an action when the window is mapped
    // -------------------------------------------------------------------------

    #[test]
    fn layout_change_produces_update_layout_when_mapped() {
        let mut sync = make_sync_with_window(1, 10);
        let notifs = vec![TmuxNotification::LayoutChange {
            window_id: 1,
            layout: "abc123,80x24,0,0".into(),
        }];
        let actions = sync.process_notifications(&notifs);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::UpdateLayout { tab_id, layout } => {
                assert_eq!(*tab_id, 10);
                assert_eq!(layout, "abc123,80x24,0,0");
            }
            other => panic!("expected UpdateLayout, got {:?}", other),
        }
    }

    #[test]
    fn layout_change_produces_no_action_when_unmapped() {
        let mut sync = TmuxSync::new();
        let notifs = vec![TmuxNotification::LayoutChange {
            window_id: 2,
            layout: "80x24".into(),
        }];
        let actions = sync.process_notifications(&notifs);
        assert!(
            actions.is_empty(),
            "unmapped layout change should produce no action (fallback in polling.rs)"
        );
    }

    // -------------------------------------------------------------------------
    // PaneOutput — Output only produces an action when the pane is mapped;
    //              pane_id in the action is the *native* PaneId.
    // -------------------------------------------------------------------------

    #[test]
    fn output_produces_pane_output_with_native_id_when_mapped() {
        let mut sync = make_sync_with_pane(20, 200);
        let payload = b"hello tmux".to_vec();
        let notifs = vec![TmuxNotification::Output {
            pane_id: 20,
            data: payload.clone(),
        }];
        let actions = sync.process_notifications(&notifs);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            SyncAction::PaneOutput { pane_id, data } => {
                assert_eq!(
                    *pane_id, 200,
                    "pane_id should be native (200), not tmux (20)"
                );
                assert_eq!(data, &payload);
            }
            other => panic!("expected PaneOutput, got {:?}", other),
        }
    }

    #[test]
    fn output_produces_no_action_when_pane_unmapped() {
        let mut sync = TmuxSync::new();
        let notifs = vec![TmuxNotification::Output {
            pane_id: 99,
            data: b"data".to_vec(),
        }];
        let actions = sync.process_notifications(&notifs);
        assert!(
            actions.is_empty(),
            "unmapped pane output should produce no action (fallback in polling.rs)"
        );
    }

    // -------------------------------------------------------------------------
    // Session lifecycle and flow-control pass-throughs
    // -------------------------------------------------------------------------

    #[test]
    fn session_ended_produces_action() {
        let mut sync = TmuxSync::new();
        let actions = sync.process_notifications(&[TmuxNotification::SessionEnded]);
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], SyncAction::SessionEnded));
    }

    #[test]
    fn pause_produces_action() {
        let mut sync = TmuxSync::new();
        let actions = sync.process_notifications(&[TmuxNotification::Pause]);
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], SyncAction::Pause));
    }

    #[test]
    fn continue_produces_action() {
        let mut sync = TmuxSync::new();
        let actions = sync.process_notifications(&[TmuxNotification::Continue]);
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], SyncAction::Continue));
    }

    // -------------------------------------------------------------------------
    // Direct-dispatch notifications are silently ignored by TmuxSync
    // -------------------------------------------------------------------------

    #[test]
    fn direct_dispatch_notifications_produce_no_actions() {
        let mut sync = TmuxSync::new();
        let notifs = vec![
            TmuxNotification::ControlModeStarted,
            TmuxNotification::SessionStarted("mysession".into()),
            TmuxNotification::SessionRenamed("newsession".into()),
            TmuxNotification::Error("something went wrong".into()),
            TmuxNotification::PaneFocusChanged { pane_id: 5 },
        ];
        let actions = sync.process_notifications(&notifs);
        assert!(
            actions.is_empty(),
            "direct-dispatch notifications should not be translated to SyncActions"
        );
    }

    // -------------------------------------------------------------------------
    // Ordering: window→tab mapping created by CreateTab is available for
    // UpdateLayout within the same call only if the call is split by group
    // (which polling.rs does — each group is a separate process_notifications call).
    // This test verifies that if window is already mapped, layout works correctly.
    // -------------------------------------------------------------------------

    #[test]
    fn layout_uses_mapping_established_in_prior_group() {
        let mut sync = TmuxSync::new();

        // Simulate what polling.rs does: first group creates the mapping via process_sync_actions
        // (which calls handle_tmux_window_add → map_window). Here we set it up directly.
        sync.map_window(8, 80);

        // Second group: LayoutChange should now find the mapping.
        let layout_notifs = vec![TmuxNotification::LayoutChange {
            window_id: 8,
            layout: "80x24,0,0,0".into(),
        }];
        let actions = sync.process_notifications(&layout_notifs);

        assert_eq!(actions.len(), 1);
        assert!(matches!(
            actions[0],
            SyncAction::UpdateLayout { tab_id: 80, .. }
        ));
    }

    // -------------------------------------------------------------------------
    // Batch ordering: multiple notifications are translated in iteration order
    // -------------------------------------------------------------------------

    #[test]
    fn multiple_notifications_translated_in_order() {
        let mut sync = TmuxSync::new();
        sync.map_window(1, 10);
        sync.map_window(2, 20);

        let notifs = vec![
            TmuxNotification::WindowRenamed {
                id: 1,
                name: "first".into(),
            },
            TmuxNotification::WindowRenamed {
                id: 2,
                name: "second".into(),
            },
        ];
        let actions = sync.process_notifications(&notifs);

        assert_eq!(actions.len(), 2);
        match (&actions[0], &actions[1]) {
            (
                SyncAction::RenameTab {
                    tab_id: 10,
                    name: n0,
                },
                SyncAction::RenameTab {
                    tab_id: 20,
                    name: n1,
                },
            ) => {
                assert_eq!(n0, "first");
                assert_eq!(n1, "second");
            }
            _ => panic!("unexpected action order: {:?}", actions),
        }
    }
}

/// Actions to perform on the par-term side based on tmux notifications
#[derive(Debug, Clone)]
pub enum SyncAction {
    /// Create a new tab for a tmux window
    CreateTab { window_id: TmuxWindowId },
    /// Close a tab
    CloseTab { tab_id: TabId },
    /// Rename a tab
    RenameTab { tab_id: TabId, name: String },
    /// Update the pane layout in a tab
    UpdateLayout { tab_id: TabId, layout: String },
    /// Route output to a pane
    PaneOutput { pane_id: PaneId, data: Vec<u8> },
    /// Session has ended
    SessionEnded,
    /// Pause updates (slow connection)
    Pause,
    /// Continue updates
    Continue,
}

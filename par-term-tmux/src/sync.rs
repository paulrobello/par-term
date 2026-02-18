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

//! Window-level state synchronization between tmux and par-term.
//!
//! Handles mapping between tmux window IDs and par-term tab IDs.

use crate::types::TmuxWindowId;
use par_term_config::TabId;
use std::collections::HashMap;

/// Window-level sync state: tracks tmux↔tab ID mappings.
#[derive(Debug, Default)]
pub struct WindowSyncState {
    /// Mapping from tmux window IDs to par-term tab IDs
    pub(crate) window_to_tab: HashMap<TmuxWindowId, TabId>,
    /// Reverse mapping from tab IDs to tmux window IDs
    pub(crate) tab_to_window: HashMap<TabId, TmuxWindowId>,
}

impl WindowSyncState {
    /// Create a new, empty window sync state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Map a tmux window to a par-term tab.
    pub fn map_window(&mut self, window_id: TmuxWindowId, tab_id: TabId) {
        self.window_to_tab.insert(window_id, tab_id);
        self.tab_to_window.insert(tab_id, window_id);
    }

    /// Unmap a tmux window, removing both directions of the mapping.
    pub fn unmap_window(&mut self, window_id: TmuxWindowId) {
        if let Some(tab_id) = self.window_to_tab.remove(&window_id) {
            self.tab_to_window.remove(&tab_id);
        }
    }

    /// Get the tab ID for a tmux window.
    pub fn get_tab(&self, window_id: TmuxWindowId) -> Option<TabId> {
        self.window_to_tab.get(&window_id).copied()
    }

    /// Get the tmux window ID for a tab.
    pub fn get_window(&self, tab_id: TabId) -> Option<TmuxWindowId> {
        self.tab_to_window.get(&tab_id).copied()
    }

    /// Clear all window mappings.
    pub fn clear(&mut self) {
        self.window_to_tab.clear();
        self.tab_to_window.clear();
    }
}

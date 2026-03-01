//! Pane-level state synchronization between tmux and par-term.
//!
//! Handles mapping between tmux pane IDs and par-term native pane IDs,
//! plus output buffering for slow-connection pause/resume handling.

use crate::types::TmuxPaneId;
use par_term_config::PaneId;
use std::collections::HashMap;

/// Pane-level sync state: tracks tmux↔native pane ID mappings and
/// buffers pane output during a connection pause.
#[derive(Debug, Default)]
pub struct PaneSyncState {
    /// Mapping from tmux pane IDs to par-term pane IDs
    pub(crate) pane_to_native: HashMap<TmuxPaneId, PaneId>,
    /// Reverse mapping from native pane IDs to tmux pane IDs
    pub(crate) native_to_pane: HashMap<PaneId, TmuxPaneId>,
    /// Whether output is paused (for slow connections)
    pub(crate) paused: bool,
    /// Buffered output during pause, keyed by pane ID
    pub(crate) pause_buffer: HashMap<TmuxPaneId, Vec<u8>>,
}

impl PaneSyncState {
    /// Create a new, empty pane sync state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Map a tmux pane to a native pane.
    pub fn map_pane(&mut self, tmux_pane_id: TmuxPaneId, native_pane_id: PaneId) {
        self.pane_to_native.insert(tmux_pane_id, native_pane_id);
        self.native_to_pane.insert(native_pane_id, tmux_pane_id);
    }

    /// Unmap a tmux pane, removing both directions of the mapping.
    pub fn unmap_pane(&mut self, tmux_pane_id: TmuxPaneId) {
        if let Some(native_id) = self.pane_to_native.remove(&tmux_pane_id) {
            self.native_to_pane.remove(&native_id);
        }
    }

    /// Get the native pane ID for a tmux pane.
    pub fn get_native_pane(&self, tmux_pane_id: TmuxPaneId) -> Option<PaneId> {
        self.pane_to_native.get(&tmux_pane_id).copied()
    }

    /// Get the tmux pane ID for a native pane.
    pub fn get_tmux_pane(&self, native_pane_id: PaneId) -> Option<TmuxPaneId> {
        self.native_to_pane.get(&native_pane_id).copied()
    }

    /// Check if output is currently paused.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Enter paused state — output will be buffered until resume.
    pub fn pause(&mut self) {
        self.paused = true;
        log::info!("tmux output paused (slow connection)");
    }

    /// Exit paused state and return buffered output.
    ///
    /// Returns a map of pane ID → buffered data.
    pub fn resume(&mut self) -> HashMap<TmuxPaneId, Vec<u8>> {
        self.paused = false;
        let buffered = std::mem::take(&mut self.pause_buffer);
        log::info!(
            "tmux output resumed, flushing {} panes with buffered data",
            buffered.len()
        );
        buffered
    }

    /// Buffer output for a pane during pause.
    ///
    /// Returns `true` if data was buffered, `false` if not currently paused.
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

    /// Get the total size of buffered data across all paused panes.
    pub fn buffered_size(&self) -> usize {
        self.pause_buffer.values().map(|v| v.len()).sum()
    }

    /// Clear all pane mappings and pause state.
    pub fn clear(&mut self) {
        self.pane_to_native.clear();
        self.native_to_pane.clear();
        self.paused = false;
        self.pause_buffer.clear();
    }
}

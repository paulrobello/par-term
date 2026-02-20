//! Tab manager for coordinating multiple terminal tabs within a window

use super::{Tab, TabId};
use crate::config::Config;
use crate::profile::Profile;
use anyhow::Result;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Manages multiple terminal tabs within a single window
pub struct TabManager {
    /// All tabs in this window, in order
    tabs: Vec<Tab>,
    /// Currently active tab ID
    active_tab_id: Option<TabId>,
    /// Counter for generating unique tab IDs
    next_tab_id: TabId,
}

impl TabManager {
    /// Create a new empty tab manager
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab_id: None,
            next_tab_id: 1,
        }
    }

    /// Create a new tab and return its ID
    ///
    /// # Arguments
    /// * `config` - Terminal configuration
    /// * `runtime` - Tokio runtime for async operations
    /// * `inherit_cwd_from_active` - Whether to inherit working directory from active tab
    /// * `grid_size` - Optional (cols, rows) override for initial terminal size.
    ///   When provided, these dimensions are used instead of config.cols/rows.
    ///   This is important when the renderer has already calculated the correct
    ///   grid size accounting for tab bar height.
    pub fn new_tab(
        &mut self,
        config: &Config,
        runtime: Arc<Runtime>,
        inherit_cwd_from_active: bool,
        grid_size: Option<(usize, usize)>,
    ) -> Result<TabId> {
        // Optionally inherit working directory from active tab
        let working_dir = if inherit_cwd_from_active {
            self.active_tab().and_then(|tab| tab.get_cwd())
        } else {
            None
        };

        let id = self.next_tab_id;
        self.next_tab_id += 1;

        // Tab number is based on current count, not unique ID
        let tab_number = self.tabs.len() + 1;
        let tab = Tab::new(id, tab_number, config, runtime, working_dir, grid_size)?;
        self.tabs.push(tab);

        // Always switch to the new tab
        self.active_tab_id = Some(id);

        log::info!("Created new tab {} (total: {})", id, self.tabs.len());

        Ok(id)
    }

    /// Create a new tab with a specific working directory
    ///
    /// Used by arrangement restore to create tabs with saved CWDs.
    pub fn new_tab_with_cwd(
        &mut self,
        config: &Config,
        runtime: Arc<Runtime>,
        working_dir: Option<String>,
        grid_size: Option<(usize, usize)>,
    ) -> Result<TabId> {
        let id = self.next_tab_id;
        self.next_tab_id += 1;

        let tab_number = self.tabs.len() + 1;
        let tab = Tab::new(id, tab_number, config, runtime, working_dir, grid_size)?;
        self.tabs.push(tab);

        // Always switch to the new tab
        self.active_tab_id = Some(id);

        log::info!(
            "Created new tab {} with cwd (total: {})",
            id,
            self.tabs.len()
        );

        Ok(id)
    }

    /// Create a new tab from a profile configuration
    ///
    /// The profile specifies the working directory, command, and tab name.
    ///
    /// # Arguments
    /// * `config` - Terminal configuration
    /// * `runtime` - Tokio runtime for async operations
    /// * `profile` - Profile configuration to use
    /// * `grid_size` - Optional (cols, rows) override for initial terminal size
    pub fn new_tab_from_profile(
        &mut self,
        config: &Config,
        runtime: Arc<Runtime>,
        profile: &Profile,
        grid_size: Option<(usize, usize)>,
    ) -> Result<TabId> {
        let id = self.next_tab_id;
        self.next_tab_id += 1;

        let tab = Tab::new_from_profile(id, config, runtime, profile, grid_size)?;
        self.tabs.push(tab);

        // Always switch to the new tab
        self.active_tab_id = Some(id);

        log::info!(
            "Created new tab {} from profile '{}' (total: {})",
            id,
            profile.name,
            self.tabs.len()
        );

        Ok(id)
    }

    /// Close a tab by ID
    /// Returns true if this was the last tab (window should close)
    pub fn close_tab(&mut self, id: TabId) -> bool {
        let index = self.tabs.iter().position(|t| t.id == id);

        if let Some(idx) = index {
            log::info!("Closing tab {} (index {})", id, idx);

            // Remove the tab
            self.tabs.remove(idx);

            // If we closed the active tab, switch to another
            if self.active_tab_id == Some(id) {
                self.active_tab_id = if self.tabs.is_empty() {
                    None
                } else {
                    // Prefer the tab at the same index (or previous if at end)
                    let new_idx = idx.min(self.tabs.len().saturating_sub(1));
                    Some(self.tabs[new_idx].id)
                };
            }

            // Renumber tabs that still have default titles
            self.renumber_default_tabs();
        }

        self.tabs.is_empty()
    }

    /// Remove a tab by ID without dropping it, returning the live Tab.
    ///
    /// Handles active tab switching and renumbering just like `close_tab`,
    /// but returns the `Tab` so the caller can keep it alive.
    ///
    /// Returns `Some((tab, is_empty))` if the tab was found, `None` otherwise.
    pub fn remove_tab(&mut self, id: TabId) -> Option<(Tab, bool)> {
        let idx = self.tabs.iter().position(|t| t.id == id)?;

        log::info!("Removing tab {} (index {}) without dropping", id, idx);

        let tab = self.tabs.remove(idx);

        // If we removed the active tab, switch to another
        if self.active_tab_id == Some(id) {
            self.active_tab_id = if self.tabs.is_empty() {
                None
            } else {
                let new_idx = idx.min(self.tabs.len().saturating_sub(1));
                Some(self.tabs[new_idx].id)
            };
        }

        self.renumber_default_tabs();
        let is_empty = self.tabs.is_empty();
        Some((tab, is_empty))
    }

    /// Insert a live Tab at a specific index and make it active.
    ///
    /// The index is clamped to `0..=self.tabs.len()`.
    pub fn insert_tab_at(&mut self, tab: Tab, index: usize) {
        let clamped = index.min(self.tabs.len());
        let id = tab.id;
        self.tabs.insert(clamped, tab);
        self.active_tab_id = Some(id);
        self.renumber_default_tabs();
        log::info!(
            "Inserted tab {} at index {} (total: {})",
            id,
            clamped,
            self.tabs.len()
        );
    }

    /// Renumber tabs that have default titles based on their current position
    fn renumber_default_tabs(&mut self) {
        for (idx, tab) in self.tabs.iter_mut().enumerate() {
            tab.set_default_title(idx + 1);
        }
    }

    /// Get a reference to the active tab
    pub fn active_tab(&self) -> Option<&Tab> {
        self.active_tab_id
            .and_then(|id| self.tabs.iter().find(|t| t.id == id))
    }

    /// Get a mutable reference to the active tab
    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        let active_id = self.active_tab_id;
        active_id.and_then(move |id| self.tabs.iter_mut().find(|t| t.id == id))
    }

    /// Switch to a tab by ID
    pub fn switch_to(&mut self, id: TabId) {
        if self.tabs.iter().any(|t| t.id == id) {
            // Clear activity indicator when switching to tab
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                tab.has_activity = false;
            }
            self.active_tab_id = Some(id);
            log::debug!("Switched to tab {}", id);
        }
    }

    /// Switch to the next tab (wraps around)
    pub fn next_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }

        if let Some(active_id) = self.active_tab_id {
            let current_idx = self
                .tabs
                .iter()
                .position(|t| t.id == active_id)
                .unwrap_or(0);
            let next_idx = (current_idx + 1) % self.tabs.len();
            let next_id = self.tabs[next_idx].id;
            self.switch_to(next_id);
        }
    }

    /// Switch to the previous tab (wraps around)
    pub fn prev_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }

        if let Some(active_id) = self.active_tab_id {
            let current_idx = self
                .tabs
                .iter()
                .position(|t| t.id == active_id)
                .unwrap_or(0);
            let prev_idx = if current_idx == 0 {
                self.tabs.len() - 1
            } else {
                current_idx - 1
            };
            let prev_id = self.tabs[prev_idx].id;
            self.switch_to(prev_id);
        }
    }

    /// Switch to tab by index (1-based for Cmd+1-9)
    pub fn switch_to_index(&mut self, index: usize) {
        if index > 0 && index <= self.tabs.len() {
            let id = self.tabs[index - 1].id;
            self.switch_to(id);
        }
    }

    /// Move a tab left or right
    /// direction: -1 for left, 1 for right
    pub fn move_tab(&mut self, id: TabId, direction: i32) {
        if let Some(current_idx) = self.tabs.iter().position(|t| t.id == id) {
            let new_idx = if direction < 0 {
                if current_idx == 0 {
                    self.tabs.len() - 1
                } else {
                    current_idx - 1
                }
            } else if current_idx >= self.tabs.len() - 1 {
                0
            } else {
                current_idx + 1
            };

            if new_idx != current_idx {
                let tab = self.tabs.remove(current_idx);
                self.tabs.insert(new_idx, tab);
                log::debug!("Moved tab {} from index {} to {}", id, current_idx, new_idx);
                // Renumber tabs that still have default titles
                self.renumber_default_tabs();
            }
        }
    }

    /// Move a tab to a specific index (used by drag-and-drop reordering)
    /// Returns true if the tab was actually moved, false if not found or already at target
    pub fn move_tab_to_index(&mut self, id: TabId, target_index: usize) -> bool {
        let current_idx = match self.tabs.iter().position(|t| t.id == id) {
            Some(idx) => idx,
            None => return false,
        };

        let clamped_target = target_index.min(self.tabs.len().saturating_sub(1));
        if clamped_target == current_idx {
            return false;
        }

        let tab = self.tabs.remove(current_idx);
        self.tabs.insert(clamped_target, tab);
        log::debug!(
            "Moved tab {} from index {} to {}",
            id,
            current_idx,
            clamped_target
        );
        self.renumber_default_tabs();
        true
    }

    /// Move active tab left
    pub fn move_active_tab_left(&mut self) {
        if let Some(id) = self.active_tab_id {
            self.move_tab(id, -1);
        }
    }

    /// Move active tab right
    pub fn move_active_tab_right(&mut self) {
        if let Some(id) = self.active_tab_id {
            self.move_tab(id, 1);
        }
    }

    /// Get the number of tabs
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Check if there are multiple tabs
    pub fn has_multiple_tabs(&self) -> bool {
        self.tabs.len() > 1
    }

    /// Get the active tab ID
    pub fn active_tab_id(&self) -> Option<TabId> {
        self.active_tab_id
    }

    /// Get all tabs as a slice
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// Get all tabs as mutable slice
    pub fn tabs_mut(&mut self) -> &mut [Tab] {
        &mut self.tabs
    }

    /// Drain all tabs from the manager, returning them without dropping
    ///
    /// This is used during fast shutdown to extract tabs so their terminals
    /// can be dropped on background threads in parallel.
    pub fn drain_tabs(&mut self) -> Vec<Tab> {
        self.active_tab_id = None;
        std::mem::take(&mut self.tabs)
    }

    /// Get a tab by ID
    #[allow(dead_code)]
    pub fn get_tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    /// Get a mutable reference to a tab by ID
    #[allow(dead_code)]
    pub fn get_tab_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    /// Mark non-active tabs as having activity when they receive output
    #[allow(dead_code)]
    pub fn mark_activity(&mut self, tab_id: TabId) {
        if Some(tab_id) != self.active_tab_id
            && let Some(tab) = self.get_tab_mut(tab_id)
        {
            tab.has_activity = true;
        }
    }

    /// Update titles for all tabs
    pub fn update_all_titles(&mut self, title_mode: par_term_config::TabTitleMode) {
        for tab in &mut self.tabs {
            tab.update_title(title_mode);
        }
    }

    /// Duplicate the active tab (creates new tab with same working directory and color)
    ///
    /// # Arguments
    /// * `config` - Terminal configuration
    /// * `runtime` - Tokio runtime for async operations
    /// * `grid_size` - Optional (cols, rows) override for initial terminal size
    pub fn duplicate_active_tab(
        &mut self,
        config: &Config,
        runtime: Arc<Runtime>,
        grid_size: Option<(usize, usize)>,
    ) -> Result<Option<TabId>> {
        if let Some(tab_id) = self.active_tab_id {
            self.duplicate_tab_by_id(tab_id, config, runtime, grid_size)
        } else {
            Ok(None)
        }
    }

    /// Duplicate a specific tab by ID (creates new tab with same working directory and color)
    ///
    /// # Arguments
    /// * `source_tab_id` - The ID of the tab to duplicate
    /// * `config` - Terminal configuration
    /// * `runtime` - Tokio runtime for async operations
    /// * `grid_size` - Optional (cols, rows) override for initial terminal size
    pub fn duplicate_tab_by_id(
        &mut self,
        source_tab_id: TabId,
        config: &Config,
        runtime: Arc<Runtime>,
        grid_size: Option<(usize, usize)>,
    ) -> Result<Option<TabId>> {
        // Gather properties from source tab
        let source_idx = self.tabs.iter().position(|t| t.id == source_tab_id);
        let source_idx = match source_idx {
            Some(idx) => idx,
            None => return Ok(None),
        };
        let working_dir = self.tabs[source_idx].get_cwd();
        let custom_color = self.tabs[source_idx].custom_color;
        let custom_icon = self.tabs[source_idx].custom_icon.clone();

        let id = self.next_tab_id;
        self.next_tab_id += 1;

        // Tab number is based on current count, not unique ID
        let tab_number = self.tabs.len() + 1;
        let mut tab = Tab::new(id, tab_number, config, runtime, working_dir, grid_size)?;

        // Copy tab color from source
        if let Some(color) = custom_color {
            tab.set_custom_color(color);
        }

        // Copy custom icon from source
        tab.custom_icon = custom_icon;

        // Insert after source tab
        self.tabs.insert(source_idx + 1, tab);

        self.active_tab_id = Some(id);
        Ok(Some(id))
    }

    /// Get index of active tab (0-based)
    #[allow(dead_code)]
    pub fn active_tab_index(&self) -> Option<usize> {
        self.active_tab_id
            .and_then(|id| self.tabs.iter().position(|t| t.id == id))
    }

    /// Clean up closed/dead tabs
    #[allow(dead_code)]
    pub fn cleanup_dead_tabs(&mut self) {
        let dead_tabs: Vec<TabId> = self
            .tabs
            .iter()
            .filter(|t| !t.is_running())
            .map(|t| t.id)
            .collect();

        for id in dead_tabs {
            log::info!("Cleaning up dead tab {}", id);
            self.close_tab(id);
        }
    }
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a TabManager with mock tabs for testing (no PTY, no runtime)
    fn manager_with_ids(ids: &[TabId]) -> TabManager {
        let mut mgr = TabManager::new();
        for &id in ids {
            let tab_number = mgr.tabs.len() + 1;
            // Create a minimal tab struct directly for testing
            mgr.tabs.push(Tab::new_stub(id, tab_number));
            mgr.next_tab_id = mgr.next_tab_id.max(id + 1);
        }
        if let Some(last) = ids.last() {
            mgr.active_tab_id = Some(*last);
        }
        mgr
    }

    #[test]
    fn move_tab_to_index_forward() {
        let mut mgr = manager_with_ids(&[1, 2, 3, 4]);
        // Move tab 1 from index 0 to index 2
        assert!(mgr.move_tab_to_index(1, 2));
        let ids: Vec<TabId> = mgr.tabs.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![2, 3, 1, 4]);
    }

    #[test]
    fn move_tab_to_index_backward() {
        let mut mgr = manager_with_ids(&[1, 2, 3, 4]);
        // Move tab 3 from index 2 to index 0
        assert!(mgr.move_tab_to_index(3, 0));
        let ids: Vec<TabId> = mgr.tabs.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![3, 1, 2, 4]);
    }

    #[test]
    fn move_tab_to_index_same_position() {
        let mut mgr = manager_with_ids(&[1, 2, 3]);
        // Moving to same position is a no-op
        assert!(!mgr.move_tab_to_index(2, 1));
        let ids: Vec<TabId> = mgr.tabs.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn move_tab_to_index_out_of_bounds_clamped() {
        let mut mgr = manager_with_ids(&[1, 2, 3]);
        // Target index 100 should clamp to last position (2)
        assert!(mgr.move_tab_to_index(1, 100));
        let ids: Vec<TabId> = mgr.tabs.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![2, 3, 1]);
    }

    #[test]
    fn move_tab_to_index_invalid_id() {
        let mut mgr = manager_with_ids(&[1, 2, 3]);
        // Non-existent tab ID returns false
        assert!(!mgr.move_tab_to_index(99, 0));
        let ids: Vec<TabId> = mgr.tabs.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn move_tab_to_index_to_end() {
        let mut mgr = manager_with_ids(&[1, 2, 3]);
        // Move first tab to last position
        assert!(mgr.move_tab_to_index(1, 2));
        let ids: Vec<TabId> = mgr.tabs.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![2, 3, 1]);
    }

    #[test]
    fn move_tab_to_index_to_start() {
        let mut mgr = manager_with_ids(&[1, 2, 3]);
        // Move last tab to first position
        assert!(mgr.move_tab_to_index(3, 0));
        let ids: Vec<TabId> = mgr.tabs.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![3, 1, 2]);
    }
}

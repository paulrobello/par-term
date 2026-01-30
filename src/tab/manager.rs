//! Tab manager for coordinating multiple terminal tabs within a window

use super::{Tab, TabId};
use crate::config::Config;
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
    pub fn new_tab(
        &mut self,
        config: &Config,
        runtime: Arc<Runtime>,
        inherit_cwd_from_active: bool,
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
        let tab = Tab::new(id, tab_number, config, runtime, working_dir)?;
        self.tabs.push(tab);

        // Always switch to the new tab
        self.active_tab_id = Some(id);

        log::info!("Created new tab {} (total: {})", id, self.tabs.len());

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
    pub fn update_all_titles(&mut self) {
        for tab in &mut self.tabs {
            tab.update_title();
        }
    }

    /// Duplicate the active tab (creates new tab with same working directory)
    pub fn duplicate_active_tab(
        &mut self,
        config: &Config,
        runtime: Arc<Runtime>,
    ) -> Result<Option<TabId>> {
        let working_dir = self.active_tab().and_then(|t| t.get_cwd());

        if working_dir.is_some() || self.active_tab_id.is_some() {
            let id = self.next_tab_id;
            self.next_tab_id += 1;

            // Tab number is based on current count, not unique ID
            let tab_number = self.tabs.len() + 1;
            let tab = Tab::new(id, tab_number, config, runtime, working_dir)?;

            // Insert after active tab
            if let Some(active_id) = self.active_tab_id {
                if let Some(idx) = self.tabs.iter().position(|t| t.id == active_id) {
                    self.tabs.insert(idx + 1, tab);
                } else {
                    self.tabs.push(tab);
                }
            } else {
                self.tabs.push(tab);
            }

            self.active_tab_id = Some(id);
            Ok(Some(id))
        } else {
            Ok(None)
        }
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

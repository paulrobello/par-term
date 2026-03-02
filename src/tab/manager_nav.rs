//! Tab navigation methods for [`TabManager`].
//!
//! Split from `tab/manager.rs` to keep that file under 500 lines.
//! Contains all tab-switching and tab-reordering operations.

use super::TabId;
use super::manager::TabManager;
// Note: TabManager fields (tabs, active_tab_id) are pub(super) — visible to crate::tab,
// which includes this sibling module.

impl TabManager {
    /// Switch to a tab by ID
    pub fn switch_to(&mut self, id: TabId) {
        if self.tabs.iter().any(|t| t.id == id) {
            // Clear activity indicator when switching to tab
            if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                tab.activity.has_activity = false;
            }
            self.set_active_tab(Some(id));
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
}

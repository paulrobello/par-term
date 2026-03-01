//! Window arrangement types and manager for saving/restoring window layouts
//!
//! Arrangements capture the positions, sizes, and tab CWDs of all windows
//! so they can be restored later. Monitor-aware to handle external monitor
//! disconnect/reconnect scenarios.
//!
//! # Shared types
//!
//! [`TabSnapshot`] is defined in `par-term-config::snapshot_types` and re-exported
//! here so that callers using `par_term_settings_ui::arrangements::TabSnapshot` see
//! no change. The session module (`src/session`) also imports the same type directly
//! from `par_term_config`, eliminating the previous duplication.

// Re-export TabSnapshot from par-term-config so existing
// `use arrangements::TabSnapshot` paths keep working unchanged.
pub use par_term_config::snapshot_types::TabSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for an arrangement
pub type ArrangementId = Uuid;

/// Information about a monitor at capture time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    /// Monitor name (primary matching key, e.g. "DELL U2720Q")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Monitor index (fallback matching)
    #[serde(default)]
    pub index: usize,

    /// Monitor position in virtual screen coordinates (physical pixels)
    #[serde(default)]
    pub position: (i32, i32),

    /// Monitor size in physical pixels
    #[serde(default)]
    pub size: (u32, u32),

    /// DPI scale factor at capture time (e.g. 2.0 for Retina/HiDPI)
    /// Used to interpret position_relative and window size in WindowSnapshot.
    #[serde(default = "default_scale_factor", skip_serializing_if = "is_one")]
    pub scale_factor: f64,
}

fn default_scale_factor() -> f64 {
    1.0
}

fn is_one(v: &f64) -> bool {
    (*v - 1.0).abs() < f64::EPSILON
}

/// Snapshot of a single window's state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSnapshot {
    /// Monitor this window was on
    pub monitor: MonitorInfo,

    /// Position relative to monitor origin (portable across setups)
    pub position_relative: (i32, i32),

    /// Inner window size in logical pixels (scale-factor-independent)
    pub size: (u32, u32),

    /// Tabs in this window
    pub tabs: Vec<TabSnapshot>,

    /// Index of the active tab
    #[serde(default)]
    pub active_tab_index: usize,
}

/// A saved window arrangement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowArrangement {
    /// Unique identifier
    pub id: ArrangementId,

    /// Display name for the arrangement
    pub name: String,

    /// All monitors present at capture time
    pub monitor_layout: Vec<MonitorInfo>,

    /// All windows in this arrangement
    pub windows: Vec<WindowSnapshot>,

    /// ISO 8601 timestamp when the arrangement was created
    #[serde(default)]
    pub created_at: String,

    /// Display order
    #[serde(default)]
    pub order: usize,
}

/// Manages a collection of saved window arrangements
#[derive(Debug, Clone, Default)]
pub struct ArrangementManager {
    /// All arrangements indexed by ID
    arrangements: HashMap<ArrangementId, WindowArrangement>,

    /// Ordered list of arrangement IDs for display
    order: Vec<ArrangementId>,
}

impl ArrangementManager {
    /// Create a new empty arrangement manager
    pub fn new() -> Self {
        Self {
            arrangements: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Create a manager from a list of arrangements
    pub fn from_arrangements(arrangements: Vec<WindowArrangement>) -> Self {
        let mut manager = Self::new();
        for arrangement in arrangements {
            manager.add(arrangement);
        }
        manager.sort_by_order();
        manager
    }

    /// Add an arrangement to the manager
    pub fn add(&mut self, arrangement: WindowArrangement) {
        let id = arrangement.id;
        if !self.order.contains(&id) {
            self.order.push(id);
        }
        self.arrangements.insert(id, arrangement);
    }

    /// Get an arrangement by ID
    pub fn get(&self, id: &ArrangementId) -> Option<&WindowArrangement> {
        self.arrangements.get(id)
    }

    /// Get a mutable reference to an arrangement by ID
    pub fn get_mut(&mut self, id: &ArrangementId) -> Option<&mut WindowArrangement> {
        self.arrangements.get_mut(id)
    }

    /// Update an arrangement (replaces if exists)
    pub fn update(&mut self, arrangement: WindowArrangement) {
        let id = arrangement.id;
        if self.arrangements.contains_key(&id) {
            self.arrangements.insert(id, arrangement);
        }
    }

    /// Remove an arrangement by ID
    pub fn remove(&mut self, id: &ArrangementId) -> Option<WindowArrangement> {
        self.order.retain(|aid| aid != id);
        self.arrangements.remove(id)
    }

    /// Get all arrangements in display order
    pub fn arrangements_ordered(&self) -> Vec<&WindowArrangement> {
        self.order
            .iter()
            .filter_map(|id| self.arrangements.get(id))
            .collect()
    }

    /// Get all arrangements as a vector (for serialization)
    pub fn to_vec(&self) -> Vec<WindowArrangement> {
        self.arrangements_ordered().into_iter().cloned().collect()
    }

    /// Get the number of arrangements
    pub fn len(&self) -> usize {
        self.arrangements.len()
    }

    /// Check if there are no arrangements
    pub fn is_empty(&self) -> bool {
        self.arrangements.is_empty()
    }

    /// Find an arrangement by name (case-insensitive)
    pub fn find_by_name(&self, name: &str) -> Option<&WindowArrangement> {
        let lower = name.to_lowercase();
        self.arrangements
            .values()
            .find(|a| a.name.to_lowercase() == lower)
    }

    /// Move an arrangement earlier in the order (towards index 0)
    pub fn move_up(&mut self, id: &ArrangementId) {
        if let Some(pos) = self.order.iter().position(|aid| aid == id)
            && pos > 0
        {
            self.order.swap(pos, pos - 1);
            self.update_orders();
        }
    }

    /// Move an arrangement later in the order (towards the end)
    pub fn move_down(&mut self, id: &ArrangementId) {
        if let Some(pos) = self.order.iter().position(|aid| aid == id)
            && pos < self.order.len() - 1
        {
            self.order.swap(pos, pos + 1);
            self.update_orders();
        }
    }

    /// Sort arrangements by their order field
    fn sort_by_order(&mut self) {
        self.order.sort_by_key(|id| {
            self.arrangements
                .get(id)
                .map(|a| a.order)
                .unwrap_or(usize::MAX)
        });
    }

    /// Update the order field of all arrangements to match their position
    fn update_orders(&mut self) {
        for (i, id) in self.order.iter().enumerate() {
            if let Some(arrangement) = self.arrangements.get_mut(id) {
                arrangement.order = i;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_arrangement(name: &str, order: usize) -> WindowArrangement {
        WindowArrangement {
            id: Uuid::new_v4(),
            name: name.to_string(),
            monitor_layout: Vec::new(),
            windows: Vec::new(),
            created_at: String::new(),
            order,
        }
    }

    #[test]
    fn test_manager_basic_operations() {
        let mut manager = ArrangementManager::new();
        assert!(manager.is_empty());

        let arr = make_arrangement("Test", 0);
        let id = arr.id;
        manager.add(arr);

        assert_eq!(manager.len(), 1);
        assert!(manager.get(&id).is_some());
        assert_eq!(manager.get(&id).unwrap().name, "Test");

        let removed = manager.remove(&id);
        assert!(removed.is_some());
        assert!(manager.is_empty());
    }

    #[test]
    fn test_manager_ordering() {
        let mut manager = ArrangementManager::new();

        let a1 = make_arrangement("First", 0);
        let a2 = make_arrangement("Second", 1);
        let a3 = make_arrangement("Third", 2);

        let id1 = a1.id;
        let id2 = a2.id;
        let id3 = a3.id;

        manager.add(a1);
        manager.add(a2);
        manager.add(a3);

        let ordered = manager.arrangements_ordered();
        assert_eq!(ordered.len(), 3);
        assert_eq!(ordered[0].id, id1);
        assert_eq!(ordered[1].id, id2);
        assert_eq!(ordered[2].id, id3);

        // Move second to first position
        manager.move_up(&id2);
        let ordered = manager.arrangements_ordered();
        assert_eq!(ordered[0].id, id2);
        assert_eq!(ordered[1].id, id1);

        // Move second (now first) down
        manager.move_down(&id2);
        let ordered = manager.arrangements_ordered();
        assert_eq!(ordered[0].id, id1);
        assert_eq!(ordered[1].id, id2);
    }

    #[test]
    fn test_find_by_name() {
        let mut manager = ArrangementManager::new();
        manager.add(make_arrangement("Work Setup", 0));
        manager.add(make_arrangement("Home Setup", 1));

        assert!(manager.find_by_name("work setup").is_some());
        assert!(manager.find_by_name("HOME SETUP").is_some());
        assert!(manager.find_by_name("nonexistent").is_none());
    }

    #[test]
    fn test_serialization() {
        let arr = WindowArrangement {
            id: Uuid::new_v4(),
            name: "Test".to_string(),
            monitor_layout: vec![MonitorInfo {
                name: Some("DELL U2720Q".to_string()),
                index: 0,
                position: (0, 0),
                size: (2560, 1440),
                scale_factor: 1.0,
            }],
            windows: vec![WindowSnapshot {
                monitor: MonitorInfo {
                    name: Some("DELL U2720Q".to_string()),
                    index: 0,
                    position: (0, 0),
                    size: (2560, 1440),
                    scale_factor: 1.0,
                },
                position_relative: (100, 200),
                size: (800, 600),
                tabs: vec![TabSnapshot {
                    cwd: Some("/home/user".to_string()),
                    title: "bash".to_string(),
                    custom_color: None,
                    user_title: None,
                    custom_icon: None,
                }],
                active_tab_index: 0,
            }],
            created_at: "2024-01-01T00:00:00Z".to_string(),
            order: 0,
        };

        let yaml = serde_yaml_ng::to_string(&arr).unwrap();
        let deserialized: WindowArrangement = serde_yaml_ng::from_str(&yaml).unwrap();

        assert_eq!(deserialized.id, arr.id);
        assert_eq!(deserialized.name, arr.name);
        assert_eq!(deserialized.windows.len(), 1);
        assert_eq!(deserialized.windows[0].tabs.len(), 1);
        assert_eq!(
            deserialized.windows[0].tabs[0].cwd,
            Some("/home/user".to_string())
        );
    }
}

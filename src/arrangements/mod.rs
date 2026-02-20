//! Window arrangement types and manager for saving/restoring window layouts
//!
//! Arrangements capture the positions, sizes, and tab CWDs of all windows
//! so they can be restored later. Monitor-aware to handle external monitor
//! disconnect/reconnect scenarios.
//!
//! Data types are defined in `par-term-settings-ui` and re-exported here.

pub mod capture;
pub mod restore;
pub mod storage;

// Re-export all arrangement types from the settings-ui crate so the rest of the
// main crate can continue using `crate::arrangements::*` unchanged.
pub use par_term_settings_ui::arrangements::{
    ArrangementId, ArrangementManager, MonitorInfo, TabSnapshot, WindowArrangement, WindowSnapshot,
};

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

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
            }],
            windows: vec![WindowSnapshot {
                monitor: MonitorInfo {
                    name: Some("DELL U2720Q".to_string()),
                    index: 0,
                    position: (0, 0),
                    size: (2560, 1440),
                },
                position_relative: (100, 200),
                size: (800, 600),
                tabs: vec![TabSnapshot {
                    cwd: Some("/home/user".to_string()),
                    title: "bash".to_string(),
                    custom_color: None,
                    user_title: None,
                }],
                active_tab_index: 0,
            }],
            created_at: "2024-01-01T00:00:00Z".to_string(),
            order: 0,
        };

        let yaml = serde_yaml::to_string(&arr).unwrap();
        let deserialized: WindowArrangement = serde_yaml::from_str(&yaml).unwrap();

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

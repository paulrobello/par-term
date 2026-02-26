//! Storage utilities for arrangement persistence
//!
//! Arrangements are stored in `~/.config/par-term/arrangements.yaml`

use super::{ArrangementManager, WindowArrangement};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// Get the default arrangements file path
pub fn arrangements_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("par-term")
        .join("arrangements.yaml")
}

/// Load arrangements from the default location
pub fn load_arrangements() -> Result<ArrangementManager> {
    load_arrangements_from(arrangements_path())
}

/// Load arrangements from a specific file
pub fn load_arrangements_from(path: PathBuf) -> Result<ArrangementManager> {
    crate::debug_info!("ARRANGEMENT", "Loading arrangements from {:?}", path);
    if !path.exists() {
        crate::debug_info!(
            "ARRANGEMENT",
            "No arrangements file found at {:?}, starting with empty arrangements",
            path
        );
        return Ok(ArrangementManager::new());
    }

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read arrangements from {:?}", path))?;

    crate::debug_info!(
        "ARRANGEMENT",
        "Read {} bytes from arrangements file",
        contents.len()
    );

    if contents.trim().is_empty() {
        crate::debug_info!(
            "ARRANGEMENT",
            "Arrangements file is empty, starting with empty arrangements"
        );
        return Ok(ArrangementManager::new());
    }

    let arrangements: Vec<WindowArrangement> = serde_yml::from_str(&contents)
        .with_context(|| format!("Failed to parse arrangements from {:?}", path))?;

    crate::debug_info!(
        "ARRANGEMENT",
        "Parsed {} arrangements from {:?}",
        arrangements.len(),
        path
    );
    for a in &arrangements {
        crate::debug_info!(
            "ARRANGEMENT",
            "  - {}: {} ({} windows)",
            a.id,
            a.name,
            a.windows.len()
        );
    }
    Ok(ArrangementManager::from_arrangements(arrangements))
}

/// Save arrangements to the default location
pub fn save_arrangements(manager: &ArrangementManager) -> Result<()> {
    save_arrangements_to(manager, arrangements_path())
}

/// Save arrangements to a specific file
pub fn save_arrangements_to(manager: &ArrangementManager, path: PathBuf) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory {:?}", parent))?;
    }

    let arrangements = manager.to_vec();
    let contents =
        serde_yml::to_string(&arrangements).context("Failed to serialize arrangements")?;

    std::fs::write(&path, contents)
        .with_context(|| format!("Failed to write arrangements to {:?}", path))?;

    log::info!("Saved {} arrangements to {:?}", arrangements.len(), path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arrangements::{MonitorInfo, TabSnapshot, WindowArrangement, WindowSnapshot};
    use tempfile::tempdir;
    use uuid::Uuid;

    #[test]
    fn test_load_nonexistent_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("nonexistent.yaml");

        let manager = load_arrangements_from(path).unwrap();
        assert!(manager.is_empty());
    }

    #[test]
    fn test_load_empty_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("empty.yaml");
        std::fs::write(&path, "").unwrap();

        let manager = load_arrangements_from(path).unwrap();
        assert!(manager.is_empty());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("arrangements.yaml");

        let mut manager = ArrangementManager::new();
        manager.add(WindowArrangement {
            id: Uuid::new_v4(),
            name: "Work Setup".to_string(),
            monitor_layout: vec![MonitorInfo {
                name: Some("Main".to_string()),
                index: 0,
                position: (0, 0),
                size: (1920, 1080),
                scale_factor: 1.0,
            }],
            windows: vec![WindowSnapshot {
                monitor: MonitorInfo {
                    name: Some("Main".to_string()),
                    index: 0,
                    position: (0, 0),
                    size: (1920, 1080),
                    scale_factor: 1.0,
                },
                position_relative: (100, 100),
                size: (800, 600),
                tabs: vec![TabSnapshot {
                    cwd: Some("/home/user/work".to_string()),
                    title: "work".to_string(),
                    custom_color: None,
                    user_title: None,
                    custom_icon: None,
                }],
                active_tab_index: 0,
            }],
            created_at: "2024-01-01T00:00:00Z".to_string(),
            order: 0,
        });

        save_arrangements_to(&manager, path.clone()).unwrap();

        let loaded = load_arrangements_from(path).unwrap();
        assert_eq!(loaded.len(), 1);

        let arrangements = loaded.arrangements_ordered();
        assert_eq!(arrangements[0].name, "Work Setup");
        assert_eq!(arrangements[0].windows.len(), 1);
        assert_eq!(arrangements[0].windows[0].tabs.len(), 1);
        assert_eq!(
            arrangements[0].windows[0].tabs[0].cwd,
            Some("/home/user/work".to_string())
        );
    }

    #[test]
    fn test_roundtrip_preserves_custom_tab_properties() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("arrangements.yaml");

        let mut manager = ArrangementManager::new();
        manager.add(WindowArrangement {
            id: Uuid::new_v4(),
            name: "Custom Props".to_string(),
            monitor_layout: vec![MonitorInfo {
                name: Some("Main".to_string()),
                index: 0,
                position: (0, 0),
                size: (1920, 1080),
                scale_factor: 1.0,
            }],
            windows: vec![WindowSnapshot {
                monitor: MonitorInfo {
                    name: Some("Main".to_string()),
                    index: 0,
                    position: (0, 0),
                    size: (1920, 1080),
                    scale_factor: 1.0,
                },
                position_relative: (0, 0),
                size: (800, 600),
                tabs: vec![
                    TabSnapshot {
                        cwd: Some("/home/user".to_string()),
                        title: "My Custom Tab".to_string(),
                        custom_color: Some([255, 128, 0]),
                        user_title: Some("My Custom Tab".to_string()),
                        custom_icon: Some("🔥".to_string()),
                    },
                    TabSnapshot {
                        cwd: Some("/tmp".to_string()),
                        title: "Tab 2".to_string(),
                        custom_color: None,
                        user_title: None,
                        custom_icon: Some("📁".to_string()),
                    },
                    TabSnapshot {
                        cwd: None,
                        title: "Colored Only".to_string(),
                        custom_color: Some([0, 200, 100]),
                        user_title: None,
                        custom_icon: None,
                    },
                ],
                active_tab_index: 1,
            }],
            created_at: "2024-01-01T00:00:00Z".to_string(),
            order: 0,
        });

        save_arrangements_to(&manager, path.clone()).unwrap();

        let loaded = load_arrangements_from(path).unwrap();
        let arrangements = loaded.arrangements_ordered();
        let tabs = &arrangements[0].windows[0].tabs;

        // Tab 0: all custom properties set
        assert_eq!(tabs[0].custom_color, Some([255, 128, 0]));
        assert_eq!(tabs[0].user_title, Some("My Custom Tab".to_string()));
        assert_eq!(tabs[0].custom_icon, Some("🔥".to_string()));

        // Tab 1: only custom icon
        assert_eq!(tabs[1].custom_color, None);
        assert_eq!(tabs[1].user_title, None);
        assert_eq!(tabs[1].custom_icon, Some("📁".to_string()));

        // Tab 2: only custom color
        assert_eq!(tabs[2].custom_color, Some([0, 200, 100]));
        assert_eq!(tabs[2].user_title, None);
        assert_eq!(tabs[2].custom_icon, None);
    }

    #[test]
    fn test_save_creates_parent_directory() {
        let temp = tempdir().unwrap();
        let path = temp
            .path()
            .join("nested")
            .join("dir")
            .join("arrangements.yaml");

        let manager = ArrangementManager::new();
        save_arrangements_to(&manager, path.clone()).unwrap();

        assert!(path.exists());
    }

    #[test]
    fn test_load_corrupt_file_returns_error() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("corrupt.yaml");
        std::fs::write(&path, "not: valid: yaml: [[[").unwrap();

        let result = load_arrangements_from(path);
        assert!(result.is_err());
    }
}

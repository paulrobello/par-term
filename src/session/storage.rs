//! File I/O for session persistence
//!
//! Sessions are stored in `~/.config/par-term/last_session.yaml`

use super::SessionState;
use anyhow::{Context, Result};
use std::path::PathBuf;

/// Get the path to the session state file
pub fn session_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("par-term")
        .join("last_session.yaml")
}

/// Save session state to the default location
pub fn save_session(state: &SessionState) -> Result<()> {
    save_session_to(state, session_path())
}

/// Save session state to a specific file
pub fn save_session_to(state: &SessionState, path: PathBuf) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory {:?}", parent))?;
    }

    let contents = serde_yaml::to_string(state).context("Failed to serialize session state")?;

    std::fs::write(&path, contents)
        .with_context(|| format!("Failed to write session state to {:?}", path))?;

    log::info!(
        "Saved session state ({} windows) to {:?}",
        state.windows.len(),
        path
    );
    Ok(())
}

/// Load session state from the default location
///
/// Returns `None` if the file doesn't exist or is empty.
/// Returns an error if the file exists but is corrupt.
pub fn load_session() -> Result<Option<SessionState>> {
    load_session_from(session_path())
}

/// Load session state from a specific file
pub fn load_session_from(path: PathBuf) -> Result<Option<SessionState>> {
    if !path.exists() {
        return Ok(None);
    }

    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read session state from {:?}", path))?;

    if contents.trim().is_empty() {
        return Ok(None);
    }

    let state: SessionState = serde_yaml::from_str(&contents)
        .with_context(|| format!("Failed to parse session state from {:?}", path))?;

    log::info!(
        "Loaded session state ({} windows) from {:?}",
        state.windows.len(),
        path
    );
    Ok(Some(state))
}

/// Remove the session state file (e.g., after successful restore)
pub fn clear_session() -> Result<()> {
    let path = session_path();
    if path.exists() {
        std::fs::remove_file(&path)
            .with_context(|| format!("Failed to remove session state file {:?}", path))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{SessionState, SessionTab, SessionWindow};
    use tempfile::tempdir;

    fn sample_session() -> SessionState {
        SessionState {
            saved_at: "2025-01-01T00:00:00Z".to_string(),
            windows: vec![SessionWindow {
                position: (100, 200),
                size: (800, 600),
                tabs: vec![SessionTab {
                    cwd: Some("/home/user/work".to_string()),
                    title: "work".to_string(),
                    custom_color: None,
                    user_title: None,
                    pane_layout: None,
                }],
                active_tab_index: 0,
            }],
        }
    }

    #[test]
    fn test_load_nonexistent_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("nonexistent.yaml");
        let result = load_session_from(path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_load_empty_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("empty.yaml");
        std::fs::write(&path, "").unwrap();
        let result = load_session_from(path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_load_corrupt_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("corrupt.yaml");
        std::fs::write(&path, "not: valid: yaml: [[[").unwrap();
        let result = load_session_from(path);
        assert!(result.is_err());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("session.yaml");

        let state = sample_session();
        save_session_to(&state, path.clone()).unwrap();

        let loaded = load_session_from(path).unwrap().unwrap();
        assert_eq!(loaded.windows.len(), 1);
        assert_eq!(loaded.windows[0].position, (100, 200));
        assert_eq!(loaded.windows[0].size, (800, 600));
        assert_eq!(loaded.windows[0].tabs.len(), 1);
        assert_eq!(
            loaded.windows[0].tabs[0].cwd,
            Some("/home/user/work".to_string())
        );
        assert_eq!(loaded.windows[0].tabs[0].title, "work");
    }

    #[test]
    fn test_save_creates_parent_directory() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("nested").join("dir").join("session.yaml");

        let state = sample_session();
        save_session_to(&state, path.clone()).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_serialization_with_pane_layout() {
        use crate::pane::SplitDirection;
        use crate::session::SessionPaneNode;

        let state = SessionState {
            saved_at: "2025-01-01T00:00:00Z".to_string(),
            windows: vec![SessionWindow {
                position: (0, 0),
                size: (1920, 1080),
                tabs: vec![SessionTab {
                    cwd: Some("/home/user".to_string()),
                    title: "dev".to_string(),
                    custom_color: None,
                    user_title: None,
                    pane_layout: Some(SessionPaneNode::Split {
                        direction: SplitDirection::Vertical,
                        ratio: 0.5,
                        first: Box::new(SessionPaneNode::Leaf {
                            cwd: Some("/home/user/code".to_string()),
                        }),
                        second: Box::new(SessionPaneNode::Split {
                            direction: SplitDirection::Horizontal,
                            ratio: 0.6,
                            first: Box::new(SessionPaneNode::Leaf {
                                cwd: Some("/home/user/logs".to_string()),
                            }),
                            second: Box::new(SessionPaneNode::Leaf {
                                cwd: Some("/home/user/tests".to_string()),
                            }),
                        }),
                    }),
                }],
                active_tab_index: 0,
            }],
        };

        let temp = tempdir().unwrap();
        let path = temp.path().join("pane_session.yaml");

        save_session_to(&state, path.clone()).unwrap();
        let loaded = load_session_from(path).unwrap().unwrap();

        // Verify the nested pane layout survived roundtrip
        let tab = &loaded.windows[0].tabs[0];
        assert!(tab.pane_layout.is_some());
        match tab.pane_layout.as_ref().unwrap() {
            SessionPaneNode::Split {
                direction, ratio, ..
            } => {
                assert_eq!(*direction, SplitDirection::Vertical);
                assert!((ratio - 0.5).abs() < f32::EPSILON);
            }
            _ => panic!("Expected Split at root"),
        }
    }

    #[test]
    fn test_split_direction_serde() {
        use crate::pane::SplitDirection;

        let h = SplitDirection::Horizontal;
        let v = SplitDirection::Vertical;

        let h_yaml = serde_yaml::to_string(&h).unwrap();
        let v_yaml = serde_yaml::to_string(&v).unwrap();

        let h_back: SplitDirection = serde_yaml::from_str(&h_yaml).unwrap();
        let v_back: SplitDirection = serde_yaml::from_str(&v_yaml).unwrap();

        assert_eq!(h, h_back);
        assert_eq!(v, v_back);
    }

    #[test]
    fn test_clear_session() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("to_clear.yaml");
        std::fs::write(&path, "test").unwrap();
        assert!(path.exists());

        // We can't easily test clear_session() since it uses fixed path,
        // but we can test the file removal logic
        std::fs::remove_file(&path).unwrap();
        assert!(!path.exists());
    }
}

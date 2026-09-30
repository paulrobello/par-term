//! Helpers for restoring session state

use par_term_config::snapshot_types::{SessionPaneNode, TabSnapshot};
use std::path::Path;
use std::sync::Arc;

/// Apply a saved tab snapshot to a freshly created local tab: the pane tree
/// (split roots only — see `capture_tab_snapshot`), then the user title,
/// sole-pane title, color, and icon. Shared by session restore and
/// arrangement restore (PN11). The caller starts pane refresh tasks.
pub fn apply_tab_snapshot(
    tab: &mut crate::tab::Tab,
    snapshot: &TabSnapshot,
    config: &crate::config::Config,
    runtime: Arc<tokio::runtime::Runtime>,
) {
    if let Some(layout) = &snapshot.pane_layout
        && matches!(layout, SessionPaneNode::Split { .. })
    {
        tab.restore_pane_layout(layout, config, runtime);
    }
    if let Some(ref user_title) = snapshot.user_title {
        tab.set_title(user_title);
        tab.user_named = true;
    }
    if let Some(ref pane_title) = snapshot.pane_user_title {
        tab.restore_sole_pane_title(pane_title);
    }
    if let Some(color) = snapshot.custom_color {
        tab.set_custom_color(color);
    }
    if let Some(ref icon) = snapshot.custom_icon {
        tab.custom_icon = Some(icon.clone());
    }
}

/// Validate a working directory path, falling back to $HOME if invalid
pub fn validate_cwd(cwd: &Option<String>) -> Option<String> {
    if let Some(dir) = cwd {
        if Path::new(dir).is_dir() {
            return Some(dir.clone());
        }
        log::warn!(
            "Session restore: directory '{}' no longer exists, falling back to home",
            dir
        );
    }
    // Fall back to home directory
    dirs::home_dir().map(|p| p.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_cwd_existing_dir() {
        let temp_dir = std::env::temp_dir();
        let cwd = Some(temp_dir.to_string_lossy().to_string());
        let result = validate_cwd(&cwd);
        assert_eq!(result, Some(temp_dir.to_string_lossy().to_string()));
    }

    #[test]
    fn test_validate_cwd_missing_dir_falls_back_to_home() {
        let cwd = Some("/nonexistent/path/that/does/not/exist".to_string());
        let result = validate_cwd(&cwd);
        // Should fall back to home directory
        let home = dirs::home_dir().map(|p| p.to_string_lossy().to_string());
        assert_eq!(result, home);
    }

    #[test]
    fn test_validate_cwd_none_falls_back_to_home() {
        let result = validate_cwd(&None);
        let home = dirs::home_dir().map(|p| p.to_string_lossy().to_string());
        assert_eq!(result, home);
    }
}

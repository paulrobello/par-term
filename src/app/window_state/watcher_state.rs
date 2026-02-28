//! Watcher and observer handles for the window manager.
//!
//! Extracted from `WindowState` as part of the God Object decomposition (ARC-001).

use crate::config::watcher::ConfigWatcher;

/// State for file and request watchers.
pub(crate) struct WatcherState {
    /// Config file watcher for automatic reload (e.g., when user modifies config.yaml)
    pub(crate) config_watcher: Option<ConfigWatcher>,
    /// Watcher for `.config-update.json` written by the MCP server
    pub(crate) config_update_watcher: Option<ConfigWatcher>,
    /// Watcher for `.screenshot-request.json` written by the MCP server
    pub(crate) screenshot_request_watcher: Option<ConfigWatcher>,
}

impl Default for WatcherState {
    fn default() -> Self {
        Self {
            config_watcher: None,
            config_update_watcher: None,
            screenshot_request_watcher: None,
        }
    }
}

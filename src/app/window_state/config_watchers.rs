//! Config file watcher setup and polling for WindowState.
//!
//! Handles live config reload (YAML changes), config update channel polling,
//! and MCP screenshot request polling.

use crate::app::window_state::WindowState;
use crate::config::Config;
use par_term_mcp::{
    SCREENSHOT_REQUEST_FILENAME, SCREENSHOT_RESPONSE_FILENAME, TerminalScreenshotRequest,
    TerminalScreenshotResponse,
};

impl WindowState {
    /// Initialize the config file watcher for automatic reload.
    ///
    /// Watches `config.yaml` for changes so that when an ACP agent modifies
    /// the config, par-term can auto-reload shader and other settings.
    pub(crate) fn init_config_watcher(&mut self) {
        let config_path = Config::config_path();
        if !config_path.exists() {
            debug_info!("CONFIG", "Config file does not exist, skipping watcher");
            return;
        }
        match crate::config::watcher::ConfigWatcher::new(&config_path, 500) {
            Ok(watcher) => {
                debug_info!("CONFIG", "Config watcher initialized");
                self.watcher_state.config_watcher = Some(watcher);
            }
            Err(e) => {
                debug_info!("CONFIG", "Failed to initialize config watcher: {}", e);
            }
        }
    }

    /// Initialize the watcher for `.config-update.json` (MCP server config updates).
    ///
    /// The MCP server (spawned by the ACP agent) writes config updates to this
    /// file. We watch it, apply the updates in-memory, and delete it.
    pub(crate) fn init_config_update_watcher(&mut self) {
        let update_path = Config::config_dir().join(".config-update.json");

        // Create the file if it doesn't exist so the watcher can start
        if !update_path.exists() {
            if let Some(parent) = update_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&update_path, "");
        }

        match crate::config::watcher::ConfigWatcher::new(&update_path, 200) {
            Ok(watcher) => {
                debug_info!("CONFIG", "Config-update watcher initialized");
                self.watcher_state.config_update_watcher = Some(watcher);
            }
            Err(e) => {
                debug_info!(
                    "CONFIG",
                    "Failed to initialize config-update watcher: {}",
                    e
                );
            }
        }
    }

    /// Initialize the watcher for `.screenshot-request.json` (MCP screenshot tool).
    ///
    /// The MCP server writes screenshot requests to this file. We watch it,
    /// capture the current renderer output, write a response to
    /// `.screenshot-response.json`, and clear the request file.
    pub(crate) fn init_screenshot_request_watcher(&mut self) {
        let request_path = Config::config_dir().join(SCREENSHOT_REQUEST_FILENAME);

        if !request_path.exists() {
            if let Some(parent) = request_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&request_path, "");
        }

        let response_path = Config::config_dir().join(SCREENSHOT_RESPONSE_FILENAME);
        if !response_path.exists() {
            if let Some(parent) = response_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&response_path, "");
        }

        match crate::config::watcher::ConfigWatcher::new(&request_path, 100) {
            Ok(watcher) => {
                debug_info!("CONFIG", "Screenshot-request watcher initialized");
                self.watcher_state.screenshot_request_watcher = Some(watcher);
            }
            Err(e) => {
                debug_info!(
                    "CONFIG",
                    "Failed to initialize screenshot-request watcher: {}",
                    e
                );
            }
        }
    }

    /// Check for pending config update file changes (from MCP server).
    ///
    /// When the MCP server writes `.config-update.json`, this reads it,
    /// applies the updates in-memory, saves to disk, and removes the file.
    pub(crate) fn check_config_update_file(&mut self) {
        let Some(watcher) = &self.watcher_state.config_update_watcher else {
            return;
        };
        if watcher.try_recv().is_none() {
            return;
        }

        let update_path = Config::config_dir().join(".config-update.json");
        let content = match std::fs::read_to_string(&update_path) {
            Ok(c) if c.trim().is_empty() => return,
            Ok(c) => c,
            Err(e) => {
                log::warn!("CONFIG: failed to read config-update file: {e}");
                return;
            }
        };

        match serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(&content)
        {
            Ok(updates) => {
                log::info!(
                    "CONFIG: applying MCP config update ({} keys): {:?}",
                    updates.len(),
                    updates
                );
                if let Err(e) = self.apply_agent_config_updates(&updates) {
                    log::error!("CONFIG: MCP config update failed: {e}");
                } else {
                    self.config_changed_by_agent = true;
                }
                self.focus_state.needs_redraw = true;
            }
            Err(e) => {
                log::error!("CONFIG: invalid JSON in config-update file: {e}");
            }
        }

        // Clear the file so we don't re-process it
        let _ = std::fs::write(&update_path, "");
    }

    /// Check for pending screenshot request file changes (from MCP server).
    ///
    /// When the MCP server writes `.screenshot-request.json`, this captures the
    /// active terminal renderer output and writes a response to
    /// `.screenshot-response.json`.
    pub(crate) fn check_screenshot_request_file(&mut self) {
        let Some(watcher) = &self.watcher_state.screenshot_request_watcher else {
            return;
        };
        if watcher.try_recv().is_none() {
            return;
        }

        let request_path = Config::config_dir().join(SCREENSHOT_REQUEST_FILENAME);
        let response_path = Config::config_dir().join(SCREENSHOT_RESPONSE_FILENAME);

        let content = match std::fs::read_to_string(&request_path) {
            Ok(c) if c.trim().is_empty() => return,
            Ok(c) => c,
            Err(e) => {
                log::warn!("ACP screenshot: failed to read request file: {e}");
                return;
            }
        };

        let request = match serde_json::from_str::<TerminalScreenshotRequest>(&content) {
            Ok(req) => req,
            Err(e) => {
                log::error!("ACP screenshot: invalid JSON in request file: {e}");
                let _ = std::fs::write(&request_path, "");
                return;
            }
        };

        let response = match self.capture_terminal_screenshot_mcp_response(&request.request_id) {
            Ok(resp) => resp,
            Err(e) => TerminalScreenshotResponse {
                request_id: request.request_id.clone(),
                ok: false,
                error: Some(e),
                mime_type: None,
                data_base64: None,
                width: None,
                height: None,
            },
        };

        match serde_json::to_vec_pretty(&response) {
            Ok(bytes) => {
                let tmp = response_path.with_extension("json.tmp");
                if let Err(e) =
                    std::fs::write(&tmp, &bytes).and_then(|_| std::fs::rename(&tmp, &response_path))
                {
                    let _ = std::fs::remove_file(&tmp);
                    log::error!(
                        "ACP screenshot: failed to write response {}: {}",
                        response_path.display(),
                        e
                    );
                }
            }
            Err(e) => {
                log::error!("ACP screenshot: failed to serialize response: {e}");
            }
        }

        // Clear request file so it is processed only once.
        let _ = std::fs::write(&request_path, "");
    }
}

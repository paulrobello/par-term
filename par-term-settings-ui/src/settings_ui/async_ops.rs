//! Async background operations for SettingsUI.
//!
//! Contains: shader install, shader install polling, self-update, self-update
//! polling, plugin add/update/remove, plugin operation polling.

use crate::{PluginGitOutcome, ShaderInstallResult, UpdateResult};

use super::SettingsUI;

impl SettingsUI {
    /// Begin shader install asynchronously with optional force overwrite.
    /// The caller must provide a function that performs the actual installation.
    pub fn start_shader_install_with<F>(&mut self, force_overwrite: bool, install_fn: F)
    where
        F: FnOnce(bool) -> Result<ShaderInstallResult, String> + Send + 'static,
    {
        use std::sync::mpsc;

        if self.shader_installing {
            return;
        }

        self.shader_error = None;
        self.shader_status = Some(if force_overwrite {
            "Reinstalling shaders (overwriting modified files)...".to_string()
        } else {
            "Reinstalling shaders...".to_string()
        });
        self.shader_installing = true;

        let (tx, rx) = mpsc::channel();
        self.shader_install_receiver = Some(rx);

        std::thread::spawn(move || {
            let result = install_fn(force_overwrite);
            let _ = tx.send(result);
        });
    }

    /// Poll for completion of async shader install.
    pub fn poll_shader_install_status(&mut self) {
        if let Some(receiver) = &self.shader_install_receiver
            && let Ok(result) = receiver.try_recv()
        {
            self.shader_installing = false;
            self.shader_install_receiver = None;
            match result {
                Ok(res) => {
                    let detail = if res.skipped > 0 {
                        format!(
                            "Installed {} shaders ({} skipped, {} removed)",
                            res.installed, res.skipped, res.removed
                        )
                    } else {
                        format!(
                            "Installed {} shaders ({} removed)",
                            res.installed, res.removed
                        )
                    };
                    self.shader_status = Some(detail);
                    self.shader_error = None;
                    self.config
                        .integrations
                        .integration_versions
                        .shaders_installed_version = Some(self.app_version.to_string());
                }
                Err(e) => {
                    self.shader_error = Some(e);
                    self.shader_status = None;
                }
            }
        }
    }

    /// Begin self-update asynchronously.
    /// The caller must provide a function that performs the actual update.
    pub fn start_self_update_with<F>(&mut self, version: String, update_fn: F)
    where
        F: FnOnce(&str) -> Result<UpdateResult, String> + Send + 'static,
    {
        use std::sync::mpsc;

        if self.update_installing {
            return;
        }

        self.update_status = Some("Downloading and installing update...".to_string());
        self.update_result = None;
        self.update_installing = true;

        let (tx, rx) = mpsc::channel();
        self.update_install_receiver = Some(rx);

        std::thread::spawn(move || {
            let result = update_fn(&version);
            let _ = tx.send(result);
        });
    }

    /// Poll for completion of async self-update.
    pub fn poll_update_install_status(&mut self) {
        if let Some(receiver) = &self.update_install_receiver
            && let Ok(result) = receiver.try_recv()
        {
            self.update_installing = false;
            self.update_install_receiver = None;
            match &result {
                Ok(res) => {
                    self.update_status = Some(format!(
                        "Update installed! Restart par-term to use v{}",
                        res.new_version
                    ));
                }
                Err(e) => {
                    self.update_status = Some(format!("Update failed: {}", e));
                }
            }
            self.update_result = Some(result);
        }
    }

    /// Begin a plugin git operation (add / update fetch / apply / remove)
    /// on a background thread. The git layer enforces the prompt-free
    /// environment and a per-invocation timeout; one operation runs at a
    /// time.
    pub fn start_plugin_git_op<F>(&mut self, status: &str, op_fn: F)
    where
        F: FnOnce() -> Result<PluginGitOutcome, String> + Send + 'static,
    {
        use std::sync::mpsc;

        if self.automation_tab.plugin_git_busy {
            return;
        }

        self.automation_tab.plugin_git_busy = true;
        self.automation_tab.plugin_git_message = Some(status.to_string());

        let (tx, rx) = mpsc::channel();
        self.automation_tab.plugin_git_receiver = Some(rx);

        std::thread::spawn(move || {
            let _ = tx.send(op_fn());
        });
    }

    /// Poll the in-flight plugin git operation; returns true when one
    /// completed this frame (the Plugins section rescans its directories
    /// then, so the change is reflected immediately).
    pub fn poll_plugin_git_op(&mut self) -> bool {
        if let Some(receiver) = &self.automation_tab.plugin_git_receiver
            && let Ok(result) = receiver.try_recv()
        {
            self.automation_tab.plugin_git_busy = false;
            self.automation_tab.plugin_git_receiver = None;
            match result {
                Ok(PluginGitOutcome::Done(message)) => {
                    self.automation_tab.plugin_git_message = Some(message);
                }
                Ok(PluginGitOutcome::UpdateReady { id, preview }) => {
                    self.automation_tab.plugin_git_message =
                        Some(format!("Update fetched for {id} — review the diff below."));
                    self.automation_tab.plugin_update_preview = Some((id, preview));
                }
                Err(e) => {
                    self.automation_tab.plugin_git_message = Some(format!("Error: {e}"));
                }
            }
            return true;
        }
        false
    }
}

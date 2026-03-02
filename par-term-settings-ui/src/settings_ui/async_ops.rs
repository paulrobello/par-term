//! Async background operations for SettingsUI.
//!
//! Contains: shader install, shader install polling, self-update, self-update polling.

use crate::{ShaderInstallResult, UpdateResult};

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
                    self.config.integration_versions.shaders_installed_version =
                        Some(self.app_version.to_string());
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
}

//! Integrations welcome dialog action handlers.
//!
//! Contains [`WindowState::handle_integrations_response`], dispatching all
//! responses from the integrations dialog: shader install, shell integration
//! install, skip, never-ask, and shader overwrite conflict resolution.

use crate::app::window_state::WindowState;
use crate::config::ShaderInstallPrompt;
use crate::integrations_ui::IntegrationsResponse;

impl WindowState {
    /// Handle responses from the integrations welcome dialog
    pub(crate) fn handle_integrations_response(&mut self, response: &IntegrationsResponse) {
        // Nothing to do if dialog wasn't interacted with
        if !response.install_shaders
            && !response.install_shell_integration
            && !response.skipped
            && !response.never_ask
            && !response.closed
            && response.shader_conflict_action.is_none()
        {
            return;
        }

        let current_version = env!("CARGO_PKG_VERSION").to_string();

        // Determine install intent and overwrite behavior
        let mut install_shaders = false;
        let mut install_shell_integration = false;
        let mut force_overwrite_modified_shaders = false;
        let mut triggered_install = false;

        // If we're waiting on a shader overwrite decision, handle that first
        if let Some(action) = response.shader_conflict_action {
            triggered_install = true;
            install_shaders = self.overlay_ui.integrations_ui.pending_install_shaders;
            install_shell_integration = self
                .overlay_ui
                .integrations_ui
                .pending_install_shell_integration;

            match action {
                crate::integrations_ui::ShaderConflictAction::Overwrite => {
                    force_overwrite_modified_shaders = true;
                }
                crate::integrations_ui::ShaderConflictAction::SkipModified => {
                    force_overwrite_modified_shaders = false;
                }
                crate::integrations_ui::ShaderConflictAction::Cancel => {
                    // Reset pending state and exit without installing
                    self.overlay_ui.integrations_ui.awaiting_shader_overwrite = false;
                    self.overlay_ui.integrations_ui.shader_conflicts.clear();
                    self.overlay_ui.integrations_ui.pending_install_shaders = false;
                    self.overlay_ui
                        .integrations_ui
                        .pending_install_shell_integration = false;
                    self.overlay_ui.integrations_ui.error_message = None;
                    self.overlay_ui.integrations_ui.success_message = None;
                    self.focus_state.needs_redraw = true;
                    return;
                }
            }

            // Clear the conflict prompt regardless of choice
            self.overlay_ui.integrations_ui.awaiting_shader_overwrite = false;
            self.overlay_ui.integrations_ui.shader_conflicts.clear();
            self.overlay_ui.integrations_ui.error_message = None;
            self.overlay_ui.integrations_ui.success_message = None;
            self.overlay_ui.integrations_ui.installing = false;
        } else if response.install_shaders || response.install_shell_integration {
            triggered_install = true;
            install_shaders = response.install_shaders;
            install_shell_integration = response.install_shell_integration;

            if install_shaders {
                match crate::shader_installer::detect_modified_bundled_shaders() {
                    Ok(conflicts) if !conflicts.is_empty() => {
                        log::info!(
                            "Detected {} modified bundled shaders; prompting for overwrite",
                            conflicts.len()
                        );
                        self.overlay_ui.integrations_ui.awaiting_shader_overwrite = true;
                        self.overlay_ui.integrations_ui.shader_conflicts = conflicts;
                        self.overlay_ui.integrations_ui.pending_install_shaders = install_shaders;
                        self.overlay_ui
                            .integrations_ui
                            .pending_install_shell_integration = install_shell_integration;
                        self.overlay_ui.integrations_ui.installing = false;
                        self.overlay_ui.integrations_ui.error_message = None;
                        self.overlay_ui.integrations_ui.success_message = None;
                        self.focus_state.needs_redraw = true;
                        return; // Wait for user decision
                    }
                    Ok(_) => {}
                    Err(e) => {
                        log::warn!(
                            "Unable to check existing shaders for modifications: {}. Proceeding without overwrite prompt.",
                            e
                        );
                    }
                }
            }
        }

        // Handle "Install Selected" - user wants to install one or both integrations
        if triggered_install {
            log::info!(
                "User requested installations: shaders={}, shell_integration={}, overwrite_modified={}",
                install_shaders,
                install_shell_integration,
                force_overwrite_modified_shaders
            );

            let mut success_parts = Vec::new();
            let mut error_parts = Vec::new();

            // Install shaders if requested
            if install_shaders {
                self.overlay_ui
                    .integrations_ui
                    .set_installing("Installing shaders...");
                self.focus_state.needs_redraw = true;
                self.request_redraw();

                match crate::shader_installer::install_shaders_with_manifest(
                    force_overwrite_modified_shaders,
                ) {
                    Ok(result) => {
                        log::info!(
                            "Installed {} shader files ({} skipped, {} removed)",
                            result.installed,
                            result.skipped,
                            result.removed
                        );
                        let detail = if result.skipped > 0 {
                            format!("{} shaders ({} skipped)", result.installed, result.skipped)
                        } else {
                            format!("{} shaders", result.installed)
                        };
                        success_parts.push(detail);
                        self.config.integration_versions.shaders_installed_version =
                            Some(current_version.clone());
                        self.config.integration_versions.shaders_prompted_version =
                            Some(current_version.clone());
                    }
                    Err(e) => {
                        log::error!("Failed to install shaders: {}", e);
                        error_parts.push(format!("Shaders: {}", e));
                    }
                }
            }

            // Install shell integration if requested
            if install_shell_integration {
                self.overlay_ui
                    .integrations_ui
                    .set_installing("Installing shell integration...");
                self.focus_state.needs_redraw = true;
                self.request_redraw();

                match crate::shell_integration_installer::install(None) {
                    Ok(result) => {
                        log::info!(
                            "Installed shell integration for {}",
                            result.shell.display_name()
                        );
                        success_parts.push(format!(
                            "shell integration ({})",
                            result.shell.display_name()
                        ));
                        self.config
                            .integration_versions
                            .shell_integration_installed_version = Some(current_version.clone());
                        self.config
                            .integration_versions
                            .shell_integration_prompted_version = Some(current_version.clone());
                    }
                    Err(e) => {
                        log::error!("Failed to install shell integration: {}", e);
                        error_parts.push(format!("Shell: {}", e));
                    }
                }
            }

            // Show result
            if error_parts.is_empty() {
                self.overlay_ui
                    .integrations_ui
                    .set_success(&format!("Installed: {}", success_parts.join(", ")));
            } else if success_parts.is_empty() {
                self.overlay_ui
                    .integrations_ui
                    .set_error(&format!("Installation failed: {}", error_parts.join("; ")));
            } else {
                // Partial success
                self.overlay_ui.integrations_ui.set_success(&format!(
                    "Installed: {}. Errors: {}",
                    success_parts.join(", "),
                    error_parts.join("; ")
                ));
            }

            // Save config
            if let Err(e) = self.save_config_debounced() {
                log::error!("Failed to save config after integration install: {}", e);
            }

            // Clear pending flags
            self.overlay_ui.integrations_ui.pending_install_shaders = false;
            self.overlay_ui
                .integrations_ui
                .pending_install_shell_integration = false;

            self.focus_state.needs_redraw = true;
        }

        // Handle "Skip" - just close the dialog for this session
        if response.skipped {
            log::info!("User skipped integrations dialog for this session");
            self.overlay_ui.integrations_ui.hide();
            // Update prompted versions so we don't ask again this version
            self.config.integration_versions.shaders_prompted_version =
                Some(current_version.clone());
            self.config
                .integration_versions
                .shell_integration_prompted_version = Some(current_version.clone());
            if let Err(e) = self.save_config_debounced() {
                log::error!("Failed to save config after skipping integrations: {}", e);
            }
        }

        // Handle "Never Ask" - disable prompting permanently
        if response.never_ask {
            log::info!("User declined integrations (never ask again)");
            self.overlay_ui.integrations_ui.hide();
            // Set install prompts to Never
            self.config.shader_install_prompt = ShaderInstallPrompt::Never;
            self.config.shell_integration_state = crate::config::InstallPromptState::Never;
            if let Err(e) = self.save_config_debounced() {
                log::error!("Failed to save config after declining integrations: {}", e);
            }
        }

        // Handle dialog closed (OK button after success)
        if response.closed {
            self.overlay_ui.integrations_ui.hide();
        }
    }
}

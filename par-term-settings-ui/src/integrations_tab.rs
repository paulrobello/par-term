//! Integrations tab for settings UI.
//!
//! Shows installation status and controls for:
//! - Shell Integration (bash, zsh, fish)
//! - Custom Shaders bundle

use arboard::Clipboard;
use egui::{Color32, RichText, Ui};
use std::collections::HashSet;

use par_term_config::{Config, ShellType};

use super::SettingsUI;
use super::section::collapsing_section;

/// Actions for shell integration (consumed by app handler)
#[derive(Debug, Clone, Copy)]
pub enum ShellIntegrationAction {
    /// Install shell integration for the detected shell
    Install,
    /// Uninstall shell integration from all shells
    Uninstall,
}

/// Actions for shader installation (consumed by app handler)
#[derive(Debug, Clone, Copy)]
pub enum ShaderAction {
    /// Install shaders from GitHub release
    Install,
    /// Uninstall all bundled shaders
    Uninstall,
}

impl SettingsUI {
    /// Show the integrations tab content.
    pub fn show_integrations_tab(
        &mut self,
        ui: &mut Ui,
        _changes_this_frame: &mut bool,
        collapsed: &mut HashSet<String>,
    ) {
        let query = self.search_query.trim().to_lowercase();

        // Shell Integration section
        if section_matches(
            &query,
            "Shell Integration",
            &[
                "shell",
                "bash",
                "zsh",
                "fish",
                "prompt",
                "integration",
                "install",
                "auto-install",
            ],
        ) {
            self.show_shell_integration_section(ui, _changes_this_frame, collapsed);
        }

        // Custom Shaders section
        if section_matches(
            &query,
            "Custom Shaders",
            &[
                "shader",
                "glsl",
                "effect",
                "background",
                "cursor",
                "custom shader",
                "animation",
                "post-processing",
            ],
        ) {
            self.show_shaders_section(ui, _changes_this_frame, collapsed);
        }
    }

    fn show_shell_integration_section(
        &mut self,
        ui: &mut Ui,
        _changes_this_frame: &mut bool,
        collapsed: &mut HashSet<String>,
    ) {
        collapsing_section(
            ui,
            "Shell Integration",
            "integrations_shell",
            true,
            collapsed,
            |ui| {
                ui.label("Shell integration provides enhanced terminal features like directory tracking and command notifications.");
                ui.add_space(8.0);

                // Detect shell and installation status
                let detected_shell = self
                    .shell_integration_detected_shell_fn
                    .map(|f| f())
                    .unwrap_or_else(ShellType::detect);
                let is_installed = self
                    .shell_integration_is_installed_fn
                    .map(|f| f())
                    .unwrap_or(false);

                // Status indicator
                ui.horizontal(|ui| {
                    ui.label("Status:");
                    if is_installed {
                        ui.colored_label(Color32::from_rgb(100, 200, 100), "Installed");
                    } else {
                        ui.colored_label(Color32::from_rgb(200, 150, 100), "Not installed");
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Detected shell:");
                    ui.label(RichText::new(shell_type_display(detected_shell)).strong());
                });

                if detected_shell == ShellType::Unknown {
                    ui.add_space(4.0);
                    ui.colored_label(
                        Color32::from_rgb(200, 100, 100),
                        "Could not detect shell type. Manual installation may be required.",
                    );
                }

                ui.add_space(8.0);

                // Action buttons
                ui.horizontal(|ui| {
                    let install_text = if is_installed { "Reinstall" } else { "Install" };

                    if detected_shell != ShellType::Unknown
                        && ui
                            .button(install_text)
                            .on_hover_text("Install shell integration scripts")
                            .clicked()
                    {
                        self.shell_integration_action = Some(ShellIntegrationAction::Install);
                    }

                    if is_installed
                        && ui
                            .button("Uninstall")
                            .on_hover_text("Remove shell integration from all shells")
                            .clicked()
                    {
                        self.shell_integration_action = Some(ShellIntegrationAction::Uninstall);
                    }
                });

                ui.add_space(8.0);

                // Manual installation instructions
                ui.label(RichText::new("Manual Installation").strong());
                ui.label("Run this command in your terminal:");

                let curl_cmd = "curl -fsSL https://paulrobello.github.io/par-term/install-shell-integration.sh | bash";

                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut curl_cmd.to_string())
                            .desired_width(400.0)
                            .interactive(false)
                            .font(egui::TextStyle::Monospace),
                    );

                    if ui.button("Copy").clicked()
                        && let Ok(mut clipboard) = Clipboard::new()
                    {
                        let _ = clipboard.set_text(curl_cmd);
                    }
                });

                // Show installed version if available
                if let Some(ref version) = self
                    .config
                    .integration_versions
                    .shell_integration_installed_version
                {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("Installed version: {}", version))
                            .small()
                            .color(Color32::GRAY),
                    );
                }
            },
        );
    }

    fn show_shaders_section(
        &mut self,
        ui: &mut Ui,
        _changes_this_frame: &mut bool,
        collapsed: &mut HashSet<String>,
    ) {
        // Update async install status
        self.poll_shader_install_status();

        collapsing_section(
            ui,
            "Custom Shaders",
            "integrations_shaders",
            true,
            collapsed,
            |ui| {
                ui.label("Custom shaders provide background effects and cursor animations for your terminal.");
                ui.add_space(8.0);

                // Check installation status
                let shaders_dir = Config::shaders_dir();
                let has_shaders = self
                    .shader_has_files_fn
                    .map(|f| f(&shaders_dir))
                    .unwrap_or(false);
                let shader_count = if has_shaders {
                    self.shader_count_files_fn
                        .map(|f| f(&shaders_dir))
                        .unwrap_or(0)
                } else {
                    0
                };

                // Status indicator
                ui.horizontal(|ui| {
                    ui.label("Status:");
                    if has_shaders {
                        ui.colored_label(
                            Color32::from_rgb(100, 200, 100),
                            format!("Installed ({} shaders)", shader_count),
                        );
                    } else {
                        ui.colored_label(Color32::from_rgb(200, 150, 100), "Not installed");
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Location:");
                    ui.label(
                        RichText::new(shaders_dir.display().to_string())
                            .small()
                            .color(Color32::GRAY),
                    );
                });

                // Show installed version if available
                if let Some(ref version) =
                    self.config.integration_versions.shaders_installed_version
                {
                    ui.horizontal(|ui| {
                        ui.label("Version:");
                        ui.label(RichText::new(version.clone()).small().color(Color32::GRAY));
                    });
                }

                ui.add_space(8.0);

                // Status / errors / progress
                if let Some(status) = &self.shader_status {
                    ui.colored_label(Color32::from_rgb(100, 200, 100), status);
                }
                if let Some(err) = &self.shader_error {
                    ui.colored_label(Color32::from_rgb(220, 120, 120), err);
                }
                if self.shader_installing {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Installing shaders...");
                    });
                }

                if self.shader_overwrite_prompt_visible {
                    ui.add_space(6.0);
                    ui.group(|ui| {
                    ui.label(RichText::new("Modified bundled shaders detected").strong());
                    if self.shader_conflicts.is_empty() {
                        ui.label(
                            RichText::new(
                                "Some bundled shaders were modified. Overwrite them or keep your changes?",
                            )
                            .small(),
                        );
                    } else {
                        let preview: Vec<_> =
                            self.shader_conflicts.iter().take(5).cloned().collect();
                        ui.label(
                            RichText::new(format!(
                                "{} modified files: {}{}",
                                self.shader_conflicts.len(),
                                preview.join(", "),
                                if self.shader_conflicts.len() > 5 {
                                    " …"
                                } else {
                                    ""
                                }
                            ))
                            .small(),
                        );
                    }

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(egui::Button::new("Overwrite modified"))
                            .clicked()
                        {
                            self.shader_overwrite_prompt_visible = false;
                            if let Some(install_fn) = self.shader_install_fn {
                                self.start_shader_install_with(true, install_fn);
                            }
                        }
                        if ui
                            .add(egui::Button::new("Skip modified"))
                            .on_hover_text("Keep your edited shaders; reinstall the rest")
                            .clicked()
                        {
                            self.shader_overwrite_prompt_visible = false;
                            if let Some(install_fn) = self.shader_install_fn {
                                self.start_shader_install_with(false, install_fn);
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.shader_overwrite_prompt_visible = false;
                            self.shader_conflicts.clear();
                        }
                    });
                });
                    ui.add_space(8.0);
                }

                // Action buttons
                ui.horizontal(|ui| {
                    let install_text = if has_shaders { "Reinstall" } else { "Install" };

                    if ui
                        .button(install_text)
                        .on_hover_text("Download and install shader bundle from GitHub")
                        .clicked()
                    {
                        if let Some(detect_fn) = self.shader_detect_modified_fn {
                            match detect_fn() {
                                Ok(conflicts) if !conflicts.is_empty() => {
                                    self.shader_conflicts = conflicts;
                                    self.shader_overwrite_prompt_visible = true;
                                }
                                Ok(_) => {
                                    if let Some(install_fn) = self.shader_install_fn {
                                        self.start_shader_install_with(false, move |force| {
                                            install_fn(force)
                                        });
                                    }
                                }
                                Err(e) => {
                                    self.shader_error =
                                        Some(format!("Failed to check existing shaders: {}", e));
                                }
                            }
                        } else {
                            self.shader_error = Some("Shader installer not configured".to_string());
                        }
                    }

                    if has_shaders
                        && ui
                            .button("Uninstall")
                            .on_hover_text(
                                "Remove all bundled shaders (keeps user-created shaders)",
                            )
                            .clicked()
                        && let Some(uninstall_fn) = self.shader_uninstall_fn
                    {
                        match uninstall_fn(false) {
                            Ok(result) => {
                                self.shader_error = None;
                                self.shader_status = Some(format!(
                                    "Removed {} files, kept {}",
                                    result.removed, result.kept
                                ));
                            }
                            Err(e) => {
                                self.shader_error =
                                    Some(format!("Failed to uninstall shaders: {}", e));
                                self.shader_status = None;
                            }
                        }
                    }

                    if ui
                        .button("Open Folder")
                        .on_hover_text("Open shaders folder in file manager")
                        .clicked()
                        && let Err(e) = open::that(&shaders_dir)
                    {
                        log::error!("Failed to open shaders folder: {}", e);
                    }
                });

                ui.add_space(8.0);

                // Manual installation instructions
                ui.label(RichText::new("Manual Installation").strong());
                ui.label("Run this command in your terminal:");

                let curl_cmd =
                    "curl -fsSL https://paulrobello.github.io/par-term/install-shaders.sh | bash";

                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut curl_cmd.to_string())
                            .desired_width(400.0)
                            .interactive(false)
                            .font(egui::TextStyle::Monospace),
                    );

                    if ui.button("Copy").clicked()
                        && let Ok(mut clipboard) = Clipboard::new()
                    {
                        let _ = clipboard.set_text(curl_cmd);
                    }
                });
            },
        );
    }
}

fn section_matches(query: &str, title: &str, keywords: &[&str]) -> bool {
    if query.is_empty() {
        return true;
    }
    if title.to_lowercase().contains(query) {
        return true;
    }
    keywords.iter().any(|k| k.to_lowercase().contains(query))
}

fn shell_type_display(shell: ShellType) -> &'static str {
    match shell {
        ShellType::Bash => "Bash",
        ShellType::Zsh => "Zsh",
        ShellType::Fish => "Fish",
        ShellType::Unknown => "Unknown",
    }
}

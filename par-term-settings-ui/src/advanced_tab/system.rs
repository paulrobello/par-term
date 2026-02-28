//! System sections for the advanced settings tab.
//!
//! Covers: Screenshots, Updates, File Transfers, Debug Logging, Security.

use crate::SettingsUI;
use crate::format_timestamp;
use crate::section::{INPUT_WIDTH, collapsing_section};
use par_term_config::{DownloadSaveLocation, LogLevel, UpdateCheckFrequency};
use std::collections::HashSet;

// ============================================================================
// Screenshots Section
// ============================================================================

pub(super) fn show_screenshot_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Screenshots",
        "advanced_screenshots",
        false,
        collapsed,
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Format:");

                let options = ["png", "jpeg", "svg", "html"];
                let mut selected = settings.config.screenshot_format.clone();

                egui::ComboBox::from_id_salt("advanced_screenshot_format")
                    .width(140.0)
                    .selected_text(selected.as_str())
                    .show_ui(ui, |ui| {
                        for opt in options {
                            ui.selectable_value(&mut selected, opt.to_string(), opt);
                        }
                    });

                if selected != settings.config.screenshot_format {
                    settings.config.screenshot_format = selected;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
            ui.label("Supported: png, jpeg, svg, html");
        },
    );
}

// ============================================================================
// Updates Section
// ============================================================================

pub(super) fn show_updates_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Updates", "advanced_updates", true, collapsed, |ui| {
        ui.horizontal(|ui| {
            ui.label("Current version:");
            ui.label(settings.app_version);
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Check for updates:");

            let current = settings.config.updates.update_check_frequency;
            egui::ComboBox::from_id_salt("advanced_update_check_frequency")
                .selected_text(current.display_name())
                .show_ui(ui, |ui| {
                    for freq in [
                        UpdateCheckFrequency::Never,
                        UpdateCheckFrequency::Hourly,
                        UpdateCheckFrequency::Daily,
                        UpdateCheckFrequency::Weekly,
                        UpdateCheckFrequency::Monthly,
                    ] {
                        if ui
                            .selectable_value(
                                &mut settings.config.updates.update_check_frequency,
                                freq,
                                freq.display_name(),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        if let Some(ref last_check) = settings.config.updates.last_update_check {
            ui.horizontal(|ui| {
                ui.label("Last checked:");
                ui.label(format_timestamp(last_check));
            });
        }

        if let Some(skipped) = settings.config.updates.skipped_version.clone() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("Skipping notifications for v{}", skipped))
                        .small()
                        .color(egui::Color32::GRAY),
                );
                if ui.small_button("Clear").clicked() {
                    settings.config.updates.skipped_version = None;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }

        ui.add_space(8.0);

        // Check Now button
        ui.horizontal(|ui| {
            if ui
                .button("Check Now")
                .on_hover_text("Check for updates immediately")
                .clicked()
            {
                settings.check_now_requested = true;
            }
        });

        // Show update check result
        if let Some(ref result) = settings.last_update_result {
            ui.add_space(4.0);
            match result {
                crate::UpdateCheckResult::UpToDate => {
                    ui.label(
                        egui::RichText::new("You are running the latest version.")
                            .color(egui::Color32::from_rgb(100, 200, 100)),
                    );
                }
                crate::UpdateCheckResult::UpdateAvailable(info) => {
                    let version_str = info.version.strip_prefix('v').unwrap_or(&info.version);
                    ui.label(
                        egui::RichText::new(format!("Version {} is available!", version_str))
                            .color(egui::Color32::YELLOW)
                            .strong(),
                    );

                    // Show release URL as clickable link
                    ui.hyperlink_to("View release on GitHub", &info.release_url);

                    ui.add_space(4.0);

                    // Detect installation type to decide what button to show
                    let installation = settings.installation_type;
                    match installation {
                        crate::InstallationType::Homebrew => {
                            ui.label(
                                egui::RichText::new(
                                    "Update via Homebrew: brew upgrade --cask par-term",
                                )
                                .color(egui::Color32::GRAY),
                            );
                        }
                        crate::InstallationType::CargoInstall => {
                            ui.label(
                                egui::RichText::new("Update via cargo: cargo install par-term")
                                    .color(egui::Color32::GRAY),
                            );
                        }
                        _ => {
                            // Show Install Update button
                            let installing = settings.update_installing;
                            let button_text = if installing {
                                "Installing..."
                            } else {
                                "Install Update"
                            };

                            let button =
                                egui::Button::new(egui::RichText::new(button_text).strong());

                            if ui
                                .add_enabled(!installing, button)
                                .on_hover_text(format!("Download and install v{}", version_str))
                                .clicked()
                            {
                                settings.update_install_requested = true;
                            }
                        }
                    }
                }
                crate::UpdateCheckResult::Error(e) => {
                    ui.label(
                        egui::RichText::new(format!("Check failed: {}", e))
                            .color(egui::Color32::from_rgb(255, 100, 100)),
                    );
                }
                _ => {}
            }
        }

        // Show update status/result
        if let Some(ref status) = settings.update_status {
            ui.add_space(4.0);
            let color = if settings.update_result.as_ref().is_some_and(|r| r.is_err()) {
                egui::Color32::from_rgb(255, 100, 100)
            } else if settings.update_result.as_ref().is_some_and(|r| r.is_ok()) {
                egui::Color32::from_rgb(100, 200, 100)
            } else {
                egui::Color32::YELLOW
            };
            ui.label(egui::RichText::new(status.as_str()).color(color));
        }

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "par-term checks for updates periodically based on the frequency above.",
            )
            .small()
            .color(egui::Color32::GRAY),
        );
    });
}

// ============================================================================
// File Transfers Section
// ============================================================================

pub(super) fn show_file_transfers_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "File Transfers",
        "advanced_file_transfers",
        true,
        collapsed,
        |ui| {
            ui.label("Configure where downloaded files are saved.");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Download save location:");

                // Determine which variant is currently selected (ignoring Custom's inner string)
                let is_custom = matches!(
                    settings.config.download_save_location,
                    DownloadSaveLocation::Custom(_)
                );
                let selected_text = settings.config.download_save_location.display_name();

                egui::ComboBox::from_id_salt("advanced_download_save_location")
                    .width(200.0)
                    .selected_text(selected_text)
                    .show_ui(ui, |ui| {
                        // Non-custom variants
                        for variant in DownloadSaveLocation::variants() {
                            if ui
                                .selectable_label(
                                    !is_custom
                                        && settings.config.download_save_location == *variant,
                                    variant.display_name(),
                                )
                                .clicked()
                                && settings.config.download_save_location != *variant
                            {
                                settings.config.download_save_location = variant.clone();
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                        // Custom variant
                        if ui.selectable_label(is_custom, "Custom directory").clicked()
                            && !is_custom
                        {
                            settings.config.download_save_location =
                                DownloadSaveLocation::Custom(String::new());
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });
            });

            // Show custom path picker when Custom is selected
            if let DownloadSaveLocation::Custom(ref path) = settings.config.download_save_location {
                let mut custom_path = path.clone();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Custom path:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut custom_path)
                                .desired_width(INPUT_WIDTH)
                                .hint_text("/path/to/downloads"),
                        )
                        .changed()
                    {
                        settings.config.download_save_location =
                            DownloadSaveLocation::Custom(custom_path.clone());
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }

                    if ui.button("Browse...").clicked() {
                        let mut dialog =
                            rfd::FileDialog::new().set_title("Select Download Directory");
                        if !custom_path.is_empty() {
                            dialog = dialog.set_directory(&custom_path);
                        }
                        if let Some(folder) = dialog.pick_folder() {
                            settings.config.download_save_location =
                                DownloadSaveLocation::Custom(folder.display().to_string());
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
            }
        },
    );
}

// ============================================================================
// Debug Logging Section
// ============================================================================

pub(super) fn show_debug_logging_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Debug Logging",
        "advanced_debug_logging",
        true,
        collapsed,
        |ui| {
            ui.label("Configure diagnostic logging to file for troubleshooting.");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Log level:");

                let current = settings.config.log_level;
                egui::ComboBox::from_id_salt("advanced_log_level")
                    .width(120.0)
                    .selected_text(current.display_name())
                    .show_ui(ui, |ui| {
                        for level in LogLevel::all() {
                            if ui
                                .selectable_label(current == *level, level.display_name())
                                .clicked()
                                && current != *level
                            {
                                settings.config.log_level = *level;
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            ui.add_space(4.0);
            let log_path = crate::log_path();
            ui.horizontal(|ui| {
                ui.label("Log file:");
                ui.label(
                    egui::RichText::new(log_path.display().to_string())
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });

            ui.add_space(4.0);
            if ui.button("Open Log File").clicked() {
                settings.open_log_requested = true;
            }

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "Set to Off to suppress log file creation. \
                     RUST_LOG env var and --log-level CLI flag override this setting.",
                )
                .small()
                .color(egui::Color32::GRAY),
            );
        },
    );
}

// ============================================================================
// Security Section
// ============================================================================

pub(super) fn show_security_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Security", "advanced_security", true, collapsed, |ui| {
        ui.label("Environment variable substitution in config files.");
        ui.add_space(8.0);

        let mut allow_all = settings.config.allow_all_env_vars;
        if ui
            .checkbox(
                &mut allow_all,
                "Allow all environment variables in config substitution",
            )
            .changed()
        {
            settings.config.allow_all_env_vars = allow_all;
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "When disabled (default), only safe environment variables (HOME, USER, \
                     SHELL, XDG_*, PAR_TERM_*, LC_*, etc.) are substituted in config files. \
                     Enable this to allow any environment variable — use with caution if \
                     loading configs from untrusted sources.",
            )
            .small()
            .color(egui::Color32::GRAY),
        );
    });
}

//! Update settings tab for the settings UI.

use super::SettingsUI;
use crate::config::UpdateCheckFrequency;
use crate::update_checker::{UpdateCheckResult, format_timestamp};

/// Show the update settings section
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    update_result: Option<&UpdateCheckResult>,
    check_now_callback: &mut Option<Box<dyn FnMut()>>,
) {
    ui.collapsing("Updates", |ui| {
        // Current version display
        ui.horizontal(|ui| {
            ui.label("Current version:");
            ui.label(env!("CARGO_PKG_VERSION"));
        });

        ui.add_space(8.0);

        // Update check frequency dropdown
        ui.horizontal(|ui| {
            ui.label("Check for updates:");

            let current = settings.config.update_check_frequency;
            egui::ComboBox::from_id_salt("update_check_frequency")
                .selected_text(current.display_name())
                .show_ui(ui, |ui| {
                    for freq in [
                        UpdateCheckFrequency::Never,
                        UpdateCheckFrequency::Daily,
                        UpdateCheckFrequency::Weekly,
                        UpdateCheckFrequency::Monthly,
                    ] {
                        if ui
                            .selectable_value(
                                &mut settings.config.update_check_frequency,
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

        // Last check time
        if let Some(ref last_check) = settings.config.last_update_check {
            ui.horizontal(|ui| {
                ui.label("Last checked:");
                ui.label(format_timestamp(last_check));
            });
        }

        ui.add_space(8.0);

        // Check Now button
        ui.horizontal(|ui| {
            let button_enabled = settings.config.update_check_frequency
                != UpdateCheckFrequency::Never
                || check_now_callback.is_some();

            if ui
                .add_enabled(button_enabled, egui::Button::new("Check Now"))
                .clicked()
                && let Some(callback) = check_now_callback
            {
                callback();
            }

            // Show status indicator
            if let Some(result) = update_result {
                match result {
                    UpdateCheckResult::UpToDate => {
                        ui.label(egui::RichText::new("Up to date").color(egui::Color32::GREEN));
                    }
                    UpdateCheckResult::UpdateAvailable(info) => {
                        ui.label(
                            egui::RichText::new(format!("v{} available!", info.version))
                                .color(egui::Color32::YELLOW),
                        );
                    }
                    UpdateCheckResult::Error(e) => {
                        ui.label(egui::RichText::new("Check failed").color(egui::Color32::RED))
                            .on_hover_text(e);
                    }
                    UpdateCheckResult::Disabled => {
                        ui.label(egui::RichText::new("Disabled").color(egui::Color32::GRAY));
                    }
                    UpdateCheckResult::Skipped => {
                        // Don't show anything for skipped
                    }
                }
            }
        });

        // Show update details if available
        if let Some(UpdateCheckResult::UpdateAvailable(info)) = update_result {
            ui.add_space(8.0);
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("New version available:").strong());
                    ui.label(&info.version);
                });

                if let Some(ref published_at) = info.published_at {
                    ui.horizontal(|ui| {
                        ui.label("Published:");
                        ui.label(format_timestamp(published_at));
                    });
                }

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    if ui.button("View Release").clicked() {
                        let _ = open::that(&info.release_url);
                    }

                    if ui.button("Skip This Version").clicked() {
                        settings.config.skipped_version = Some(
                            info.version
                                .strip_prefix('v')
                                .unwrap_or(&info.version)
                                .to_string(),
                        );
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });

                // Show release notes preview if available
                if let Some(ref notes) = info.release_notes {
                    ui.add_space(4.0);
                    ui.collapsing("Release Notes", |ui| {
                        // Truncate long notes
                        let truncated = if notes.len() > 500 {
                            format!("{}...", &notes[..500])
                        } else {
                            notes.clone()
                        };
                        ui.label(&truncated);
                    });
                }
            });
        }

        // Skipped version info
        if let Some(skipped) = settings.config.skipped_version.clone() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("Skipping notifications for v{}", skipped))
                        .small()
                        .color(egui::Color32::GRAY),
                );
                if ui.small_button("Clear").clicked() {
                    settings.config.skipped_version = None;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "par-term only checks for updates and notifies you. \
                Download and install updates manually from GitHub.",
            )
            .small()
            .color(egui::Color32::GRAY),
        );
    });
}

/// Simplified show function that doesn't require update state
/// (for initial integration, state management can be added later)
pub fn show_simple(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Updates", |ui| {
        // Current version display
        ui.horizontal(|ui| {
            ui.label("Current version:");
            ui.label(env!("CARGO_PKG_VERSION"));
        });

        ui.add_space(8.0);

        // Update check frequency dropdown
        ui.horizontal(|ui| {
            ui.label("Check for updates:");

            let current = settings.config.update_check_frequency;
            egui::ComboBox::from_id_salt("update_check_frequency")
                .selected_text(current.display_name())
                .show_ui(ui, |ui| {
                    for freq in [
                        UpdateCheckFrequency::Never,
                        UpdateCheckFrequency::Daily,
                        UpdateCheckFrequency::Weekly,
                        UpdateCheckFrequency::Monthly,
                    ] {
                        if ui
                            .selectable_value(
                                &mut settings.config.update_check_frequency,
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

        // Last check time
        if let Some(ref last_check) = settings.config.last_update_check {
            ui.horizontal(|ui| {
                ui.label("Last checked:");
                ui.label(format_timestamp(last_check));
            });
        }

        // Skipped version info
        if let Some(skipped) = settings.config.skipped_version.clone() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("Skipping notifications for v{}", skipped))
                        .small()
                        .color(egui::Color32::GRAY),
                );
                if ui.small_button("Clear").clicked() {
                    settings.config.skipped_version = None;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "par-term only checks for updates and notifies you. \
                Download and install updates manually from GitHub.",
            )
            .small()
            .color(egui::Color32::GRAY),
        );
    });
}

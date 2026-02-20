//! Update notification dialog overlay.
//!
//! Renders an egui modal window when a new version of par-term is available,
//! showing version info, release notes, and install/skip/dismiss actions.

use crate::update_checker::UpdateCheckResult;

/// Action returned by the update dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateDialogAction {
    /// User dismissed the dialog.
    Dismiss,
    /// User wants to skip this version.
    SkipVersion(String),
    /// User wants to install the update.
    InstallUpdate(String),
    /// Dialog is still open, no action taken.
    None,
}

/// Render the update dialog overlay.
///
/// Call this when `show_update_dialog` is true. Returns the user's action.
///
/// When `installing` is true, the Install button is disabled and shows "Installing...".
/// The `install_status` message (if any) is displayed below the buttons.
pub fn render(
    ctx: &egui::Context,
    update_result: &UpdateCheckResult,
    current_version: &str,
    installation_type: par_term_settings_ui::InstallationType,
    installing: bool,
    install_status: Option<&str>,
) -> UpdateDialogAction {
    let mut action = UpdateDialogAction::None;

    // Only show dialog for UpdateAvailable
    let info = match update_result {
        UpdateCheckResult::UpdateAvailable(info) => info,
        _ => return UpdateDialogAction::Dismiss,
    };

    let version_str = info.version.strip_prefix('v').unwrap_or(&info.version);

    egui::Window::new("Update Available")
        .collapsible(false)
        .resizable(true)
        .default_width(450.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                // Version info
                ui.heading(format!("par-term v{} is available!", version_str));
                ui.add_space(4.0);
                ui.label(format!("You are currently running v{}", current_version));
                ui.add_space(12.0);

                // Release notes
                if let Some(ref notes) = info.release_notes {
                    if !notes.is_empty() {
                        ui.label(egui::RichText::new("Release Notes").strong());
                        ui.add_space(4.0);
                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .show(ui, |ui| {
                                ui.label(notes);
                            });
                        ui.add_space(8.0);
                    }
                }

                // Release URL link
                ui.hyperlink_to("View release on GitHub", &info.release_url);
                ui.add_space(12.0);

                // Installation-specific UI
                match installation_type {
                    par_term_settings_ui::InstallationType::Homebrew => {
                        ui.label(
                            egui::RichText::new("Update via Homebrew:")
                                .color(egui::Color32::GRAY),
                        );
                        ui.code("brew upgrade --cask par-term");
                        ui.add_space(8.0);
                    }
                    par_term_settings_ui::InstallationType::CargoInstall => {
                        ui.label(
                            egui::RichText::new("Update via Cargo:").color(egui::Color32::GRAY),
                        );
                        ui.code("cargo install par-term");
                        ui.add_space(8.0);
                    }
                    _ => {
                        // Standalone/Bundle - show Install button
                        if installing {
                            let button = egui::Button::new(
                                egui::RichText::new("Installing...").strong(),
                            );
                            ui.add_enabled(false, button);
                        } else if ui
                            .button(egui::RichText::new("Install Update").strong())
                            .clicked()
                        {
                            action =
                                UpdateDialogAction::InstallUpdate(version_str.to_string());
                        }
                        ui.add_space(8.0);
                    }
                }

                // Show install status message
                if let Some(status) = install_status {
                    ui.add_space(4.0);
                    let color = if status.starts_with("Update failed") {
                        egui::Color32::from_rgb(255, 100, 100)
                    } else if status.starts_with("Updated to") {
                        egui::Color32::from_rgb(100, 255, 100)
                    } else {
                        egui::Color32::YELLOW
                    };
                    ui.label(egui::RichText::new(status).color(color));
                    ui.add_space(4.0);
                }

                // Bottom buttons
                ui.separator();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    // Disable Skip and Dismiss while installing
                    ui.add_enabled_ui(!installing, |ui| {
                        if ui.button("Skip This Version").clicked() {
                            action = UpdateDialogAction::SkipVersion(version_str.to_string());
                        }
                        if ui.button("Dismiss").clicked() {
                            action = UpdateDialogAction::Dismiss;
                        }
                    });
                });
            });
        });

    action
}

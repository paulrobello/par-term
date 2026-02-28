//! Update checking logic for the window manager.
//!
//! Handles periodic update checks, forced update checks (from UI),
//! desktop notifications, and syncing update state to settings windows.

use crate::update_checker::{UpdateCheckResult, UpdateInfo};

/// Show desktop notification when update is available.
pub(super) fn notify_update_available(info: &UpdateInfo) {
    let version_str = info.version.strip_prefix('v').unwrap_or(&info.version);
    let current = env!("CARGO_PKG_VERSION");
    let summary = format!("par-term v{} Available", version_str);
    let body = format!(
        "You have v{}. Check Settings > Advanced > Updates.",
        current
    );

    #[cfg(not(target_os = "macos"))]
    {
        use notify_rust::Notification;
        let _ = Notification::new()
            .summary(&summary)
            .body(&body)
            .appname("par-term")
            .timeout(notify_rust::Timeout::Milliseconds(8000))
            .show();
    }

    #[cfg(target_os = "macos")]
    {
        // Escape backslashes, quotes, and newlines for AppleScript string safety
        // Order matters: escape backslashes FIRST, then quotes, then newlines
        let escaped_body = body
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r");
        let escaped_summary = summary
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r");
        let script = format!(
            r#"display notification "{}" with title "{}""#,
            escaped_body, escaped_summary,
        );
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .spawn();
    }
}

/// Convert a main-crate UpdateCheckResult to the settings-ui crate's type.
pub(super) fn to_settings_update_result(
    result: &UpdateCheckResult,
) -> crate::settings_ui::UpdateCheckResult {
    match result {
        UpdateCheckResult::UpToDate => crate::settings_ui::UpdateCheckResult::UpToDate,
        UpdateCheckResult::UpdateAvailable(info) => {
            crate::settings_ui::UpdateCheckResult::UpdateAvailable(
                crate::settings_ui::UpdateCheckInfo {
                    version: info.version.clone(),
                    release_notes: info.release_notes.clone(),
                    release_url: info.release_url.clone(),
                    published_at: info.published_at.clone(),
                },
            )
        }
        UpdateCheckResult::Disabled => crate::settings_ui::UpdateCheckResult::Disabled,
        UpdateCheckResult::Skipped => crate::settings_ui::UpdateCheckResult::Skipped,
        UpdateCheckResult::Error(e) => crate::settings_ui::UpdateCheckResult::Error(e.clone()),
    }
}

/// Extract the available version string from an update result (None if not available).
pub(super) fn update_available_version(result: &UpdateCheckResult) -> Option<String> {
    match result {
        UpdateCheckResult::UpdateAvailable(info) => Some(
            info.version
                .strip_prefix('v')
                .unwrap_or(&info.version)
                .to_string(),
        ),
        _ => None,
    }
}

use super::WindowManager;

impl WindowManager {
    /// Check for updates (called periodically from about_to_wait)
    pub fn check_for_updates(&mut self) {
        use crate::update_checker::current_timestamp;
        use std::time::{Duration, Instant};

        let now = Instant::now();

        // Schedule initial check shortly after startup (5 seconds delay)
        if self.next_update_check.is_none() {
            self.next_update_check = Some(now + Duration::from_secs(5));
            return;
        }

        // Check if it's time for scheduled check
        if let Some(next_check) = self.next_update_check
            && now >= next_check
        {
            // Perform the check
            let (result, should_save) = self.update_checker.check_now(&self.config, false);

            // Log the result and notify if appropriate
            let mut config_changed = should_save;
            match &result {
                UpdateCheckResult::UpdateAvailable(info) => {
                    let version_str = info
                        .version
                        .strip_prefix('v')
                        .unwrap_or(&info.version)
                        .to_string();

                    log::info!(
                        "Update available: {} (current: {})",
                        version_str,
                        env!("CARGO_PKG_VERSION")
                    );

                    // Only notify if we haven't already notified about this version
                    let already_notified = self
                        .config
                        .updates
                        .last_notified_version
                        .as_ref()
                        .is_some_and(|v| v == &version_str);

                    if !already_notified {
                        notify_update_available(info);
                        self.config.updates.last_notified_version = Some(version_str);
                        config_changed = true;
                    }
                }
                UpdateCheckResult::UpToDate => {
                    log::info!("par-term is up to date ({})", env!("CARGO_PKG_VERSION"));
                }
                UpdateCheckResult::Error(e) => {
                    log::warn!("Update check failed: {}", e);
                }
                UpdateCheckResult::Disabled | UpdateCheckResult::Skipped => {
                    // Silent
                }
            }

            self.last_update_result = Some(result);

            // Sync update version to status bar widgets
            let version = self
                .last_update_result
                .as_ref()
                .and_then(update_available_version);
            let result_clone = self.last_update_result.clone();
            for ws in self.windows.values_mut() {
                ws.status_bar_ui.update_available_version = version.clone();
                ws.update_state.last_result = result_clone.clone();
            }

            // Save config with updated timestamp if check was successful
            if config_changed {
                self.config.updates.last_update_check = Some(current_timestamp());
                if let Err(e) = self.config.save() {
                    log::warn!("Failed to save config after update check: {}", e);
                }
            }

            // Schedule next check based on frequency
            self.next_update_check = self
                .config
                .updates
                .update_check_frequency
                .as_seconds()
                .map(|secs| now + Duration::from_secs(secs));
        }
    }

    /// Force an immediate update check (triggered from UI)
    pub fn force_update_check(&mut self) {
        use crate::update_checker::current_timestamp;

        let (result, should_save) = self.update_checker.check_now(&self.config, true);

        // Log the result
        match &result {
            UpdateCheckResult::UpdateAvailable(info) => {
                log::info!(
                    "Update available: {} (current: {})",
                    info.version,
                    env!("CARGO_PKG_VERSION")
                );
            }
            UpdateCheckResult::UpToDate => {
                log::info!("par-term is up to date ({})", env!("CARGO_PKG_VERSION"));
            }
            UpdateCheckResult::Error(e) => {
                log::warn!("Update check failed: {}", e);
            }
            _ => {}
        }

        self.last_update_result = Some(result);

        // Sync update version and full result to status bar widgets and update dialog
        let version = self
            .last_update_result
            .as_ref()
            .and_then(update_available_version);
        let result_clone = self.last_update_result.clone();
        for ws in self.windows.values_mut() {
            ws.status_bar_ui.update_available_version = version.clone();
            ws.update_state.last_result = result_clone.clone();
        }

        // Save config with updated timestamp
        if should_save {
            self.config.updates.last_update_check = Some(current_timestamp());
            if let Err(e) = self.config.save() {
                log::warn!("Failed to save config after update check: {}", e);
            }
        }
    }

    /// Force an update check and sync the result to the settings window.
    pub fn force_update_check_for_settings(&mut self) {
        self.force_update_check();
        // Sync the result to the settings window
        if let Some(settings_window) = &mut self.settings_window {
            settings_window.settings_ui.last_update_result = self
                .last_update_result
                .as_ref()
                .map(to_settings_update_result);
            settings_window.request_redraw();
        }
    }

    /// Detect the installation type and convert to the settings-ui enum.
    pub(super) fn detect_installation_type(&self) -> par_term_settings_ui::InstallationType {
        let install = crate::self_updater::detect_installation();
        match install {
            crate::self_updater::InstallationType::Homebrew => {
                par_term_settings_ui::InstallationType::Homebrew
            }
            crate::self_updater::InstallationType::CargoInstall => {
                par_term_settings_ui::InstallationType::CargoInstall
            }
            crate::self_updater::InstallationType::MacOSBundle => {
                par_term_settings_ui::InstallationType::MacOSBundle
            }
            crate::self_updater::InstallationType::StandaloneBinary => {
                par_term_settings_ui::InstallationType::StandaloneBinary
            }
        }
    }

    /// Send a test desktop notification (for debugging notification support).
    pub fn send_test_notification(&self) {
        log::info!("Sending test notification");

        #[cfg(not(target_os = "macos"))]
        {
            use notify_rust::Notification;
            if let Err(e) = Notification::new()
                .summary("par-term Test Notification")
                .body("If you see this, notifications are working!")
                .timeout(notify_rust::Timeout::Milliseconds(5000))
                .show()
            {
                log::warn!("Failed to send test notification: {}", e);
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS notifications via osascript
            let script = r#"display notification "If you see this, notifications are working!" with title "par-term Test Notification""#;

            if let Err(e) = std::process::Command::new("osascript")
                .arg("-e")
                .arg(script)
                .output()
            {
                log::warn!("Failed to send macOS test notification: {}", e);
            }
        }
    }
}

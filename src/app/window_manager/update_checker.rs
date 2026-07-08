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
    crate::platform::deliver_desktop_notification(
        &summary,
        &body,
        8000,
        crate::platform::NotificationUrgency::Normal,
    );
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

/// Outcome of an off-thread update check, delivered to the main thread.
pub(crate) struct UpdateCheckOutcome {
    pub result: UpdateCheckResult,
    pub should_save: bool,
    /// `true` for the user-initiated "Check Now" — skips the desktop notification
    /// (and last-notified recording) that the periodic check sends on a
    /// newly-discovered update, preserving the prior synchronous behavior.
    pub force: bool,
}

impl WindowManager {
    /// Check for updates (called periodically from about_to_wait)
    ///
    /// The blocking HTTP fetch runs off the main thread via
    /// `runtime.spawn_blocking`; results arrive through `update_check_rx` and
    /// are applied on the main thread by the drain at the top of this method.
    /// Previously the synchronous `ureq` GET (30s timeout to the GitHub releases
    /// API) ran on the main event-loop thread, freezing all I/O whenever the
    /// network was slow or the request hung.
    pub fn check_for_updates(&mut self) {
        use std::time::{Duration, Instant};

        // Apply any off-thread checks that completed since the last frame.
        // Logging, notifications, status sync, and config save all touch `self`
        // and must run here, on the main thread.
        while let Ok(outcome) = self.update_check_rx.try_recv() {
            self.process_update_result(outcome);
        }

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
            // Offload the blocking check off the main event-loop thread.
            // `load_full()` yields an owned `Arc<Config>` that is `Send`, unlike
            // `load()`'s thread-bound `Guard`. Mirrors the DynamicProfileManager
            // pattern of `spawn_blocking` for synchronous ureq fetches.
            let checker = std::sync::Arc::clone(&self.update_checker);
            let config = self.config.load_full();
            let tx = self.update_check_tx.clone();
            self.runtime.spawn_blocking(move || {
                let (result, should_save) = checker.check_now(&config, false);
                let _ = tx.send(UpdateCheckOutcome {
                    result,
                    should_save,
                    force: false,
                });
            });

            // Schedule the next check immediately so the timer doesn't re-fire
            // while one is in flight; `check_now`'s `check_in_progress` guard
            // also prevents duplicate concurrent network fetches.
            self.next_update_check = self
                .config
                .load()
                .updates
                .update_check_frequency
                .as_seconds()
                .map(|secs| now + Duration::from_secs(secs));
        }
    }

    /// Apply a completed update-check result on the main thread: log, notify,
    /// sync to status bar / update state, and persist the last-check timestamp.
    fn process_update_result(&mut self, outcome: UpdateCheckOutcome) {
        use crate::update_checker::current_timestamp;

        let UpdateCheckOutcome {
            result,
            should_save,
            force,
        } = outcome;
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

                // Only the periodic check desktop-notifies (and records the
                // last-notified version); force checks just log, preserving
                // the prior synchronous behavior.
                if !force {
                    let already_notified = self
                        .config
                        .load()
                        .updates
                        .last_notified_version
                        .as_ref()
                        .is_some_and(|v| v == &version_str);

                    if !already_notified {
                        notify_update_available(info);
                        self.config.rcu(|old| {
                            let mut new = (**old).clone();
                            new.updates.last_notified_version = Some(version_str.clone());
                            std::sync::Arc::new(new)
                        });
                        config_changed = true;
                    }
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

        // Sync to the settings window if open — covers both the periodic check
        // and the user-triggered "Check Now", whose off-thread result lands here.
        if let Some(settings_window) = &mut self.settings_window {
            settings_window.settings_ui.last_update_result = self
                .last_update_result
                .as_ref()
                .map(to_settings_update_result);
            settings_window.request_redraw();
        }

        // Save config with updated timestamp if check was successful
        if config_changed {
            self.config.rcu(|old| {
                let mut new = (**old).clone();
                new.updates.last_update_check = Some(current_timestamp());
                std::sync::Arc::new(new)
            });
            if let Err(e) = self.config.load().save() {
                log::warn!("Failed to save config after update check: {}", e);
            }
        }
    }

    /// Force an immediate update check (triggered from UI).
    ///
    /// Like the periodic check, the blocking HTTP fetch runs off the main thread
    /// via `runtime.spawn_blocking`; the result is applied by
    /// `process_update_result` (with `force = true`) when it arrives. Previously
    /// this ran `check_now` synchronously on the main thread, freezing all I/O
    /// for up to the 30s ureq timeout when the network was slow.
    pub fn force_update_check(&mut self) {
        let checker = std::sync::Arc::clone(&self.update_checker);
        let config = self.config.load_full();
        let tx = self.update_check_tx.clone();
        self.runtime.spawn_blocking(move || {
            let (result, should_save) = checker.check_now(&config, true);
            let _ = tx.send(UpdateCheckOutcome {
                result,
                should_save,
                force: true,
            });
        });
    }

    /// Force an update check from the settings window. The result is synced back
    /// to the settings window asynchronously by `process_update_result` when the
    /// off-thread check completes; the settings "Check Now" button no longer
    /// blocks the event loop.
    pub fn force_update_check_for_settings(&mut self) {
        self.force_update_check();
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
        crate::platform::deliver_desktop_notification(
            "par-term Test Notification",
            "If you see this, notifications are working!",
            5000,
            crate::platform::NotificationUrgency::Normal,
        );
    }
}

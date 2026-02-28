//! Self-update flow state for the window manager.
//!
//! Extracted from `WindowState` as part of the God Object decomposition (ARC-001).

use anyhow::Result;

/// State for the in-app self-update flow.
pub(crate) struct UpdateState {
    /// Whether to show the update dialog overlay (set when user clicks the update widget)
    pub(crate) show_dialog: bool,
    /// Last update check result (for update dialog)
    pub(crate) last_result: Option<crate::update_checker::UpdateCheckResult>,
    /// Detected installation type
    pub(crate) installation_type: par_term_settings_ui::InstallationType,
    /// Whether an update install is in progress (from the update dialog)
    pub(crate) installing: bool,
    /// Status message from the update install
    pub(crate) install_status: Option<String>,
    /// Channel receiver for async update install result
    pub(crate) install_receiver:
        Option<std::sync::mpsc::Receiver<Result<crate::self_updater::UpdateResult, String>>>,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            show_dialog: false,
            last_result: None,
            installation_type: par_term_settings_ui::InstallationType::StandaloneBinary,
            installing: false,
            install_status: None,
            install_receiver: None,
        }
    }
}

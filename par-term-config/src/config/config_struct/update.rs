use serde::{Deserialize, Serialize};
use crate::types::UpdateCheckFrequency;

/// Configuration for automatic update checking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    /// How often to check for new par-term releases
    /// - never: Disable automatic update checks
    /// - daily: Check once per day
    /// - weekly: Check once per week (default)
    /// - monthly: Check once per month
    #[serde(default = "crate::defaults::update_check_frequency")]
    pub update_check_frequency: UpdateCheckFrequency,

    /// ISO 8601 timestamp of the last update check (auto-managed)
    #[serde(default)]
    pub last_update_check: Option<String>,

    /// Version that user chose to skip notifications for
    #[serde(default)]
    pub skipped_version: Option<String>,

    /// Last version we notified the user about (prevents repeat notifications)
    #[serde(default)]
    pub last_notified_version: Option<String>,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            update_check_frequency: crate::defaults::update_check_frequency(),
            last_update_check: None,
            skipped_version: None,
            last_notified_version: None,
        }
    }
}

//! Integration, update, and progress bar configuration types.

use serde::{Deserialize, Serialize};

// ============================================================================
// Integration / Install Prompt Types
// ============================================================================

/// Shader install prompt mode
///
/// Controls whether the user is prompted to install shaders when the shaders
/// folder is missing or empty on startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ShaderInstallPrompt {
    /// Ask the user if they want to install shaders (default)
    #[default]
    Ask,
    /// Never ask - user declined installation
    Never,
    /// Shaders have been installed
    Installed,
}

/// State of an integration's install prompt
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InstallPromptState {
    /// Prompt user when appropriate (default)
    #[default]
    Ask,
    /// User said "never ask again"
    Never,
    /// Currently installed
    Installed,
}

impl InstallPromptState {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Ask => "Ask",
            Self::Never => "Never",
            Self::Installed => "Installed",
        }
    }
}

/// Tracks installed and prompted versions for integrations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntegrationVersions {
    /// Version when shaders were installed
    pub shaders_installed_version: Option<String>,
    /// Version when user was last prompted about shaders
    pub shaders_prompted_version: Option<String>,
    /// Version when shell integration was installed
    pub shell_integration_installed_version: Option<String>,
    /// Version when user was last prompted about shell integration
    pub shell_integration_prompted_version: Option<String>,
}

// ============================================================================
// Update Check Types
// ============================================================================

/// Update check frequency
///
/// Controls how often par-term checks GitHub for new releases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UpdateCheckFrequency {
    /// Never check for updates
    Never,
    /// Check once per hour
    Hourly,
    /// Check once per day (default)
    #[default]
    Daily,
    /// Check once per week
    Weekly,
    /// Check once per month
    Monthly,
}

impl UpdateCheckFrequency {
    /// Get the duration in seconds for this frequency
    pub fn as_seconds(&self) -> Option<u64> {
        match self {
            UpdateCheckFrequency::Never => None,
            UpdateCheckFrequency::Hourly => Some(3600),
            UpdateCheckFrequency::Daily => Some(24 * 60 * 60), // 86400
            UpdateCheckFrequency::Weekly => Some(7 * 24 * 60 * 60), // 604800
            UpdateCheckFrequency::Monthly => Some(30 * 24 * 60 * 60), // 2592000
        }
    }

    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            UpdateCheckFrequency::Never => "Never",
            UpdateCheckFrequency::Hourly => "Hourly",
            UpdateCheckFrequency::Daily => "Daily",
            UpdateCheckFrequency::Weekly => "Weekly",
            UpdateCheckFrequency::Monthly => "Monthly",
        }
    }
}

// ============================================================================
// Progress Bar Types
// ============================================================================

/// Progress bar visual style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProgressBarStyle {
    /// Thin bar line (default)
    #[default]
    Bar,
    /// Bar with percentage text
    BarWithText,
}

impl ProgressBarStyle {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            ProgressBarStyle::Bar => "Bar",
            ProgressBarStyle::BarWithText => "Bar with Text",
        }
    }

    /// All available styles for UI iteration
    pub fn all() -> &'static [ProgressBarStyle] {
        &[ProgressBarStyle::Bar, ProgressBarStyle::BarWithText]
    }
}

/// Progress bar position on screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProgressBarPosition {
    /// Top of the terminal window (default)
    #[default]
    Top,
    /// Bottom of the terminal window
    Bottom,
}

impl ProgressBarPosition {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            ProgressBarPosition::Bottom => "Bottom",
            ProgressBarPosition::Top => "Top",
        }
    }

    /// All available positions for UI iteration
    pub fn all() -> &'static [ProgressBarPosition] {
        &[ProgressBarPosition::Top, ProgressBarPosition::Bottom]
    }
}

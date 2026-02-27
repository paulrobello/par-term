//! Alert sound configuration types.

use serde::{Deserialize, Serialize};

// ============================================================================
// Alert Sound Types
// ============================================================================

/// Terminal events that can trigger alert sounds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertEvent {
    /// Bell character received (BEL / 0x07)
    Bell,
    /// Command completed (requires shell integration)
    CommandComplete,
    /// A new tab was created
    NewTab,
    /// A tab was closed
    TabClose,
}

impl AlertEvent {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            AlertEvent::Bell => "Bell",
            AlertEvent::CommandComplete => "Command Complete",
            AlertEvent::NewTab => "New Tab",
            AlertEvent::TabClose => "Tab Close",
        }
    }

    /// All available events for UI iteration
    pub fn all() -> &'static [AlertEvent] {
        &[
            AlertEvent::Bell,
            AlertEvent::CommandComplete,
            AlertEvent::NewTab,
            AlertEvent::TabClose,
        ]
    }
}

/// Configuration for an alert sound tied to a specific event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertSoundConfig {
    /// Whether this alert sound is enabled
    #[serde(default = "crate::defaults::bool_true")]
    pub enabled: bool,
    /// Volume 0-100 (0 effectively disables)
    #[serde(default = "crate::defaults::bell_sound")]
    pub volume: u8,
    /// Optional path to a custom sound file (WAV/OGG/FLAC).
    /// If None, uses built-in tone with the configured frequency.
    #[serde(default)]
    pub sound_file: Option<String>,
    /// Frequency in Hz for the built-in tone (used when sound_file is None)
    #[serde(default = "default_alert_frequency")]
    pub frequency: f32,
    /// Duration of the built-in tone in milliseconds
    #[serde(default = "default_alert_duration_ms")]
    pub duration_ms: u64,
}

fn default_alert_frequency() -> f32 {
    800.0
}

fn default_alert_duration_ms() -> u64 {
    100
}

impl Default for AlertSoundConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: 50,
            sound_file: None,
            frequency: 800.0,
            duration_ms: 100,
        }
    }
}

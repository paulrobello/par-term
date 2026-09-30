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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// What a terminal bell plays (UX.md B40: one bell model).
///
/// An enabled Alert Sounds › Bell entry owns the bell's audio; otherwise
/// the "Audio bell volume" setting does. A disabled entry no longer
/// silences the volume setting, and an enabled entry plays even when the
/// volume setting is 0.
#[derive(Debug, Clone, PartialEq)]
pub enum BellAudio<'a> {
    /// No sound.
    Silent,
    /// The plain bell tone at this volume (0-100).
    Tone(u8),
    /// The configured Alert Sounds › Bell sound.
    Alert(&'a AlertSoundConfig),
}

/// Decide the bell's audio from the volume setting and the Alert Sounds map.
pub fn bell_audio(
    bell_volume: u8,
    alert_sounds: &std::collections::HashMap<AlertEvent, AlertSoundConfig>,
) -> BellAudio<'_> {
    match alert_sounds.get(&AlertEvent::Bell) {
        Some(alert) if alert.enabled => {
            if alert.volume > 0 {
                BellAudio::Alert(alert)
            } else {
                BellAudio::Silent
            }
        }
        _ if bell_volume > 0 => BellAudio::Tone(bell_volume),
        _ => BellAudio::Silent,
    }
}

#[cfg(test)]
mod bell_audio_tests {
    use super::*;
    use std::collections::HashMap;

    fn bell_entry(enabled: bool, volume: u8) -> HashMap<AlertEvent, AlertSoundConfig> {
        let mut map = HashMap::new();
        map.insert(
            AlertEvent::Bell,
            AlertSoundConfig {
                enabled,
                volume,
                ..AlertSoundConfig::default()
            },
        );
        map
    }

    #[test]
    fn volume_plays_the_tone_when_no_alert_sound_is_set() {
        assert_eq!(bell_audio(60, &HashMap::new()), BellAudio::Tone(60));
        assert_eq!(bell_audio(0, &HashMap::new()), BellAudio::Silent);
    }

    #[test]
    fn a_disabled_alert_entry_no_longer_silences_the_volume() {
        // RT16: tick then untick Alert Sounds › Bell leaves a disabled entry.
        assert_eq!(bell_audio(60, &bell_entry(false, 50)), BellAudio::Tone(60));
    }

    #[test]
    fn an_enabled_alert_sound_plays_even_with_volume_zero() {
        let map = bell_entry(true, 40);
        assert!(matches!(bell_audio(0, &map), BellAudio::Alert(a) if a.volume == 40));
        assert!(matches!(bell_audio(80, &map), BellAudio::Alert(_)));
    }

    #[test]
    fn an_enabled_alert_sound_at_volume_zero_is_silent() {
        assert_eq!(bell_audio(80, &bell_entry(true, 0)), BellAudio::Silent);
    }
}

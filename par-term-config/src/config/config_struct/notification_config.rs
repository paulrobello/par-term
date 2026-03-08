//! Notification settings for the terminal emulator.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file — existing
//! config files remain 100% compatible.
//!
//! Covers bell (audio, visual, desktop), activity/silence alerts, anti-idle
//! keep-alive, and OSC 9/777 notification buffer limits.

use crate::types::{AlertEvent, AlertSoundConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Notification and alert settings for the terminal emulator.
///
/// Controls the bell (audio, visual, desktop notifications), activity and
/// silence alerts, the anti-idle keep-alive mechanism, and limits on the
/// OSC 9/777 notification buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// Forward BEL events to desktop notification centers
    #[serde(default = "crate::defaults::bool_false", alias = "bell_desktop")]
    pub notification_bell_desktop: bool,

    /// Volume (0-100) for backend bell sound alerts (0 disables)
    #[serde(default = "crate::defaults::bell_sound", alias = "bell_sound")]
    pub notification_bell_sound: u8,

    /// Enable backend visual bell overlay
    #[serde(default = "crate::defaults::bool_true", alias = "bell_visual")]
    pub notification_bell_visual: bool,

    /// Visual bell flash color [R, G, B] (0-255, default: white)
    #[serde(default = "crate::defaults::visual_bell_color")]
    pub notification_visual_bell_color: [u8; 3],

    /// Enable notifications when activity resumes after inactivity
    #[serde(
        default = "crate::defaults::bool_false",
        alias = "activity_notifications"
    )]
    pub notification_activity_enabled: bool,

    /// Seconds of inactivity required before an activity alert fires
    #[serde(
        default = "crate::defaults::activity_threshold",
        alias = "activity_threshold"
    )]
    pub notification_activity_threshold: u64,

    /// Enable anti-idle keep-alive (sends code after idle period)
    #[serde(default = "crate::defaults::bool_false")]
    pub anti_idle_enabled: bool,

    /// Seconds of inactivity before sending keep-alive code
    #[serde(default = "crate::defaults::anti_idle_seconds")]
    pub anti_idle_seconds: u64,

    /// ASCII code to send as keep-alive (e.g., 0 = NUL, 27 = ESC)
    #[serde(default = "crate::defaults::anti_idle_code")]
    pub anti_idle_code: u8,

    /// Enable notifications after prolonged silence
    #[serde(
        default = "crate::defaults::bool_false",
        alias = "silence_notifications"
    )]
    pub notification_silence_enabled: bool,

    /// Seconds of silence before a silence alert fires
    #[serde(
        default = "crate::defaults::silence_threshold",
        alias = "silence_threshold"
    )]
    pub notification_silence_threshold: u64,

    /// Enable notification when a shell/session exits
    #[serde(default = "crate::defaults::bool_false", alias = "session_ended")]
    pub notification_session_ended: bool,

    /// Suppress desktop notifications when the terminal window is focused
    #[serde(default = "crate::defaults::bool_true")]
    pub suppress_notifications_when_focused: bool,

    /// Maximum number of OSC 9/777 notification entries retained by backend
    #[serde(
        default = "crate::defaults::notification_max_buffer",
        alias = "max_notifications"
    )]
    pub notification_max_buffer: usize,

    /// Alert sound configuration per event type.
    ///
    /// Maps [`AlertEvent`] variants to their sound settings.
    #[serde(default)]
    pub alert_sounds: HashMap<AlertEvent, AlertSoundConfig>,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            notification_bell_desktop: crate::defaults::bool_false(),
            notification_bell_sound: crate::defaults::bell_sound(),
            notification_bell_visual: crate::defaults::bool_true(),
            notification_visual_bell_color: crate::defaults::visual_bell_color(),
            notification_activity_enabled: crate::defaults::bool_false(),
            notification_activity_threshold: crate::defaults::activity_threshold(),
            anti_idle_enabled: crate::defaults::bool_false(),
            anti_idle_seconds: crate::defaults::anti_idle_seconds(),
            anti_idle_code: crate::defaults::anti_idle_code(),
            notification_silence_enabled: crate::defaults::bool_false(),
            notification_silence_threshold: crate::defaults::silence_threshold(),
            notification_session_ended: crate::defaults::bool_false(),
            suppress_notifications_when_focused: crate::defaults::bool_true(),
            notification_max_buffer: crate::defaults::notification_max_buffer(),
            alert_sounds: HashMap::new(),
        }
    }
}

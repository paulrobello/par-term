//! Notifications settings tab.
//!
//! Consolidates: bell_tab (expanded)
//!
//! Contains:
//! - [`bell`]: Visual bell, audio bell volume, and desktop notifications
//! - [`activity`]: Activity, silence, and session notification settings
//! - [`alert_sounds`]: Per-event sound configuration
//! - [`behavior`]: Suppress-when-focused, buffer size, and test notification
//! - [`anti_idle`]: Anti-idle keep-alive settings

mod activity;
mod alert_sounds;
mod anti_idle;
mod behavior;
mod bell;

use super::SettingsUI;
use super::section::section_matches;
use std::collections::HashSet;

/// Show the notifications tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // Bell section
    if section_matches(
        &query,
        "Bell",
        &["visual", "audio", "sound", "beep", "volume", "flash"],
    ) {
        bell::show_bell_section(ui, settings, changes_this_frame, collapsed);
    }

    // Activity section
    if section_matches(
        &query,
        "Activity",
        &[
            "activity",
            "notify",
            "idle",
            "inactivity",
            "silence",
            "threshold",
            "session",
            "session ended",
            "shell exits",
        ],
    ) {
        activity::show_activity_section(ui, settings, changes_this_frame, collapsed);
    }

    // Alert sounds section
    if section_matches(
        &query,
        "Alert Sounds",
        &[
            "alert",
            "sound",
            "event",
            "command",
            "tab",
            "frequency",
            "duration",
            "wav",
            "ogg",
        ],
    ) {
        alert_sounds::show_alert_sounds_section(ui, settings, changes_this_frame, collapsed);
    }

    // Behavior section (collapsed by default)
    if section_matches(
        &query,
        "Behavior",
        &[
            "suppress",
            "focused",
            "buffer",
            "notification queue",
            "suppress when focused",
        ],
    ) {
        behavior::show_behavior_section(ui, settings, changes_this_frame, collapsed);
    }

    // Anti-Idle section (collapsed by default)
    if section_matches(
        &query,
        "Anti-Idle",
        &[
            "anti-idle",
            "keep-alive",
            "timeout",
            "ssh",
            "connection",
            "keep alive",
        ],
    ) {
        anti_idle::show_anti_idle_section(ui, settings, changes_this_frame, collapsed);
    }
}

/// Search keywords for the Notifications settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        // Bell
        "bell",
        "visual bell",
        "audio bell",
        "sound",
        "beep",
        "volume",
        "desktop notification",
        // Activity
        "notification",
        "activity",
        "activity notification",
        "activity threshold",
        "inactivity",
        // Silence
        "silence",
        "silence notification",
        "silence threshold",
        // Session
        "session ended",
        "shell exits",
        // Behavior
        "suppress",
        "focused",
        "suppress notifications",
        "buffer",
        "max buffer",
        "test notification",
        // Anti-idle
        "anti-idle",
        "anti idle",
        "keep-alive",
        "keepalive",
        "idle",
        "timeout",
        "ssh timeout",
        "connection timeout",
        "alert",
        // Alert sound extras
        "frequency",
        "duration",
        "sound file",
        "custom sound",
        // Anti-idle character
        "character",
        "ascii",
        "nul",
        "enq",
        "esc",
        "space",
        // Sound file formats
        "wav",
        "ogg",
        "flac",
    ]
}

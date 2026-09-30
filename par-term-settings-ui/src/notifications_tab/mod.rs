//! Notifications settings tab.
//!
//! Consolidates: bell_tab (expanded)
//!
//! Contains:
//! - `bell`: Visual bell, audio bell volume, and desktop notifications
//! - `activity`: Activity, silence, and session notification settings
//! - `alert_sounds`: Per-event sound configuration
//! - `behavior`: Suppress-when-focused, buffer size, and test notification
//! - `anti_idle`: Anti-idle keep-alive settings

mod activity;
mod alert_sounds;
mod anti_idle;
mod behavior;
mod bell;

use super::SettingsUI;
use std::collections::HashSet;

/// Show the notifications tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    // Bell section
    bell::show_bell_section(ui, settings, changes_this_frame, collapsed);

    // Activity section
    activity::show_activity_section(ui, settings, changes_this_frame, collapsed);

    // Alert sounds section
    alert_sounds::show_alert_sounds_section(ui, settings, changes_this_frame, collapsed);

    // Behavior section (collapsed by default)
    behavior::show_behavior_section(ui, settings, changes_this_frame, collapsed);

    // Anti-Idle section (collapsed by default)
    anti_idle::show_anti_idle_section(ui, settings, changes_this_frame, collapsed);
}

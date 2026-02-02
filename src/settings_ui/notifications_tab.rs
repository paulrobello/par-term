//! Notifications settings tab.
//!
//! Consolidates: bell_tab (expanded)
//!
//! Contains:
//! - Bell settings (visual, audio, desktop)
//! - Activity notifications
//! - Silence notifications
//! - Session notifications
//! - Notification behavior
//! - Anti-idle keep-alive

use super::section::{collapsing_section, SLIDER_WIDTH};
use super::SettingsUI;

const SLIDER_HEIGHT: f32 = 18.0;

/// Show the notifications tab content.
pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    let query = settings.search_query.trim().to_lowercase();

    // Bell section
    if section_matches(&query, "Bell", &["visual", "audio", "sound", "beep"]) {
        show_bell_section(ui, settings, changes_this_frame);
    }

    // Activity section
    if section_matches(&query, "Activity", &["activity", "notify", "idle"]) {
        show_activity_section(ui, settings, changes_this_frame);
    }

    // Behavior section (collapsed by default)
    if section_matches(&query, "Behavior", &["suppress", "focused", "buffer"]) {
        show_behavior_section(ui, settings, changes_this_frame);
    }

    // Anti-Idle section (collapsed by default)
    if section_matches(&query, "Anti-Idle", &["anti-idle", "keep-alive", "timeout"]) {
        show_anti_idle_section(ui, settings, changes_this_frame);
    }
}

fn section_matches(query: &str, title: &str, keywords: &[&str]) -> bool {
    if query.is_empty() {
        return true;
    }
    if title.to_lowercase().contains(query) {
        return true;
    }
    keywords.iter().any(|k| k.to_lowercase().contains(query))
}

// ============================================================================
// Bell Section
// ============================================================================

fn show_bell_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Bell", "notifications_bell", true, |ui| {
        if ui
            .checkbox(&mut settings.config.notification_bell_visual, "Visual bell")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.horizontal(|ui| {
            ui.label("Audio bell volume (0=off):");
            if ui
                .add_sized([SLIDER_WIDTH, SLIDER_HEIGHT], egui::Slider::new(
                    &mut settings.config.notification_bell_sound,
                    0..=100,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.notification_bell_desktop,
                "Desktop notifications for bell",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

// ============================================================================
// Activity Section
// ============================================================================

fn show_activity_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Activity", "notifications_activity", true, |ui| {
        ui.label("Activity Notifications:");
        if ui
            .checkbox(
                &mut settings.config.notification_activity_enabled,
                "Notify on activity after inactivity",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.horizontal(|ui| {
            ui.label("Activity threshold (seconds):");
            if ui
                .add_sized([SLIDER_WIDTH, SLIDER_HEIGHT], egui::Slider::new(
                    &mut settings.config.notification_activity_threshold,
                    1..=300,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.separator();
        ui.label("Silence Notifications:");
        if ui
            .checkbox(
                &mut settings.config.notification_silence_enabled,
                "Notify after prolonged silence",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.horizontal(|ui| {
            ui.label("Silence threshold (seconds):");
            if ui
                .add_sized([SLIDER_WIDTH, SLIDER_HEIGHT], egui::Slider::new(
                    &mut settings.config.notification_silence_threshold,
                    1..=600,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.separator();
        ui.label("Session Notifications:");
        if ui
            .checkbox(
                &mut settings.config.notification_session_ended,
                "Notify when session/shell exits",
            )
            .on_hover_text("Send a desktop notification when the shell process exits")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

// ============================================================================
// Behavior Section
// ============================================================================

fn show_behavior_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Behavior", "notifications_behavior", false, |ui| {
        if ui
            .checkbox(
                &mut settings.config.suppress_notifications_when_focused,
                "Suppress notifications when focused",
            )
            .on_hover_text("Skip desktop notifications when the terminal window is focused")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.horizontal(|ui| {
            ui.label("Max notification buffer:");
            if ui
                .add_sized([SLIDER_WIDTH, SLIDER_HEIGHT], egui::Slider::new(
                    &mut settings.config.notification_max_buffer,
                    10..=1000,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui
                .button("Test Notification")
                .on_hover_text("Send a test notification to verify permissions are granted")
                .clicked()
            {
                settings.test_notification_requested = true;
            }
            #[cfg(target_os = "macos")]
            {
                if ui
                    .button("Open System Preferences")
                    .on_hover_text("Open macOS notification settings")
                    .clicked()
                {
                    let _ = std::process::Command::new("open")
                        .arg("x-apple.systempreferences:com.apple.preference.notifications")
                        .spawn();
                }
            }
        });
    });
}

// ============================================================================
// Anti-Idle Section
// ============================================================================

fn show_anti_idle_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Anti-Idle Keep-Alive", "notifications_anti_idle", false, |ui| {
        ui.label(
            "Prevents SSH and connection timeouts by periodically sending invisible characters.",
        );
        ui.add_space(4.0);

        if ui
            .checkbox(
                &mut settings.config.anti_idle_enabled,
                "Send code when idle",
            )
            .on_hover_text("Periodically send a character to keep connections alive")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.horizontal(|ui| {
            ui.label("Seconds before sending:");
            if ui
                .add(
                    egui::DragValue::new(&mut settings.config.anti_idle_seconds)
                        .range(10..=3600)
                        .speed(1.0),
                )
                .on_hover_text("How long to wait before sending keep-alive (10-3600 seconds)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Character to send:");
            egui::ComboBox::from_id_salt("notifications_anti_idle_code")
                .selected_text(match settings.config.anti_idle_code {
                    0 => "NUL (0x00)",
                    5 => "ENQ (0x05)",
                    27 => "ESC (0x1B)",
                    32 => "Space (0x20)",
                    _ => "Custom",
                })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(
                            &mut settings.config.anti_idle_code,
                            0,
                            "NUL (0x00) - Null character, most common",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                    if ui
                        .selectable_value(
                            &mut settings.config.anti_idle_code,
                            27,
                            "ESC (0x1B) - Escape, safe for most apps",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                    if ui
                        .selectable_value(
                            &mut settings.config.anti_idle_code,
                            5,
                            "ENQ (0x05) - Enquiry, may trigger answerback",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                    if ui
                        .selectable_value(
                            &mut settings.config.anti_idle_code,
                            32,
                            "Space (0x20) - Visible but harmless",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
        });

        ui.horizontal(|ui| {
            ui.label("Custom ASCII code:");
            if ui
                .add(
                    egui::DragValue::new(&mut settings.config.anti_idle_code)
                        .range(0..=127)
                        .speed(1.0),
                )
                .on_hover_text("ASCII code (0-127) to send as keep-alive")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            ui.label(format!("(0x{:02X})", settings.config.anti_idle_code));
        });
    });
}

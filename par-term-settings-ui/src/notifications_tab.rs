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

use super::SettingsUI;
use super::section::{SLIDER_WIDTH, collapsing_section, section_matches};
use par_term_config::AlertEvent;
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

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
        show_bell_section(ui, settings, changes_this_frame, collapsed);
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
        show_activity_section(ui, settings, changes_this_frame, collapsed);
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
        show_alert_sounds_section(ui, settings, changes_this_frame, collapsed);
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
        show_behavior_section(ui, settings, changes_this_frame, collapsed);
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
        show_anti_idle_section(ui, settings, changes_this_frame, collapsed);
    }
}

// ============================================================================
// Bell Section
// ============================================================================

fn show_bell_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Bell", "notifications_bell", true, collapsed, |ui| {
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
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.notification_bell_sound, 0..=100),
                )
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
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Activity",
        "notifications_activity",
        true,
        collapsed,
        |ui| {
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
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.notification_activity_threshold,
                            1..=300,
                        ),
                    )
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
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.notification_silence_threshold,
                            1..=600,
                        ),
                    )
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
        },
    );
}

// ============================================================================
// Behavior Section
// ============================================================================

fn show_behavior_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Behavior",
        "notifications_behavior",
        false,
        collapsed,
        |ui| {
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
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(&mut settings.config.notification_max_buffer, 10..=1000),
                    )
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
        },
    );
}

// ============================================================================
// Anti-Idle Section
// ============================================================================

fn show_anti_idle_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Anti-Idle Keep-Alive",
        "notifications_anti_idle",
        false,
        collapsed,
        |ui| {
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
        },
    );
}

// ============================================================================
// Alert Sounds Section
// ============================================================================

fn show_alert_sounds_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Alert Sounds",
        "notifications_alert_sounds",
        false,
        collapsed,
        |ui| {
            ui.label("Configure sounds for terminal events. Leave unconfigured to use defaults.");
            ui.add_space(4.0);

            for event in AlertEvent::all() {
                let id_str = format!("alert_{:?}", event);
                let has_config = settings.config.alert_sounds.contains_key(event);

                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        let mut enabled = has_config
                            && settings
                                .config
                                .alert_sounds
                                .get(event)
                                .is_some_and(|c| c.enabled);

                        if ui.checkbox(&mut enabled, event.display_name()).changed() {
                            if enabled {
                                settings
                                    .config
                                    .alert_sounds
                                    .entry(*event)
                                    .or_default()
                                    .enabled = true;
                            } else if let Some(cfg) = settings.config.alert_sounds.get_mut(event) {
                                cfg.enabled = false;
                            }
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });

                    if has_config
                        && settings
                            .config
                            .alert_sounds
                            .get(event)
                            .is_some_and(|c| c.enabled)
                    {
                        let cfg = settings.config.alert_sounds.get_mut(event).expect("alert_sounds entry verified present by get().is_some_and() guard above");

                        ui.horizontal(|ui| {
                            ui.label("  Volume:");
                            if ui
                                .add_sized(
                                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                                    egui::Slider::new(&mut cfg.volume, 0..=100),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("  Frequency (Hz):");
                            if ui
                                .add_sized(
                                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                                    egui::Slider::new(&mut cfg.frequency, 200.0..=2000.0)
                                        .step_by(50.0),
                                )
                                .on_hover_text("Tone frequency for built-in sound")
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("  Duration (ms):");
                            if ui
                                .add_sized(
                                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                                    egui::Slider::new(&mut cfg.duration_ms, 10..=1000)
                                        .step_by(10.0),
                                )
                                .on_hover_text("Duration of the alert tone")
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });

                        // Sound file path (optional)
                        ui.horizontal(|ui| {
                            ui.label("  Sound file:");
                            let mut file_str = cfg.sound_file.clone().unwrap_or_default();
                            let response = ui.add_sized(
                                [SLIDER_WIDTH, SLIDER_HEIGHT],
                                egui::TextEdit::singleline(&mut file_str)
                                    .hint_text("(optional WAV/OGG/FLAC path)"),
                            );
                            if response.changed() {
                                cfg.sound_file = if file_str.is_empty() {
                                    None
                                } else {
                                    Some(file_str)
                                };
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });
                    }
                });

                // suppress unused variable warning
                let _ = &id_str;
            }
        },
    );
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

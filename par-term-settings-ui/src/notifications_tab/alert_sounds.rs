//! Alert sounds settings — per-event sound configuration.

use crate::SettingsUI;
use crate::section::{SLIDER_WIDTH, collapsing_section};
use par_term_config::AlertEvent;
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub(super) fn show_alert_sounds_section(
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
                            } else if let Some(cfg) =
                                settings.config.alert_sounds.get_mut(event)
                            {
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
                        let cfg = settings.config.alert_sounds.get_mut(event).expect(
                            "alert_sounds entry verified present by get().is_some_and() guard above",
                        );

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
                            if ui.button("Browse…").clicked() {
                                let sounds_dir = dirs::config_dir()
                                    .map(|d| d.join("par-term").join("sounds"))
                                    .unwrap_or_default();
                                if let Some(path) = rfd::FileDialog::new()
                                    .set_title("Select alert sound file")
                                    .set_directory(&sounds_dir)
                                    .add_filter(
                                        "Audio",
                                        &["wav", "mp3", "ogg", "flac", "aac", "m4a"],
                                    )
                                    .pick_file()
                                {
                                    // If inside sounds dir, store relative; otherwise full path.
                                    cfg.sound_file = Some(
                                        path.strip_prefix(&sounds_dir)
                                            .map(|p| p.display().to_string())
                                            .unwrap_or_else(|_| path.display().to_string()),
                                    );
                                    settings.has_changes = true;
                                    *changes_this_frame = true;
                                }
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

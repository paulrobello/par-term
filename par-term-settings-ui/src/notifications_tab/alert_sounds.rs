//! Alert sounds settings — per-event sound configuration.

use crate::SettingsUI;
use crate::section::{SLIDER_WIDTH, collapsing_section};
use par_term_config::{AlertEvent, AlertSoundConfig};
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
                ui.push_id(("alert_sound", *event), |ui| {
                    ui.group(|ui| {
                        show_alert_event(ui, settings, changes_this_frame, *event);
                    });
                });
            }
        },
    );
}

/// One event's row: the on/off checkbox, then its sound settings indented
/// under it and disabled while it is off (UX.md SC1; they used to be hidden).
fn show_alert_event(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    event: AlertEvent,
) {
    let sounds = &mut settings.config.notifications.alert_sounds;
    let mut enabled = sounds.get(&event).is_some_and(|c| c.enabled);
    if ui.checkbox(&mut enabled, event.display_name()).changed() {
        if enabled {
            sounds.entry(event).or_default().enabled = true;
        } else if let Some(cfg) = sounds.get_mut(&event) {
            cfg.enabled = false;
        }
        settings.has_changes = true;
        *changes_this_frame = true;
    }

    let parent = event.display_name();
    if enabled {
        let cfg = settings
            .config
            .notifications
            .alert_sounds
            .get_mut(&event)
            .expect("an enabled alert sound has an entry");
        if crate::dependent::dependent(ui, true, parent, |ui| sound_rows(ui, cfg)) {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    } else {
        // Nothing is stored until the event is turned on, so draw the values
        // it would start with; they cannot be edited while it is off.
        let mut preview = settings
            .config
            .notifications
            .alert_sounds
            .get(&event)
            .cloned()
            .unwrap_or_default();
        crate::dependent::dependent(ui, false, parent, |ui| sound_rows(ui, &mut preview));
    }
}

/// Volume, tone, duration, and sound file for one alert. Returns whether any
/// of them changed.
fn sound_rows(ui: &mut egui::Ui, cfg: &mut AlertSoundConfig) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Volume:");
        changed |= ui
            .add_sized(
                [SLIDER_WIDTH, SLIDER_HEIGHT],
                egui::Slider::new(&mut cfg.volume, 0..=100).suffix(" %"),
            )
            .changed();
    });
    ui.horizontal(|ui| {
        ui.label("Frequency:");
        changed |= ui
            .add_sized(
                [SLIDER_WIDTH, SLIDER_HEIGHT],
                egui::Slider::new(&mut cfg.frequency, 200.0..=2000.0)
                    .suffix(" Hz")
                    .step_by(50.0),
            )
            .on_hover_text("Tone frequency for the built-in sound")
            .changed();
    });
    ui.horizontal(|ui| {
        ui.label("Duration:");
        changed |= ui
            .add_sized(
                [SLIDER_WIDTH, SLIDER_HEIGHT],
                egui::Slider::new(&mut cfg.duration_ms, 10..=1000)
                    .suffix(" ms")
                    .step_by(10.0),
            )
            .on_hover_text("Duration of the alert tone")
            .changed();
    });
    ui.horizontal(|ui| {
        ui.label("Sound file:");
        let mut file_str = cfg.sound_file.clone().unwrap_or_default();
        if ui
            .add_sized(
                [SLIDER_WIDTH, SLIDER_HEIGHT],
                egui::TextEdit::singleline(&mut file_str)
                    .hint_text("(optional WAV/MP3/OGG/FLAC path)"),
            )
            .changed()
        {
            cfg.sound_file = (!file_str.is_empty()).then_some(file_str);
            changed = true;
        }
        if ui.button("Browse…").clicked() {
            let sounds_dir = dirs::config_dir()
                .map(|d| d.join("par-term").join("sounds"))
                .unwrap_or_default();
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Select alert sound file")
                .set_directory(&sounds_dir)
                .add_filter("Audio", &["wav", "mp3", "ogg", "flac", "aac", "m4a"])
                .pick_file()
            {
                // Inside the sounds dir, store relative; otherwise the full path.
                cfg.sound_file = Some(
                    path.strip_prefix(&sounds_dir)
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| path.display().to_string()),
                );
                changed = true;
            }
        }
    });
    changed
}

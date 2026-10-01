//! Leader key section: the chord that arms the which-key table, its
//! timing, and the vim-style focus keys (UX.md K4/K7).

use crate::SettingsUI;
use crate::search::SearchTag;
use crate::section::{SLIDER_WIDTH, keyword_section};
use std::collections::HashSet;

use super::keybindings::{capture_key_combo, display_key_combo};

/// The parent the timing and vim-key controls depend on (UX.md SC1): they
/// do nothing while the leader chord is empty.
const LEADER_PARENT: &str = "Leader chord: a chord recorded";

pub(crate) fn show_leader_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "Leader Key",
        "input_leader",
        &["leader", "which-key", "prefix", "vim"],
        true,
        collapsed,
        |ui| {
            ui.label(
                "Press the leader chord, then one key, to run a window, tab, pane, or \
                 par-mux session action. A which-key overlay lists the table.",
            );
            ui.add_space(4.0);

            if settings.leader_recording {
                if let Some(combo) = capture_key_combo(ui) {
                    settings.leader_conflict =
                        settings.check_recorded_chord_conflict(&combo, "leader_key");
                    settings.config.input.leader_key = combo;
                    settings.leader_recording = false;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    settings.leader_recording = false;
                    settings.leader_conflict = None;
                }
            }

            ui.horizontal(|ui| {
                ui.label("Leader chord:");
                if settings.leader_recording {
                    ui.colored_label(egui::Color32::YELLOW, "Press key combo... (Esc to cancel)");
                } else if settings.config.input.leader_key.trim().is_empty() {
                    ui.monospace("(off)");
                } else {
                    ui.monospace(display_key_combo(&settings.config.input.leader_key));
                }

                let label = if settings.leader_recording {
                    "Cancel"
                } else {
                    "Record"
                };
                let record = ui.button(label).search_tag(&["leader_key"]);
                if record.clicked() {
                    settings.leader_recording = !settings.leader_recording;
                    settings.leader_conflict = None;
                }

                if !settings.leader_recording
                    && !settings.config.input.leader_key.is_empty()
                    && ui
                        .button("Turn off")
                        .on_hover_text("An empty chord disables the leader key")
                        .clicked()
                {
                    settings.config.input.leader_key.clear();
                    settings.leader_conflict = None;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                    &mut c.input.leader_key
                });
            });

            if let Some(conflict) = &settings.leader_conflict {
                ui.colored_label(
                    egui::Color32::RED,
                    format!("Leader key conflict: {conflict}"),
                );
            }

            ui.add_space(8.0);

            let leader_on = !settings.config.input.leader_key.trim().is_empty();
            crate::dependent::dependent(ui, leader_on, LEADER_PARENT, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Timeout:");
                    if ui
                        .add_sized(
                            [SLIDER_WIDTH, 20.0],
                            egui::Slider::new(
                                &mut settings.config.input.leader_timeout_ms,
                                250..=10000,
                            )
                            .suffix(" ms"),
                        )
                        .search_tag(&["leader_timeout_ms"])
                        .on_hover_text("How long the armed leader waits for its next key")
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                    crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                        &mut c.input.leader_timeout_ms
                    });
                });

                ui.horizontal(|ui| {
                    ui.label("Overlay delay:");
                    if ui
                        .add_sized(
                            [SLIDER_WIDTH, 20.0],
                            egui::Slider::new(
                                &mut settings.config.input.leader_overlay_delay_ms,
                                0..=3000,
                            )
                            .suffix(" ms"),
                        )
                        .search_tag(&["leader_overlay_delay_ms"])
                        .on_hover_text("How long after the leader its which-key overlay appears")
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                    crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                        &mut c.input.leader_overlay_delay_ms
                    });
                });

                ui.horizontal(|ui| {
                    if ui
                        .checkbox(
                            &mut settings.config.input.leader_vim_keys,
                            "Vim-style focus and swap keys",
                        )
                        .search_tag(&["leader_vim_keys"])
                        .on_hover_text(
                            "Adds h j k l (focus) and H J K L (swap) to the leader table. \
                             The last-used tab key moves from l to Tab.",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                    crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                        &mut c.input.leader_vim_keys
                    });
                });
            });
        },
    );
}

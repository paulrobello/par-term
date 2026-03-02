//! Status bar poll intervals section (system monitor, git branch refresh rates).

use crate::SettingsUI;
use crate::section::{SLIDER_WIDTH, collapsing_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

pub fn show_poll_intervals_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Poll Intervals",
        "status_bar_poll_intervals",
        false,
        collapsed,
        |ui| {
            ui.horizontal(|ui| {
                ui.label("System monitor:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.status_bar.status_bar_system_poll_interval,
                            0.5..=30.0,
                        )
                        .suffix(" sec")
                        .show_value(true),
                    )
                    .on_hover_text("How often to poll CPU, memory, and network usage")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Git branch:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.status_bar.status_bar_git_poll_interval,
                            1.0..=60.0,
                        )
                        .suffix(" sec")
                        .show_value(true),
                    )
                    .on_hover_text("How often to poll the current git branch name")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        },
    );
}

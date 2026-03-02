//! Render cache section for the Content Prettifier tab.

use crate::SettingsUI;
use crate::section::collapsing_section;
use std::collections::HashSet;

pub fn show_cache_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Render Cache",
        "prettifier_cache",
        false,
        collapsed,
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Max cache entries:");
                if ui
                    .add(
                        egui::DragValue::new(
                            &mut settings.config.content_prettifier.cache.max_entries,
                        )
                        .range(8..=512)
                        .speed(1.0),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        },
    );
}

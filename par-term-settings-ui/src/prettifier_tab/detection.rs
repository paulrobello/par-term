//! Detection settings section for the Content Prettifier tab.

use crate::SettingsUI;
use crate::section::collapsing_section;
use std::collections::HashSet;

pub fn show_detection_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Detection Settings",
        "prettifier_detection",
        true,
        collapsed,
        |ui| {
            // Detection scope dropdown.
            ui.horizontal(|ui| {
                ui.label("Detection scope:");
                let scope = &mut settings.config.content_prettifier.detection.scope;
                let label = match scope.as_str() {
                    "command_output" => "Command Output",
                    "all" => "All Output",
                    "manual_only" => "Manual Only",
                    _ => "Command Output",
                };
                egui::ComboBox::from_id_salt("prettifier_detection_scope")
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(scope == "command_output", "Command Output")
                            .clicked()
                        {
                            *scope = "command_output".to_string();
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        if ui.selectable_label(scope == "all", "All Output").clicked() {
                            *scope = "all".to_string();
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        if ui
                            .selectable_label(scope == "manual_only", "Manual Only")
                            .clicked()
                        {
                            *scope = "manual_only".to_string();
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });
            });

            // Confidence threshold slider.
            ui.horizontal(|ui| {
                ui.label("Confidence threshold:");
                let threshold = &mut settings
                    .config
                    .content_prettifier
                    .detection
                    .confidence_threshold;
                if ui
                    .add(egui::Slider::new(threshold, 0.0..=1.0).step_by(0.05))
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Max scan lines.
            ui.horizontal(|ui| {
                ui.label("Max scan lines:");
                if ui
                    .add(
                        egui::DragValue::new(
                            &mut settings.config.content_prettifier.detection.max_scan_lines,
                        )
                        .range(50..=5000)
                        .speed(10.0),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Debounce ms.
            ui.horizontal(|ui| {
                ui.label("Debounce (ms):");
                if ui
                    .add(
                        egui::DragValue::new(
                            &mut settings.config.content_prettifier.detection.debounce_ms,
                        )
                        .range(0..=1000)
                        .speed(10.0),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Per-block toggle.
            if ui
                .checkbox(
                    &mut settings.config.content_prettifier.per_block_toggle,
                    "Per-block source/rendered toggle",
                )
                .on_hover_text("Allow toggling between source and rendered view per content block")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            // Respect alternate screen.
            if ui
                .checkbox(
                    &mut settings.config.content_prettifier.respect_alternate_screen,
                    "Respect alternate screen",
                )
                .on_hover_text("Treat alternate screen transitions as content block boundaries")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        },
    );
}

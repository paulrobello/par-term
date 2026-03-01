//! Test detection section for the Content Prettifier tab.

use crate::SettingsUI;
use crate::section::collapsing_section;
use std::collections::HashSet;

pub fn show_test_detection_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Test Detection",
        "prettifier_test_detection",
        true,
        collapsed,
        |ui| {
            ui.label("Paste sample content to test which format the detector identifies:");
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .max_height(150.0)
                .id_salt("test_detection_content")
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut settings.test_detection_content)
                            .desired_width(f32::INFINITY)
                            .desired_rows(6)
                            .font(egui::TextStyle::Monospace)
                            .hint_text("Paste sample output here…"),
                    );
                });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Preceding command (optional):");
                ui.add(
                    egui::TextEdit::singleline(&mut settings.test_detection_command)
                        .desired_width(200.0)
                        .hint_text("e.g. git diff"),
                );
            });

            ui.add_space(4.0);
            if ui
                .add_enabled(
                    !settings.test_detection_content.is_empty(),
                    egui::Button::new("Test Detection"),
                )
                .clicked()
            {
                settings.test_detection_requested = true;
            }

            // Display results
            if let Some((ref format_id, confidence, ref matched_rules, threshold)) =
                settings.test_detection_result
            {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                if format_id.is_empty() {
                    ui.colored_label(
                        egui::Color32::from_rgb(200, 100, 100),
                        "No format detected.",
                    );
                } else {
                    let passed = confidence >= threshold;
                    let status = if passed { "PASS" } else { "BELOW THRESHOLD" };
                    let status_color = if passed {
                        egui::Color32::from_rgb(100, 200, 100)
                    } else {
                        egui::Color32::from_rgb(200, 180, 80)
                    };

                    egui::Grid::new("test_detection_results")
                        .num_columns(2)
                        .spacing([10.0, 4.0])
                        .show(ui, |ui| {
                            ui.label("Format:");
                            ui.strong(format_id);
                            ui.end_row();

                            ui.label("Confidence:");
                            ui.horizontal(|ui| {
                                ui.label(format!("{:.0}%", confidence * 100.0));
                                ui.colored_label(status_color, format!("[{}]", status));
                            });
                            ui.end_row();

                            ui.label("Threshold:");
                            ui.label(format!("{:.0}%", threshold * 100.0));
                            ui.end_row();

                            if !matched_rules.is_empty() {
                                ui.label("Matched rules:");
                                ui.label(matched_rules.join(", "));
                                ui.end_row();
                            }
                        });
                }
            }
        },
    );
}

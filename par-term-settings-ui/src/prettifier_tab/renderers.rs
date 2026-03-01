//! Per-renderer cards section for the Content Prettifier tab.

use crate::SettingsUI;
use crate::section::collapsing_section;
use std::collections::HashSet;

pub fn show_renderers_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Renderers",
        "prettifier_renderers",
        true,
        collapsed,
        |ui| {
            ui.label("Enable or disable individual format renderers and adjust their priority.");
            ui.add_space(4.0);

            // Helper macro to avoid repeating the same pattern for each renderer.
            show_renderer_toggle(
                ui,
                "Markdown",
                "MD",
                &mut settings.config.content_prettifier.renderers.markdown,
                &mut settings.has_changes,
                changes_this_frame,
            );
            show_renderer_toggle(
                ui,
                "JSON",
                "{}",
                &mut settings.config.content_prettifier.renderers.json,
                &mut settings.has_changes,
                changes_this_frame,
            );
            show_renderer_toggle(
                ui,
                "YAML",
                "YML",
                &mut settings.config.content_prettifier.renderers.yaml,
                &mut settings.has_changes,
                changes_this_frame,
            );
            show_renderer_toggle(
                ui,
                "TOML",
                "TML",
                &mut settings.config.content_prettifier.renderers.toml,
                &mut settings.has_changes,
                changes_this_frame,
            );
            show_renderer_toggle(
                ui,
                "XML",
                "XML",
                &mut settings.config.content_prettifier.renderers.xml,
                &mut settings.has_changes,
                changes_this_frame,
            );
            show_renderer_toggle(
                ui,
                "CSV",
                "CSV",
                &mut settings.config.content_prettifier.renderers.csv,
                &mut settings.has_changes,
                changes_this_frame,
            );

            // Diff renderer with extra display_mode option.
            ui.horizontal(|ui| {
                if ui
                    .checkbox(
                        &mut settings.config.content_prettifier.renderers.diff.enabled,
                        "Diff",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                ui.label(
                    egui::RichText::new("+-")
                        .monospace()
                        .small()
                        .color(egui::Color32::from_rgb(100, 160, 255)),
                );
                ui.label("Priority:");
                if ui
                    .add(
                        egui::DragValue::new(
                            &mut settings.config.content_prettifier.renderers.diff.priority,
                        )
                        .range(1..=100)
                        .speed(1.0),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            show_renderer_toggle(
                ui,
                "Log",
                "LOG",
                &mut settings.config.content_prettifier.renderers.log,
                &mut settings.has_changes,
                changes_this_frame,
            );

            // Diagrams renderer with extra engine option.
            ui.horizontal(|ui| {
                if ui
                    .checkbox(
                        &mut settings
                            .config
                            .content_prettifier
                            .renderers
                            .diagrams
                            .enabled,
                        "Diagrams",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                ui.label(
                    egui::RichText::new("DG")
                        .monospace()
                        .small()
                        .color(egui::Color32::from_rgb(100, 160, 255)),
                );
                ui.label("Priority:");
                if ui
                    .add(
                        egui::DragValue::new(
                            &mut settings
                                .config
                                .content_prettifier
                                .renderers
                                .diagrams
                                .priority,
                        )
                        .range(1..=100)
                        .speed(1.0),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Diagram engine selection (indented under diagrams).
            if settings
                .config
                .content_prettifier
                .renderers
                .diagrams
                .enabled
            {
                ui.indent("diagram_settings", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Engine:");
                        let current = settings
                            .config
                            .content_prettifier
                            .renderers
                            .diagrams
                            .engine
                            .clone()
                            .unwrap_or_else(|| "auto".to_string());
                        egui::ComboBox::from_id_salt("diagram_engine")
                            .selected_text(&current)
                            .show_ui(ui, |ui| {
                                for opt in &["auto", "native", "local", "kroki", "text_fallback"] {
                                    if ui.selectable_label(current == *opt, *opt).clicked() {
                                        settings
                                            .config
                                            .content_prettifier
                                            .renderers
                                            .diagrams
                                            .engine = if *opt == "auto" {
                                            None
                                        } else {
                                            Some(opt.to_string())
                                        };
                                        settings.has_changes = true;
                                        *changes_this_frame = true;
                                    }
                                }
                            });
                    });

                    // Kroki server URL (only shown when engine is kroki).
                    let is_kroki = settings
                        .config
                        .content_prettifier
                        .renderers
                        .diagrams
                        .engine
                        .as_deref()
                        == Some("kroki");
                    if is_kroki {
                        ui.horizontal(|ui| {
                            ui.label("Kroki URL:");
                            let url = settings
                                .config
                                .content_prettifier
                                .renderers
                                .diagrams
                                .kroki_server
                                .get_or_insert_with(|| "https://kroki.io".to_string());
                            if ui
                                .add(egui::TextEdit::singleline(url).desired_width(250.0))
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });
                    }
                });
            }

            show_renderer_toggle(
                ui,
                "SQL Results",
                "SQL",
                &mut settings.config.content_prettifier.renderers.sql_results,
                &mut settings.has_changes,
                changes_this_frame,
            );
            show_renderer_toggle(
                ui,
                "Stack Trace",
                "STK",
                &mut settings.config.content_prettifier.renderers.stack_trace,
                &mut settings.has_changes,
                changes_this_frame,
            );
        },
    );
}

/// Show a single renderer toggle with enable checkbox and priority drag value.
pub fn show_renderer_toggle(
    ui: &mut egui::Ui,
    name: &str,
    badge: &str,
    toggle: &mut par_term_config::config::prettifier::RendererToggle,
    has_changes: &mut bool,
    changes_this_frame: &mut bool,
) {
    ui.horizontal(|ui| {
        if ui.checkbox(&mut toggle.enabled, name).changed() {
            *has_changes = true;
            *changes_this_frame = true;
        }
        ui.label(
            egui::RichText::new(badge)
                .monospace()
                .small()
                .color(egui::Color32::from_rgb(100, 160, 255)),
        );
        ui.label("Priority:");
        if ui
            .add(
                egui::DragValue::new(&mut toggle.priority)
                    .range(1..=100)
                    .speed(1.0),
            )
            .changed()
        {
            *has_changes = true;
            *changes_this_frame = true;
        }
    });
}

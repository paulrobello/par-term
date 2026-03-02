//! Custom renderers section for the Content Prettifier tab.

use crate::SettingsUI;
use crate::section::collapsing_section;
use std::collections::HashSet;

pub fn show_custom_renderers_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Custom Renderers",
        "prettifier_custom_renderers",
        false,
        collapsed,
        |ui| {
            ui.label("User-defined renderers that pipe content to external commands.");
            ui.label(
                egui::RichText::new(
                    "ANSI color output from external commands is preserved automatically.",
                )
                .small()
                .weak(),
            );
            ui.add_space(4.0);

            let mut delete_index: Option<usize> = None;
            let count = settings.config.content_prettifier.custom_renderers.len();

            for i in 0..count {
                let cr = &settings.config.content_prettifier.custom_renderers[i];
                let header_text = format!("{} ({})", cr.name, cr.id);

                // Use egui::CollapsingHeader directly to avoid nested
                // collapsing_section borrow conflicts.
                egui::CollapsingHeader::new(egui::RichText::new(&header_text).strong())
                    .id_salt(format!("custom_renderer_{i}"))
                    .default_open(false)
                    .show(ui, |ui| {
                        let cr = &mut settings.config.content_prettifier.custom_renderers[i];

                        // ID field.
                        ui.horizontal(|ui| {
                            ui.label("ID:");
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut cr.id)
                                        .desired_width(150.0)
                                        .hint_text("unique_id"),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });

                        // Name field.
                        ui.horizontal(|ui| {
                            ui.label("Name:");
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut cr.name)
                                        .desired_width(200.0)
                                        .hint_text("Display Name"),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });

                        // Priority.
                        ui.horizontal(|ui| {
                            ui.label("Priority:");
                            if ui
                                .add(
                                    egui::DragValue::new(&mut cr.priority)
                                        .range(1..=100)
                                        .speed(1.0),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });

                        // Render command.
                        ui.horizontal(|ui| {
                            ui.label("Command:");
                            let mut cmd_text = cr.render_command.clone().unwrap_or_default();
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut cmd_text)
                                        .desired_width(250.0)
                                        .font(egui::TextStyle::Monospace)
                                        .hint_text("e.g. bat --color=always"),
                                )
                                .changed()
                            {
                                cr.render_command = if cmd_text.is_empty() {
                                    None
                                } else {
                                    Some(cmd_text)
                                };
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });

                        // Render args.
                        ui.label("Arguments:");
                        let mut remove_arg: Option<usize> = None;
                        for (j, arg) in cr.render_args.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(arg)
                                            .desired_width(200.0)
                                            .font(egui::TextStyle::Monospace),
                                    )
                                    .changed()
                                {
                                    settings.has_changes = true;
                                    *changes_this_frame = true;
                                }
                                if ui.small_button("-").clicked() {
                                    remove_arg = Some(j);
                                }
                            });
                        }
                        if let Some(j) = remove_arg {
                            cr.render_args.remove(j);
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            if ui.small_button("+ Add Argument").clicked() {
                                cr.render_args.push(String::new());
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });

                        // Detection patterns.
                        ui.add_space(4.0);
                        ui.label("Detection patterns (regex):");
                        let mut remove_pat: Option<usize> = None;
                        for (j, pat) in cr.detect_patterns.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                ui.label(egui::RichText::new("/").monospace().weak());
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(pat)
                                            .desired_width(200.0)
                                            .font(egui::TextStyle::Monospace),
                                    )
                                    .changed()
                                {
                                    settings.has_changes = true;
                                    *changes_this_frame = true;
                                }
                                ui.label(egui::RichText::new("/").monospace().weak());
                                if ui.small_button("-").clicked() {
                                    remove_pat = Some(j);
                                }
                            });
                        }
                        if let Some(j) = remove_pat {
                            cr.detect_patterns.remove(j);
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            if ui.small_button("+ Add Pattern").clicked() {
                                cr.detect_patterns.push(String::new());
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        });

                        ui.add_space(4.0);
                        if ui
                            .small_button(
                                egui::RichText::new("Delete Renderer")
                                    .color(egui::Color32::from_rgb(200, 80, 80)),
                            )
                            .clicked()
                        {
                            delete_index = Some(i);
                        }
                    });
            }

            if let Some(i) = delete_index {
                settings
                    .config
                    .content_prettifier
                    .custom_renderers
                    .remove(i);
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(4.0);

            if ui
                .button("+ Add Custom Renderer")
                .on_hover_text("Add a user-defined renderer with external command")
                .clicked()
            {
                settings.config.content_prettifier.custom_renderers.push(
                    par_term_config::config::prettifier::CustomRendererConfig {
                        id: format!(
                            "custom_{}",
                            settings.config.content_prettifier.custom_renderers.len()
                        ),
                        name: "New Renderer".to_string(),
                        detect_patterns: vec![],
                        render_command: None,
                        render_args: vec![],
                        priority: 50,
                    },
                );
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        },
    );
}

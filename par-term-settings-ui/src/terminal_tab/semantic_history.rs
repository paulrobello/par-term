//! Semantic history section for the terminal settings tab.
//!
//! Covers: link handler, file path detection, link highlight color/underline,
//! editor mode, custom editor command.

use crate::SettingsUI;
use crate::section::{INPUT_WIDTH, collapsing_section};
use std::collections::HashSet;

pub(super) fn show_semantic_history_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Semantic History",
        "terminal_semantic_history",
        true,
        collapsed,
        |ui| {
            ui.label(
                egui::RichText::new(
                    "Click file paths in terminal output to open them in your editor.",
                )
                .weak(),
            );

            // Link handler command
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Link handler:");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut settings.config.link_handler_command)
                            .desired_width(INPUT_WIDTH)
                            .hint_text("System default"),
                    )
                    .on_hover_text(
                        "Custom command to open URLs.\n\n\
                     Use {url} as placeholder for the URL.\n\n\
                     Examples:\n\
                     • firefox {url}\n\
                     • open -a Safari {url} (macOS)\n\
                     • chromium-browser {url} (Linux)\n\n\
                     Leave empty to use system default browser.",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            if !settings.config.link_handler_command.is_empty()
                && !settings.config.link_handler_command.contains("{url}")
            {
                ui.label(
                    egui::RichText::new("⚠ Command should contain {url} placeholder")
                        .small()
                        .color(egui::Color32::from_rgb(255, 193, 7)),
                );
            }

            ui.add_space(8.0);
            ui.separator();

            ui.add_space(4.0);

            if ui
                .checkbox(
                    &mut settings.config.semantic_history_enabled,
                    "Enable file path detection",
                )
                .on_hover_text(
                    "Detect file paths in terminal output.\n\
                 Cmd+Click (macOS) or Ctrl+Click (Windows/Linux) to open.",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Link highlight color:");
                let mut color = settings.config.link_highlight_color;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.link_highlight_color = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            if ui
                .checkbox(
                    &mut settings.config.link_highlight_underline,
                    "Underline highlighted links",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if settings.config.link_highlight_underline {
                ui.horizontal(|ui| {
                    ui.label("Underline style:");
                    egui::ComboBox::from_id_salt("link_underline_style")
                        .selected_text(settings.config.link_underline_style.display_name())
                        .show_ui(ui, |ui| {
                            for style in par_term_config::LinkUnderlineStyle::all() {
                                if ui
                                    .selectable_value(
                                        &mut settings.config.link_underline_style,
                                        *style,
                                        style.display_name(),
                                    )
                                    .changed()
                                {
                                    settings.has_changes = true;
                                    *changes_this_frame = true;
                                }
                            }
                        });
                });
            }

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Editor mode:");
                egui::ComboBox::from_id_salt("semantic_history_editor_mode")
                    .selected_text(settings.config.semantic_history_editor_mode.display_name())
                    .show_ui(ui, |ui| {
                        for mode in par_term_config::SemanticHistoryEditorMode::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.semantic_history_editor_mode,
                                    *mode,
                                    mode.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            // Show description based on selected mode
            let mode_description = match settings.config.semantic_history_editor_mode {
                par_term_config::SemanticHistoryEditorMode::Custom => {
                    "Use the custom editor command configured below"
                }
                par_term_config::SemanticHistoryEditorMode::EnvironmentVariable => {
                    "Use the $EDITOR environment variable"
                }
                par_term_config::SemanticHistoryEditorMode::SystemDefault => {
                    "Use the system default application for each file type"
                }
            };
            ui.label(egui::RichText::new(mode_description).small().weak());

            // Only show custom editor command when mode is Custom
            if settings.config.semantic_history_editor_mode
                == par_term_config::SemanticHistoryEditorMode::Custom
            {
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("Editor command:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(
                                &mut settings.config.semantic_history_editor,
                            )
                            .desired_width(INPUT_WIDTH),
                        )
                        .on_hover_text(
                            "Command to open files.\n\n\
                         Placeholders:\n\
                         • {file} - file path\n\
                         • {line} - line number (if available)\n\
                         • {col} - column number (if available)\n\n\
                         Examples:\n\
                         • code -g {file}:{line} (VS Code)\n\
                         • subl {file}:{line} (Sublime Text)\n\
                         • vim +{line} {file} (Vim)",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });

                if settings.config.semantic_history_editor.is_empty() {
                    ui.label(
                        egui::RichText::new(
                            "Note: When custom command is empty, falls back to system default",
                        )
                        .small()
                        .weak(),
                    );
                }
            }
        },
    );
}

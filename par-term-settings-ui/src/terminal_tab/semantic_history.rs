//! Semantic history section for the terminal settings tab.
//!
//! Covers: link handler, file path detection, link highlight color/underline,
//! editor mode, custom editor command.

use crate::SettingsUI;
use crate::search::SearchTag;
use crate::section::{INPUT_WIDTH, keyword_section};
use std::collections::HashSet;

pub(super) fn show_semantic_history_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "Semantic History",
        "terminal_semantic_history",
        &[
            "vs code",
            "sublime",
            "vim",
            "editor command",
            "url color",
            "file url",
            "file scheme",
        ],
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
                        egui::TextEdit::singleline(
                            &mut settings.config.semantic_history.link_handler_command,
                        )
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

            if !settings
                .config
                .semantic_history
                .link_handler_command
                .is_empty()
                && !settings
                    .config
                    .semantic_history
                    .link_handler_command
                    .contains("{url}")
            {
                ui.label(
                    egui::RichText::new("⚠ Command should contain {url} placeholder")
                        .small()
                        .color(egui::Color32::from_rgb(255, 193, 7)),
                );
            }

            // file:// link policy (SEC-009)
            if ui
                .checkbox(
                    &mut settings.config.semantic_history.allow_file_scheme_urls,
                    "Allow opening file:// links",
                )
                .search_tag(&["allow_file_scheme_urls"])
                .on_hover_text(format!(
                    "Let {}-click open `file://` hyperlinks via the OS \
                     default handler (e.g. browser for .html, the file manager for folders).\n\n\
                     Off by default for security: a remote program can emit a `file://` OSC 8 \
                     hyperlink to open an arbitrary local path. Enable only if you trust the \
                     programs you run.",
                    crate::live_binding::primary_modifier_name()
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(8.0);
            ui.separator();

            ui.add_space(4.0);

            if ui
                .checkbox(
                    &mut settings.config.semantic_history.semantic_history_enabled,
                    "Enable file path detection",
                )
                .search_tag(&["semantic_history_enabled"])
                .on_hover_text(format!(
                    "Detect file paths in terminal output. {}-click a path to open it.",
                    crate::live_binding::primary_modifier_name()
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Link highlight color:");
                let mut color = settings.config.semantic_history.link_highlight_color;
                let color_enabled = settings
                    .config
                    .semantic_history
                    .link_highlight_color_enabled;
                ui.add_enabled_ui(color_enabled, |ui| {
                    if ui.color_edit_button_srgb(&mut color).changed() {
                        settings.config.semantic_history.link_highlight_color = color;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            });

            if ui
                .checkbox(
                    &mut settings.config.semantic_history.link_highlight_color_enabled,
                    "Change text color on hover",
                ).search_tag(&["link_highlight_color_enabled"])
                .on_hover_text("Apply the highlight color to link text. Disable to underline only without changing the text color.")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.semantic_history.link_highlight_underline,
                    "Underline highlighted links",
                )
                .search_tag(&["link_highlight_underline"])
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            crate::dependent::dependent(
                ui,
                settings.config.semantic_history.link_highlight_underline,
                "Underline highlighted links",
                |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Underline style:");
                        egui::ComboBox::from_id_salt("link_underline_style")
                            .selected_text(
                                settings
                                    .config
                                    .semantic_history
                                    .link_underline_style
                                    .display_name(),
                            )
                            .show_ui(ui, |ui| {
                                for style in par_term_config::LinkUnderlineStyle::all() {
                                    if ui
                                        .selectable_value(
                                            &mut settings
                                                .config
                                                .semantic_history
                                                .link_underline_style,
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
                },
            );

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Editor mode:");
                egui::ComboBox::from_id_salt("semantic_history_editor_mode")
                    .selected_text(
                        settings
                            .config
                            .semantic_history
                            .semantic_history_editor_mode
                            .display_name(),
                    )
                    .show_ui(ui, |ui| {
                        for mode in par_term_config::SemanticHistoryEditorMode::all() {
                            if ui
                                .selectable_value(
                                    &mut settings
                                        .config
                                        .semantic_history
                                        .semantic_history_editor_mode,
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
            let mode_description = match settings
                .config
                .semantic_history
                .semantic_history_editor_mode
            {
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
            if settings
                .config
                .semantic_history
                .semantic_history_editor_mode
                == par_term_config::SemanticHistoryEditorMode::Custom
            {
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("Editor command:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(
                                &mut settings.config.semantic_history.semantic_history_editor,
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

                if settings
                    .config
                    .semantic_history
                    .semantic_history_editor
                    .is_empty()
                {
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

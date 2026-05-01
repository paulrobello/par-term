//! Search, command history, and command separator sections for the terminal settings tab.
//!
//! Covers: search highlight colors, default options, command history, command separators.

use crate::SettingsUI;
use crate::section::{SLIDER_WIDTH, collapsing_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

// ============================================================================
// Search Section
// ============================================================================

pub(super) fn show_search_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Search", "terminal_search", true, collapsed, |ui| {
        ui.label(egui::RichText::new("Highlight Colors").strong());

        // Match highlight color
        ui.horizontal(|ui| {
            ui.label("Match highlight:");
            let mut color = egui::Color32::from_rgba_unmultiplied(
                settings.config.search.search_highlight_color[0],
                settings.config.search.search_highlight_color[1],
                settings.config.search.search_highlight_color[2],
                settings.config.search.search_highlight_color[3],
            );
            if ui.color_edit_button_srgba(&mut color).changed() {
                settings.config.search.search_highlight_color =
                    [color.r(), color.g(), color.b(), color.a()];
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // Current match highlight color
        ui.horizontal(|ui| {
            ui.label("Current match:");
            let mut color = egui::Color32::from_rgba_unmultiplied(
                settings.config.search.search_current_highlight_color[0],
                settings.config.search.search_current_highlight_color[1],
                settings.config.search.search_current_highlight_color[2],
                settings.config.search.search_current_highlight_color[3],
            );
            if ui.color_edit_button_srgba(&mut color).changed() {
                settings.config.search.search_current_highlight_color =
                    [color.r(), color.g(), color.b(), color.a()];
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Default Options").strong());

        // Case sensitivity default
        if ui
            .checkbox(
                &mut settings.config.search.search_case_sensitive,
                "Case sensitive by default",
            )
            .on_hover_text("When enabled, search will be case-sensitive by default")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        // Regex default
        if ui
            .checkbox(
                &mut settings.config.search.search_regex,
                "Use regex by default",
            )
            .on_hover_text(
                "When enabled, search patterns will be treated as regular expressions by default",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        // Wrap around
        if ui
            .checkbox(
                &mut settings.config.search.search_wrap_around,
                "Wrap around when navigating",
            )
            .on_hover_text("When enabled, navigating past the last match wraps to the first match")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Keyboard Shortcuts").weak().small());
        ui.label(
            egui::RichText::new("  Cmd/Ctrl+F: Open search, Enter: Next, Shift+Enter: Previous")
                .weak()
                .small(),
        );
    });
}

// ============================================================================
// Command History Section
// ============================================================================

pub(super) fn show_command_history_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Command History",
        "terminal_command_history",
        true,
        collapsed,
        |ui| {
            ui.label("Fuzzy search through previously executed commands (Cmd+R / Ctrl+Alt+R).");
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("Max history entries:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.command_history_max_entries,
                            100..=10000,
                        ),
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

// ============================================================================
// Command Separator Section
// ============================================================================

pub(super) fn show_command_separator_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Command Separators",
        "terminal_command_separator",
        false,
        collapsed,
        |ui| {
            if ui
                .checkbox(
                    &mut settings.config.command_separator_enabled,
                    "Show separator lines between commands (requires shell integration)",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_enabled_ui(settings.config.command_separator_enabled, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Thickness (px):");
                    if ui
                        .add_sized(
                            [SLIDER_WIDTH, SLIDER_HEIGHT],
                            egui::Slider::new(
                                &mut settings.config.command_separator_thickness,
                                0.5..=5.0,
                            ),
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Opacity:");
                    if ui
                        .add_sized(
                            [SLIDER_WIDTH, SLIDER_HEIGHT],
                            egui::Slider::new(
                                &mut settings.config.command_separator_opacity,
                                0.0..=1.0,
                            ),
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });

                if ui
                    .checkbox(
                        &mut settings.config.command_separator_exit_color,
                        "Color by exit code (green=success, red=failure)",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                // Custom color picker (only when exit-code coloring is off)
                ui.add_enabled_ui(!settings.config.command_separator_exit_color, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Custom color:");
                        let mut color = egui::Color32::from_rgb(
                            settings.config.command_separator_color[0],
                            settings.config.command_separator_color[1],
                            settings.config.command_separator_color[2],
                        );
                        if egui::color_picker::color_edit_button_srgba(
                            ui,
                            &mut color,
                            egui::color_picker::Alpha::Opaque,
                        )
                        .changed()
                        {
                            settings.config.command_separator_color =
                                [color.r(), color.g(), color.b()];
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });
                });
            });
        },
    );
}

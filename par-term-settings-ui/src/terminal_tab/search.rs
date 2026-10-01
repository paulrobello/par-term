//! Search, command history, and command separator sections for the terminal settings tab.
//!
//! Covers: search highlight colors, default options, command history, command separators.

use crate::SettingsUI;
use crate::search::SearchTag;
use crate::section::{SLIDER_WIDTH, collapsing_section, keyword_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

// ============================================================================
// Search Section
// ============================================================================

pub(crate) fn show_search_section(
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
            if crate::color_helpers::rgba_color_button(
                ui,
                &mut settings.config.search.search_highlight_color,
            )
            .search_tag(&["search_highlight_color"])
            .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // Current match highlight color
        ui.horizontal(|ui| {
            ui.label("Current match:");
            if crate::color_helpers::rgba_color_button(
                ui,
                &mut settings.config.search.search_current_highlight_color,
            )
            .search_tag(&["search_current_highlight_color"])
            .changed()
            {
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
            .search_tag(&["search_case_sensitive"])
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
            .search_tag(&["search_regex"])
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
            .search_tag(&["search_wrap_around"])
            .on_hover_text("When enabled, navigating past the last match wraps to the first match")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Keyboard Shortcuts").weak().small());
        let open = crate::live_binding::binding_for(&settings.config, "toggle_search")
            .map(|chord| format!("{chord}: Open search, "))
            .unwrap_or_default();
        ui.label(
            egui::RichText::new(format!(
                "  {open}Enter: Next match, Shift+Enter: Previous match"
            ))
            .weak()
            .small(),
        );
    });
}

// ============================================================================
// Command History Section
// ============================================================================

pub(crate) fn show_command_history_section(
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
            ui.label(crate::live_binding::with_binding(
                &settings.config,
                "Fuzzy search through previously run commands",
                "toggle_command_history",
            ));
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("Max history entries:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.command_history_max_entries,
                            100..=10000,
                        )
                        .suffix(" entries"),
                    )
                    .search_tag(&["command_history_max_entries"])
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                    &mut c.command_history_max_entries
                });
            });
        },
    );
}

// ============================================================================
// Command Separator Section
// ============================================================================

pub(crate) fn show_command_separator_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "Command Separators",
        "terminal_command_separator",
        &["divider"],
        false,
        collapsed,
        |ui| {
            if ui
                .checkbox(
                    &mut settings.config.command_separator.command_separator_enabled,
                    "Show separator lines between commands (requires shell integration)",
                )
                .search_tag(&["command_separator_enabled"])
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_enabled_ui(
                settings.config.command_separator.command_separator_enabled,
                |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Thickness:");
                        if ui
                            .add_sized(
                                [SLIDER_WIDTH, SLIDER_HEIGHT],
                                egui::Slider::new(
                                    &mut settings
                                        .config
                                        .command_separator
                                        .command_separator_thickness,
                                    0.5..=5.0,
                                )
                                .suffix(" px"),
                            )
                            .search_tag(&["command_separator_thickness"])
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                            &mut c.command_separator.command_separator_thickness
                        });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Opacity:");
                        if ui
                            .add_sized(
                                [SLIDER_WIDTH, SLIDER_HEIGHT],
                                crate::units::percent(egui::Slider::new(
                                    &mut settings
                                        .config
                                        .command_separator
                                        .command_separator_opacity,
                                    0.0..=1.0,
                                )),
                            )
                            .search_tag(&["command_separator_opacity"])
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                            &mut c.command_separator.command_separator_opacity
                        });
                    });

                    if ui
                        .checkbox(
                            &mut settings
                                .config
                                .command_separator
                                .command_separator_exit_color,
                            "Color by exit code (green=success, red=failure)",
                        )
                        .search_tag(&["command_separator_exit_color"])
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }

                    // Custom color picker (only when exit-code coloring is off)
                    ui.add_enabled_ui(
                        !settings
                            .config
                            .command_separator
                            .command_separator_exit_color,
                        |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Custom color:");
                                if crate::color_helpers::rgb_color_button(
                                    ui,
                                    &mut settings.config.command_separator.command_separator_color,
                                )
                                .search_tag(&["command_separator_color"])
                                .changed()
                                {
                                    settings.has_changes = true;
                                    *changes_this_frame = true;
                                }
                            });
                        },
                    );
                },
            );
        },
    );
}

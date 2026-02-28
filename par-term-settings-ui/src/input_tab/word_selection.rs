//! Word selection and copy mode settings sections.

use crate::SettingsUI;
use crate::section::collapsing_section;
use std::collections::HashSet;

// ============================================================================
// Word Selection Section
// ============================================================================

pub(super) fn show_word_selection_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Word Selection",
        "input_word_selection",
        false,
        collapsed,
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Word characters:");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut settings.config.word_characters)
                            .hint_text("/-+\\~_.")
                            .desired_width(150.0),
                    )
                    .on_hover_text(
                        "Characters considered part of a word (in addition to alphanumeric)",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            if ui
                .checkbox(
                    &mut settings.config.smart_selection_enabled,
                    "Enable smart selection",
                )
                .on_hover_text("Double-click will try to match patterns like URLs, emails, paths")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if settings.config.smart_selection_enabled {
                ui.separator();
                ui.label("Smart Selection Rules");
                ui.label(
                    egui::RichText::new("Higher precision rules are checked first")
                        .small()
                        .weak(),
                );

                egui::ScrollArea::vertical()
                    .max_height(150.0)
                    .show(ui, |ui| {
                        for rule in &mut settings.config.smart_selection_rules {
                            ui.horizontal(|ui| {
                                if ui.checkbox(&mut rule.enabled, "").changed() {
                                    settings.has_changes = true;
                                    *changes_this_frame = true;
                                }
                                let label = egui::RichText::new(&rule.name);
                                let label = if rule.enabled {
                                    label
                                } else {
                                    label.strikethrough().weak()
                                };
                                ui.label(label).on_hover_ui(|ui| {
                                    ui.label(format!("Pattern: {}", rule.regex));
                                    ui.label(format!("Precision: {:?}", rule.precision));
                                });
                            });
                        }
                    });

                if ui
                    .button("Reset rules to defaults")
                    .on_hover_text("Replace all rules with the default set")
                    .clicked()
                {
                    settings.config.smart_selection_rules =
                        par_term_config::default_smart_selection_rules();
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            }
        },
    );
}

// ============================================================================
// Copy Mode Section
// ============================================================================

pub(super) fn show_copy_mode_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Copy Mode", "input_copy_mode", true, collapsed, |ui| {
        ui.label(
            egui::RichText::new(
                "Vi-style keyboard-driven text selection and navigation. \
                 Activate via the toggle_copy_mode keybinding action.",
            )
            .weak()
            .size(11.0),
        );
        ui.add_space(4.0);

        if ui
            .checkbox(&mut settings.config.copy_mode_enabled, "Enable copy mode")
            .on_hover_text(
                "Allow entering copy mode via the toggle_copy_mode keybinding action. \
                 When disabled, the keybinding action is ignored.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.copy_mode_auto_exit_on_yank,
                "Auto-exit on yank",
            )
            .on_hover_text(
                "Automatically exit copy mode after yanking (copying) selected text. \
                 When disabled, copy mode stays active after pressing y so you can \
                 continue selecting.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.copy_mode_show_status,
                "Show status bar",
            )
            .on_hover_text(
                "Display a status bar at the bottom of the terminal when copy mode is active. \
                 Shows the current mode (COPY/VISUAL/V-LINE/V-BLOCK/SEARCH) and cursor position.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Tip: Add a keybinding with action \"toggle_copy_mode\" to activate. \
                 In copy mode: hjkl to move, v/V/Ctrl+V for visual select, y to yank, \
                 /? to search, Esc/q to exit.",
            )
            .weak()
            .italics()
            .size(10.5),
        );
    });
}

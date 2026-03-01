//! Cursor-related sections of the appearance settings tab.
//!
//! Covers: Cursor, Cursor Locks, and Cursor Effects sections.

use crate::SettingsUI;
use crate::section::{collapsing_section, section_matches, subsection_label};
use par_term_config::{CursorStyle, UnfocusedCursorStyle};
use std::collections::HashSet;

pub(super) fn show_cursor_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    if section_matches(
        &settings.search_query.trim().to_lowercase(),
        "Cursor",
        &[
            "style",
            "block",
            "beam",
            "underline",
            "blink",
            "color",
            "text color",
            "cursor text color",
            "unfocused cursor",
            "hollow",
        ],
    ) {
        collapsing_section(ui, "Cursor", "appearance_cursor", true, collapsed, |ui| {
            ui.horizontal(|ui| {
                ui.label("Style:");
                let current = match settings.config.cursor_style {
                    CursorStyle::Block => 0,
                    CursorStyle::Beam => 1,
                    CursorStyle::Underline => 2,
                };
                let mut selected = current;
                egui::ComboBox::from_id_salt("appearance_cursor_style")
                    .selected_text(match current {
                        0 => "Block",
                        1 => "Beam",
                        2 => "Underline",
                        _ => "Unknown",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut selected, 0, "Block");
                        ui.selectable_value(&mut selected, 1, "Beam");
                        ui.selectable_value(&mut selected, 2, "Underline");
                    });
                if selected != current {
                    settings.config.cursor_style = match selected {
                        0 => CursorStyle::Block,
                        1 => CursorStyle::Beam,
                        2 => CursorStyle::Underline,
                        _ => CursorStyle::Block,
                    };
                    settings.has_changes = true;
                }
            });

            if ui
                .checkbox(&mut settings.config.cursor_blink, "Cursor blink")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.horizontal(|ui| {
                ui.label("Blink interval (ms):");
                if ui
                    .add(egui::Slider::new(
                        &mut settings.config.cursor_blink_interval,
                        100..=2000,
                    ))
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Color:");
                let mut color = settings.config.cursor_color;
                if ui.color_edit_button_srgb(&mut color).changed() {
                    settings.config.cursor_color = color;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Text color (block cursor):");
                let mut use_custom_color = settings.config.cursor_text_color.is_some();
                if ui
                    .checkbox(&mut use_custom_color, "")
                    .on_hover_text("Enable custom text color under block cursor")
                    .changed()
                {
                    if use_custom_color {
                        settings.config.cursor_text_color = Some([0, 0, 0]);
                    } else {
                        settings.config.cursor_text_color = None;
                    }
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if let Some(ref mut text_color) = settings.config.cursor_text_color {
                    let mut color = *text_color;
                    if ui.color_edit_button_srgb(&mut color).changed() {
                        *text_color = color;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                } else {
                    ui.label("(auto)");
                }
            });

            subsection_label(ui, "When Unfocused");

            ui.horizontal(|ui| {
                ui.label("Style:");
                let current = match settings.config.unfocused_cursor_style {
                    UnfocusedCursorStyle::Hollow => 0,
                    UnfocusedCursorStyle::Same => 1,
                    UnfocusedCursorStyle::Hidden => 2,
                };
                let mut selected = current;
                egui::ComboBox::from_id_salt("appearance_unfocused_cursor_style")
                    .selected_text(match current {
                        0 => "Hollow (outline)",
                        1 => "Same",
                        2 => "Hidden",
                        _ => "Unknown",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut selected, 0, "Hollow (outline)");
                        ui.selectable_value(&mut selected, 1, "Same");
                        ui.selectable_value(&mut selected, 2, "Hidden");
                    });
                if selected != current {
                    settings.config.unfocused_cursor_style = match selected {
                        0 => UnfocusedCursorStyle::Hollow,
                        1 => UnfocusedCursorStyle::Same,
                        2 => UnfocusedCursorStyle::Hidden,
                        _ => UnfocusedCursorStyle::Hollow,
                    };
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        });
    }
}

pub(super) fn show_cursor_locks_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    if section_matches(
        &settings.search_query.trim().to_lowercase(),
        "Cursor Locks",
        &[
            "lock",
            "visibility",
            "style",
            "blink",
            "prevent applications",
        ],
    ) {
        collapsing_section(
            ui,
            "Cursor Locks",
            "appearance_cursor_locks",
            false,
            collapsed,
            |ui| {
                ui.label("Prevent applications from changing cursor settings:");
                ui.add_space(4.0);

                if ui
                    .checkbox(
                        &mut settings.config.lock_cursor_visibility,
                        "Lock cursor visibility",
                    )
                    .on_hover_text("Prevent applications from hiding the cursor")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if ui
                    .checkbox(&mut settings.config.lock_cursor_style, "Lock cursor style")
                    .on_hover_text("Prevent applications from changing cursor style")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                ui.add_enabled_ui(!settings.config.lock_cursor_style, |ui| {
                    if ui
                        .checkbox(&mut settings.config.lock_cursor_blink, "Lock cursor blink")
                        .on_hover_text(if settings.config.lock_cursor_style {
                            "Disabled: Lock cursor style already controls blink"
                        } else {
                            "Prevent applications from enabling cursor blink"
                        })
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            },
        );
    }
}

pub(super) fn show_cursor_effects_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    if section_matches(
        &settings.search_query.trim().to_lowercase(),
        "Cursor Effects",
        &[
            "guide",
            "shadow",
            "boost",
            "glow",
            "horizontal line",
            "drop shadow",
            "shadow blur",
            "cursor row",
        ],
    ) {
        collapsing_section(
            ui,
            "Cursor Effects",
            "appearance_cursor_effects",
            false,
            collapsed,
            |ui| {
                // Cursor Guide
                if ui
                    .checkbox(
                        &mut settings.config.cursor_guide_enabled,
                        "Cursor guide (horizontal line)",
                    )
                    .on_hover_text("Show a subtle horizontal line at the cursor row")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if settings.config.cursor_guide_enabled {
                    ui.horizontal(|ui| {
                        ui.label("Guide color:");
                        let mut color = settings.config.cursor_guide_color;
                        if ui
                            .color_edit_button_srgba_unmultiplied(&mut color)
                            .changed()
                        {
                            settings.config.cursor_guide_color = color;
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });
                }

                ui.add_space(4.0);

                // Cursor Shadow
                if ui
                    .checkbox(&mut settings.config.cursor_shadow_enabled, "Cursor shadow")
                    .on_hover_text("Add a drop shadow behind the cursor")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if settings.config.cursor_shadow_enabled {
                    ui.horizontal(|ui| {
                        ui.label("Shadow color:");
                        let mut color = settings.config.cursor_shadow_color;
                        if ui
                            .color_edit_button_srgba_unmultiplied(&mut color)
                            .changed()
                        {
                            settings.config.cursor_shadow_color = color;
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Shadow offset X:");
                        if ui
                            .add(egui::Slider::new(
                                &mut settings.config.cursor_shadow_offset[0],
                                0.0..=10.0,
                            ))
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Shadow offset Y:");
                        if ui
                            .add(egui::Slider::new(
                                &mut settings.config.cursor_shadow_offset[1],
                                0.0..=10.0,
                            ))
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Shadow blur:");
                        if ui
                            .add(
                                egui::Slider::new(
                                    &mut settings.config.cursor_shadow_blur,
                                    0.0..=20.0,
                                )
                                .suffix(" px"),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });
                }

                ui.add_space(4.0);

                // Cursor Boost (Glow)
                ui.horizontal(|ui| {
                    ui.label("Cursor boost (glow):");
                    if ui
                        .add(egui::Slider::new(
                            &mut settings.config.cursor_boost,
                            0.0..=1.0,
                        ))
                        .on_hover_text("Add a glow effect around the cursor for visibility")
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });

                if settings.config.cursor_boost > 0.0 {
                    ui.horizontal(|ui| {
                        ui.label("Boost color:");
                        let mut color = settings.config.cursor_boost_color;
                        if ui.color_edit_button_srgb(&mut color).changed() {
                            settings.config.cursor_boost_color = color;
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });
                }
            },
        );
    }
}

//! Progress bar settings tab.
//!
//! Contains:
//! - Progress bar enable/disable
//! - Style and position selection
//! - Bar height and opacity
//! - State-specific color settings

use super::SettingsUI;
use super::section::{SLIDER_WIDTH, collapsing_section, section_matches};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

/// Show the progress bar tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    if section_matches(
        &query,
        "General",
        &[
            "enable", "progress", "bar", "style", "position", "osc", "934", "osc 934", "osc 9;4",
        ],
    ) {
        show_general_section(ui, settings, changes_this_frame, collapsed);
    }

    if section_matches(
        &query,
        "Colors",
        &[
            "color",
            "normal",
            "warning",
            "error",
            "indeterminate",
            "progress",
        ],
    ) {
        show_colors_section(ui, settings, changes_this_frame, collapsed);
    }
}


// ============================================================================
// General Section
// ============================================================================

fn show_general_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "General",
        "progress_bar_general",
        true,
        collapsed,
        |ui| {
            if ui
                .checkbox(
                    &mut settings.config.progress_bar_enabled,
                    "Enable progress bar",
                )
                .on_hover_text(
                    "Display progress bars from OSC 9;4 and OSC 934 escape sequences.\n\
                 Programs can report progress which is shown as a thin bar overlay.",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(8.0);

            // Style selection
            ui.horizontal(|ui| {
                ui.label("Style:");
                egui::ComboBox::from_id_salt("progress_bar_style")
                    .selected_text(settings.config.progress_bar_style.display_name())
                    .show_ui(ui, |ui| {
                        for style in par_term_config::ProgressBarStyle::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.progress_bar_style,
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

            // Position selection
            ui.horizontal(|ui| {
                ui.label("Position:");
                egui::ComboBox::from_id_salt("progress_bar_position")
                    .selected_text(settings.config.progress_bar_position.display_name())
                    .show_ui(ui, |ui| {
                        for position in par_term_config::ProgressBarPosition::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.progress_bar_position,
                                    *position,
                                    position.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            ui.add_space(8.0);

            // Height slider
            ui.horizontal(|ui| {
                ui.label("Height:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(&mut settings.config.progress_bar_height, 2.0..=20.0)
                            .suffix(" px")
                            .show_value(true),
                    )
                    .on_hover_text("Height of the progress bar in pixels")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Opacity slider
            ui.horizontal(|ui| {
                ui.label("Opacity:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(&mut settings.config.progress_bar_opacity, 0.1..=1.0)
                            .show_value(true),
                    )
                    .on_hover_text("Opacity of the progress bar overlay")
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
// Colors Section
// ============================================================================

fn show_colors_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "State Colors",
        "progress_bar_colors",
        true,
        collapsed,
        |ui| {
            ui.label("Colors for different progress bar states:");
            ui.add_space(4.0);

            // Normal color
            ui.horizontal(|ui| {
                ui.label("Normal:");
                let mut color = egui::Color32::from_rgb(
                    settings.config.progress_bar_normal_color[0],
                    settings.config.progress_bar_normal_color[1],
                    settings.config.progress_bar_normal_color[2],
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    settings.config.progress_bar_normal_color = [color.r(), color.g(), color.b()];
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                ui.label(
                    egui::RichText::new("Standard progress")
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });

            // Warning color
            ui.horizontal(|ui| {
                ui.label("Warning:");
                let mut color = egui::Color32::from_rgb(
                    settings.config.progress_bar_warning_color[0],
                    settings.config.progress_bar_warning_color[1],
                    settings.config.progress_bar_warning_color[2],
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    settings.config.progress_bar_warning_color = [color.r(), color.g(), color.b()];
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                ui.label(
                    egui::RichText::new("Operation has warnings")
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });

            // Error color
            ui.horizontal(|ui| {
                ui.label("Error:");
                let mut color = egui::Color32::from_rgb(
                    settings.config.progress_bar_error_color[0],
                    settings.config.progress_bar_error_color[1],
                    settings.config.progress_bar_error_color[2],
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    settings.config.progress_bar_error_color = [color.r(), color.g(), color.b()];
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                ui.label(
                    egui::RichText::new("Operation failed")
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });

            // Indeterminate color
            ui.horizontal(|ui| {
                ui.label("Indeterminate:");
                let mut color = egui::Color32::from_rgb(
                    settings.config.progress_bar_indeterminate_color[0],
                    settings.config.progress_bar_indeterminate_color[1],
                    settings.config.progress_bar_indeterminate_color[2],
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    settings.config.progress_bar_indeterminate_color =
                        [color.r(), color.g(), color.b()];
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                ui.label(
                    egui::RichText::new("Unknown duration (animated)")
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });
        },
    );
}

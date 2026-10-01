//! Progress bar sections (Appearance › Progress Bar). Placement is set in
//! [`crate::layout`].
//!
//! - Progress bar enable/disable
//! - Style and position selection
//! - Bar height and opacity
//! - State-specific color settings

use super::SettingsUI;
use super::section::{SLIDER_WIDTH, keyword_section};
use crate::search::SearchTag;
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

// ============================================================================
// General Section
// ============================================================================

pub(crate) fn show_general_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "General",
        "progress_bar_general",
        &["progress bar"],
        true,
        collapsed,
        |ui| {
            if ui
                .checkbox(
                    &mut settings.config.progress_bar.progress_bar_enabled,
                    "Enable progress bar",
                )
                .search_tag(&["progress_bar_enabled"])
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
                    .selected_text(
                        settings
                            .config
                            .progress_bar
                            .progress_bar_style
                            .display_name(),
                    )
                    .show_ui(ui, |ui| {
                        for style in par_term_config::ProgressBarStyle::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.progress_bar.progress_bar_style,
                                    *style,
                                    style.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    })
                    .response
                    .search_tag(&["progress_bar_style"]);
            });

            // Position selection
            ui.horizontal(|ui| {
                ui.label("Position:");
                egui::ComboBox::from_id_salt("progress_bar_position")
                    .selected_text(
                        settings
                            .config
                            .progress_bar
                            .progress_bar_position
                            .display_name(),
                    )
                    .show_ui(ui, |ui| {
                        for position in par_term_config::ProgressBarPosition::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.progress_bar.progress_bar_position,
                                    *position,
                                    position.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    })
                    .response
                    .search_tag(&["progress_bar_position"]);
            });

            ui.add_space(8.0);

            // Height slider
            ui.horizontal(|ui| {
                ui.label("Height:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.progress_bar.progress_bar_height,
                            2.0..=20.0,
                        )
                        .suffix(" px")
                        .show_value(true),
                    )
                    .search_tag(&["progress_bar_height"])
                    .on_hover_text("Height of the progress bar in pixels")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                    &mut c.progress_bar.progress_bar_height
                });
            });

            // Opacity slider
            ui.horizontal(|ui| {
                ui.label("Opacity:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        crate::units::percent(egui::Slider::new(
                            &mut settings.config.progress_bar.progress_bar_opacity,
                            0.1..=1.0,
                        ))
                        .show_value(true),
                    )
                    .search_tag(&["progress_bar_opacity"])
                    .on_hover_text("Opacity of the progress bar overlay")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                crate::reset::reset_button(ui, settings, changes_this_frame, |c| {
                    &mut c.progress_bar.progress_bar_opacity
                });
            });
        },
    );
}

// ============================================================================
// Colors Section
// ============================================================================

pub(crate) fn show_colors_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "State Colors",
        "progress_bar_colors",
        &[
            "progress bar",
            "normal",
            "warning",
            "error",
            "indeterminate",
        ],
        true,
        collapsed,
        |ui| {
            ui.label("Colors for different progress bar states:");
            ui.add_space(4.0);

            // Normal color
            ui.horizontal(|ui| {
                ui.label("Normal:");
                if crate::color_helpers::rgb_color_button(
                    ui,
                    &mut settings.config.progress_bar.progress_bar_normal_color,
                )
                .search_tag(&["progress_bar_normal_color"])
                .changed()
                {
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
                if crate::color_helpers::rgb_color_button(
                    ui,
                    &mut settings.config.progress_bar.progress_bar_warning_color,
                )
                .search_tag(&["progress_bar_warning_color"])
                .changed()
                {
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
                if crate::color_helpers::rgb_color_button(
                    ui,
                    &mut settings.config.progress_bar.progress_bar_error_color,
                )
                .search_tag(&["progress_bar_error_color"])
                .changed()
                {
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
                if crate::color_helpers::rgb_color_button(
                    ui,
                    &mut settings
                        .config
                        .progress_bar
                        .progress_bar_indeterminate_color,
                )
                .search_tag(&["progress_bar_indeterminate_color"])
                .changed()
                {
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

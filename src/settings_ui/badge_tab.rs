//! Badge settings tab.
//!
//! Contains:
//! - Badge enable/disable
//! - Badge format string with variable interpolation
//! - Badge appearance (color, opacity, font)
//! - Badge positioning (margins, max size)

use super::SettingsUI;
use super::section::{SLIDER_WIDTH, collapsing_section};

const SLIDER_HEIGHT: f32 = 18.0;

/// Show the badge tab content.
pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    let query = settings.search_query.trim().to_lowercase();

    // General section
    if section_matches(&query, "General", &["enable", "badge", "format"]) {
        show_general_section(ui, settings, changes_this_frame);
    }

    // Appearance section
    if section_matches(&query, "Appearance", &["color", "opacity", "font", "bold"]) {
        show_appearance_section(ui, settings, changes_this_frame);
    }

    // Position section
    if section_matches(&query, "Position", &["margin", "size", "width", "height"]) {
        show_position_section(ui, settings, changes_this_frame);
    }

    // Variables section (help/reference)
    if section_matches(
        &query,
        "Variables",
        &["variable", "session", "hostname", "username", "path"],
    ) {
        show_variables_section(ui, settings, changes_this_frame);
    }
}

fn section_matches(query: &str, title: &str, keywords: &[&str]) -> bool {
    if query.is_empty() {
        return true;
    }
    if title.to_lowercase().contains(query) {
        return true;
    }
    keywords.iter().any(|k| k.to_lowercase().contains(query))
}

// ============================================================================
// General Section
// ============================================================================

fn show_general_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "General", "badge_general", true, |ui| {
        if ui
            .checkbox(&mut settings.config.badge_enabled, "Enable badge")
            .on_hover_text("Display a semi-transparent text overlay in the terminal corner")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);
        ui.label("Badge format:");
        ui.add_space(2.0);

        // Multi-line text editor for format string
        if ui
            .add(
                egui::TextEdit::singleline(&mut settings.config.badge_format)
                    .hint_text("\\(session.username)@\\(session.hostname)")
                    .desired_width(ui.available_width() - 20.0),
            )
            .on_hover_text("Format string with variable placeholders like \\(session.hostname)")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("Use \\(session.variable) syntax for dynamic values")
                .small()
                .color(egui::Color32::GRAY),
        );
    });
}

// ============================================================================
// Appearance Section
// ============================================================================

fn show_appearance_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Appearance", "badge_appearance", true, |ui| {
        // Color picker
        ui.horizontal(|ui| {
            ui.label("Text color:");
            let mut color = egui::Color32::from_rgb(
                settings.config.badge_color[0],
                settings.config.badge_color[1],
                settings.config.badge_color[2],
            );
            if ui.color_edit_button_srgba(&mut color).changed() {
                settings.config.badge_color = [color.r(), color.g(), color.b()];
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
                    egui::Slider::new(&mut settings.config.badge_color_alpha, 0.0..=1.0)
                        .show_value(true),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);

        // Font family
        ui.horizontal(|ui| {
            ui.label("Font:");
            if ui
                .add(
                    egui::TextEdit::singleline(&mut settings.config.badge_font)
                        .hint_text("Helvetica")
                        .desired_width(150.0),
                )
                .on_hover_text("Font family for badge text (uses system font if not found)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // Bold checkbox
        if ui
            .checkbox(&mut settings.config.badge_font_bold, "Bold")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

// ============================================================================
// Position Section
// ============================================================================

fn show_position_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Position & Size", "badge_position", false, |ui| {
        ui.label("Margins (pixels):");

        ui.horizontal(|ui| {
            ui.label("Top:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.badge_top_margin, 0.0..=100.0),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Right:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.badge_right_margin, 0.0..=100.0),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);
        ui.label("Maximum size (fraction of terminal):");

        ui.horizontal(|ui| {
            ui.label("Max width:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.badge_max_width, 0.1..=1.0)
                        .show_value(true),
                )
                .on_hover_text("Maximum badge width as fraction of terminal width")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Max height:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.badge_max_height, 0.05..=0.5)
                        .show_value(true),
                )
                .on_hover_text("Maximum badge height as fraction of terminal height")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}

// ============================================================================
// Variables Section (Reference)
// ============================================================================

fn show_variables_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Available Variables", "badge_variables", false, |ui| {
        ui.label("Click a variable to append it to the format string:");
        ui.add_space(4.0);

        let variables = [
            ("\\(session.hostname)", "Remote or local hostname"),
            ("\\(session.username)", "Current username"),
            ("\\(session.path)", "Current working directory"),
            ("\\(session.job)", "Foreground job name"),
            ("\\(session.last_command)", "Last executed command"),
            ("\\(session.profile_name)", "Current profile name"),
            ("\\(session.tty)", "TTY device name"),
            ("\\(session.columns)", "Terminal columns"),
            ("\\(session.rows)", "Terminal rows"),
            ("\\(session.bell_count)", "Number of bells received"),
            ("\\(session.selection)", "Currently selected text"),
            ("\\(session.tmux_pane_title)", "tmux pane title"),
        ];

        // Collect clicked variable to avoid borrow issues
        let mut clicked_var: Option<&str> = None;

        egui::Grid::new("badge_variables_grid")
            .num_columns(2)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                for (var, desc) in variables {
                    // Make variable a clickable link
                    let response = ui.add(
                        egui::Label::new(
                            egui::RichText::new(var)
                                .monospace()
                                .color(egui::Color32::from_rgb(100, 150, 255)),
                        )
                        .sense(egui::Sense::click()),
                    );

                    if response.clicked() {
                        clicked_var = Some(var);
                    }

                    // Show pointer cursor and tooltip on hover
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    response.on_hover_text("Click to append to format");

                    ui.label(desc);
                    ui.end_row();
                }
            });

        // Handle click outside the grid to avoid borrow conflict
        if let Some(var) = clicked_var {
            settings.config.badge_format.push_str(var);
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(
                "Tip: Badge format can also be set via OSC 1337;SetBadgeFormat=BASE64",
            )
            .small()
            .color(egui::Color32::GRAY),
        );
    });
}

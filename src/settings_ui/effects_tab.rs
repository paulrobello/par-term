//! Effects settings tab.
//!
//! Consolidates: background_tab (refactored)
//!
//! Contains:
//! - Background mode (default/color/image)
//! - Background image settings
//! - Background shader settings
//! - Shader channel textures
//! - Inline image settings (Sixel, iTerm2, Kitty)
//! - Cursor shader settings

use crate::config::ImageScalingMode;
use std::collections::HashSet;

use super::SettingsUI;

/// Show the effects tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    _collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // Background section
    if section_matches(
        &query,
        "Background",
        &[
            "background",
            "image",
            "color",
            "mode",
            "wallpaper",
            "shader",
            "glsl",
            "fit",
            "fill",
            "stretch",
            "tile",
            "center",
        ],
    ) {
        // Delegate to the existing background_tab implementation
        super::background_tab::show_background(ui, settings, changes_this_frame);
    }

    // Inline Images section (Sixel, iTerm2, Kitty)
    if section_matches(
        &query,
        "Inline Images",
        &[
            "inline",
            "image",
            "sixel",
            "iterm",
            "kitty",
            "scaling",
            "aspect",
            "graphics protocol",
            "nearest neighbor",
            "linear",
        ],
    ) {
        show_inline_images(ui, settings, changes_this_frame);
    }

    // Cursor Shader section
    if section_matches(
        &query,
        "Cursor Shader",
        &[
            "cursor shader",
            "trail",
            "glow",
            "cursor effect",
            "glsl",
            "animation",
        ],
    ) {
        // Delegate to the existing cursor shader implementation
        super::background_tab::show_cursor_shader(ui, settings, changes_this_frame);
    }
}

/// Show inline image settings (Sixel, iTerm2, Kitty protocols).
fn show_inline_images(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    egui::CollapsingHeader::new("Inline Images (Sixel, iTerm2, Kitty)")
        .default_open(true)
        .show(ui, |ui| {
            ui.label("Settings for inline graphics rendered in the terminal.");
            ui.add_space(4.0);

            // Image scaling mode (nearest vs linear)
            ui.horizontal(|ui| {
                ui.label("Scaling quality:");
                let current = settings.config.image_scaling_mode;
                egui::ComboBox::from_id_salt("image_scaling_mode")
                    .selected_text(current.display_name())
                    .show_ui(ui, |ui| {
                        for mode in ImageScalingMode::all() {
                            if ui
                                .selectable_label(current == *mode, mode.display_name())
                                .clicked()
                            {
                                settings.config.image_scaling_mode = *mode;
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            // Preserve aspect ratio
            if ui
                .checkbox(
                    &mut settings.config.image_preserve_aspect_ratio,
                    "Preserve aspect ratio",
                )
                .on_hover_text(
                    "Maintain image proportions when scaling. When disabled, images stretch to fill their cell grid.",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
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

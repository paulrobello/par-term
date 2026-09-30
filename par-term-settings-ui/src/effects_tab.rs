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

use par_term_config::ImageScalingMode;
use std::collections::HashSet;

use super::SettingsUI;
use super::section::keyword_section;
use crate::search::SearchTag;

/// Show the effects tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    super::background_tab::show_background(ui, settings, changes_this_frame, collapsed);
    super::background_tab::show_pane_backgrounds(ui, settings, changes_this_frame, collapsed);
    show_inline_images(ui, settings, changes_this_frame, collapsed);
    super::background_tab::show_cursor_shader(ui, settings, changes_this_frame, collapsed);
}

/// Show inline image settings (Sixel, iTerm2, Kitty protocols).
fn show_inline_images(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "Inline Images (Sixel, iTerm2, Kitty)",
        "inline_images",
        &["graphics protocol", "nearest neighbor"],
        true,
        collapsed,
        |ui| {
            ui.label("Settings for inline graphics rendered in the terminal.");
            ui.add_space(4.0);

            // Image scaling mode (nearest vs linear)
            ui.horizontal(|ui| {
                ui.label("Scaling quality:");
                let current = settings.config.image.image_scaling_mode;
                egui::ComboBox::from_id_salt("image_scaling_mode")
                    .selected_text(current.display_name())
                    .show_ui(ui, |ui| {
                        for mode in ImageScalingMode::all() {
                            if ui
                                .selectable_label(current == *mode, mode.display_name())
                                .clicked()
                            {
                                settings.config.image.image_scaling_mode = *mode;
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            // Preserve aspect ratio
            if ui
                .checkbox(
                    &mut settings.config.image.image_preserve_aspect_ratio,
                    "Preserve aspect ratio",
                ).search_tag(&["image_preserve_aspect_ratio"])
                .on_hover_text(
                    "Maintain image proportions when scaling. When disabled, images stretch to fill their cell grid.",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use crate::sidebar::SettingsTab;

    #[test]
    fn shader_texture_terms_find_the_effects_tab() {
        for keyword in [
            "noise",
            "built-in noise",
            "blend",
            "blend mode",
            "background blend",
        ] {
            assert!(
                crate::search::tab_has_result(SettingsTab::Effects, keyword),
                "Effects should have a result for {keyword:?}"
            );
        }
    }
}

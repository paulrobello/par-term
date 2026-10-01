//! Inline images (Effects & Shaders › Inline Images). Background and shader
//! sections are in `background_tab`; placement is set in [`crate::layout`].

use par_term_config::ImageScalingMode;
use std::collections::HashSet;

use super::SettingsUI;
use super::section::keyword_section;
use crate::search::SearchTag;

/// Show inline image settings (Sixel, iTerm2, Kitty protocols).
pub(crate) fn show_inline_images(
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
                    })
                    .response
                    .search_tag(&["image_scaling_mode"]);
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
    fn shader_texture_terms_find_the_effects_and_shaders_tab() {
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

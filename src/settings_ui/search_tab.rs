//! Search settings tab for the settings UI.

use super::SettingsUI;
use egui::Color32;

pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Search", |ui| {
        ui.label("Highlight Colors");
        ui.separator();

        // Match highlight color
        ui.horizontal(|ui| {
            ui.label("Match highlight:");
            let mut color = Color32::from_rgba_unmultiplied(
                settings.config.search_highlight_color[0],
                settings.config.search_highlight_color[1],
                settings.config.search_highlight_color[2],
                settings.config.search_highlight_color[3],
            );
            if ui.color_edit_button_srgba(&mut color).changed() {
                settings.config.search_highlight_color =
                    [color.r(), color.g(), color.b(), color.a()];
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // Current match highlight color
        ui.horizontal(|ui| {
            ui.label("Current match:");
            let mut color = Color32::from_rgba_unmultiplied(
                settings.config.search_current_highlight_color[0],
                settings.config.search_current_highlight_color[1],
                settings.config.search_current_highlight_color[2],
                settings.config.search_current_highlight_color[3],
            );
            if ui.color_edit_button_srgba(&mut color).changed() {
                settings.config.search_current_highlight_color =
                    [color.r(), color.g(), color.b(), color.a()];
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);
        ui.label("Default Options");
        ui.separator();

        // Case sensitivity default
        if ui
            .checkbox(
                &mut settings.config.search_case_sensitive,
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
            .checkbox(&mut settings.config.search_regex, "Use regex by default")
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
                &mut settings.config.search_wrap_around,
                "Wrap around when navigating",
            )
            .on_hover_text("When enabled, navigating past the last match wraps to the first match")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Keyboard Shortcuts:").weak().small());
        ui.label(
            egui::RichText::new("  Cmd/Ctrl+F: Open search")
                .weak()
                .small(),
        );
        ui.label(egui::RichText::new("  Enter: Next match").weak().small());
        ui.label(
            egui::RichText::new("  Shift+Enter: Previous match")
                .weak()
                .small(),
        );
        ui.label(egui::RichText::new("  Escape: Close search").weak().small());
    });
}

//! Keyboard input settings tab for the Settings UI.

use crate::config::OptionKeyMode;

use super::SettingsUI;

/// Show the keyboard input settings section
pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Keyboard Input", |ui| {
        ui.label("Option/Alt key behavior for emacs, vim, and other terminal applications.");
        ui.add_space(4.0);

        // Left Option/Alt key mode
        ui.horizontal(|ui| {
            ui.label("Left Option/Alt sends:");
            let current = settings.config.left_option_key_mode;
            egui::ComboBox::from_id_salt("left_option_key_mode")
                .selected_text(option_key_mode_label(current))
                .show_ui(ui, |ui| {
                    for mode in [
                        OptionKeyMode::Esc,
                        OptionKeyMode::Meta,
                        OptionKeyMode::Normal,
                    ] {
                        if ui
                            .selectable_value(
                                &mut settings.config.left_option_key_mode,
                                mode,
                                option_key_mode_label(mode),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        // Description for left Option mode
        ui.indent("left_option_desc", |ui| {
            ui.label(
                egui::RichText::new(option_key_mode_description(
                    settings.config.left_option_key_mode,
                ))
                .weak()
                .small(),
            );
        });

        ui.add_space(8.0);

        // Right Option/Alt key mode
        ui.horizontal(|ui| {
            ui.label("Right Option/Alt sends:");
            let current = settings.config.right_option_key_mode;
            egui::ComboBox::from_id_salt("right_option_key_mode")
                .selected_text(option_key_mode_label(current))
                .show_ui(ui, |ui| {
                    for mode in [
                        OptionKeyMode::Esc,
                        OptionKeyMode::Meta,
                        OptionKeyMode::Normal,
                    ] {
                        if ui
                            .selectable_value(
                                &mut settings.config.right_option_key_mode,
                                mode,
                                option_key_mode_label(mode),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        // Description for right Option mode
        ui.indent("right_option_desc", |ui| {
            ui.label(
                egui::RichText::new(option_key_mode_description(
                    settings.config.right_option_key_mode,
                ))
                .weak()
                .small(),
            );
        });

        ui.add_space(8.0);

        // Hint about common use cases
        ui.separator();
        ui.label(egui::RichText::new("Tips:").strong());
        ui.label("• Use \"Esc\" mode for emacs Meta key (M-x, M-f, M-b, etc.)");
        ui.label("• Use \"Esc\" mode for vim Alt mappings");
        ui.label("• Use \"Normal\" to type special characters (ƒ, ∂, ß, etc.)");
        ui.label("• Configure left/right Option keys differently for flexibility");
    });
}

/// Get the display label for an OptionKeyMode
fn option_key_mode_label(mode: OptionKeyMode) -> &'static str {
    match mode {
        OptionKeyMode::Normal => "Normal",
        OptionKeyMode::Meta => "Meta",
        OptionKeyMode::Esc => "Esc (Recommended)",
    }
}

/// Get the description for an OptionKeyMode
fn option_key_mode_description(mode: OptionKeyMode) -> &'static str {
    match mode {
        OptionKeyMode::Normal => {
            "Sends special characters (e.g., Option+f → ƒ). Default macOS behavior."
        }
        OptionKeyMode::Meta => {
            "Sets high bit on character (e.g., Option+f → 0xE6). Legacy Meta key mode."
        }
        OptionKeyMode::Esc => {
            "Sends Escape prefix (e.g., Option+f → ESC f). Best for emacs/vim compatibility."
        }
    }
}

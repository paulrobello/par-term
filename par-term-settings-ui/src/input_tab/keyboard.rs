//! Keyboard and modifier remapping settings sections.

use crate::SettingsUI;
use crate::section::collapsing_section;
use par_term_config::{ModifierTarget, OptionKeyMode};
use std::collections::HashSet;

// ============================================================================
// Keyboard Section
// ============================================================================

pub(super) fn show_keyboard_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Keyboard", "input_keyboard", true, collapsed, |ui| {
        ui.label("Option/Alt key behavior for emacs, vim, and other terminal applications.");
        ui.add_space(4.0);

        // Left Option/Alt key mode
        ui.horizontal(|ui| {
            ui.label("Left Option/Alt sends:");
            let current = settings.config.left_option_key_mode;
            egui::ComboBox::from_id_salt("input_left_option_key_mode")
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

        ui.indent("input_left_option_desc", |ui| {
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
            egui::ComboBox::from_id_salt("input_right_option_key_mode")
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

        ui.indent("input_right_option_desc", |ui| {
            ui.label(
                egui::RichText::new(option_key_mode_description(
                    settings.config.right_option_key_mode,
                ))
                .weak()
                .small(),
            );
        });

        ui.add_space(8.0);
        ui.separator();

        // Physical key preference
        if ui
            .checkbox(
                &mut settings.config.use_physical_keys,
                "Use physical key positions for keybindings",
            )
            .on_hover_text(
                "Match keybindings by key position (scan code) instead of character produced.\n\
                 This makes shortcuts like Ctrl+Z work consistently across keyboard layouts\n\
                 (QWERTY, AZERTY, Dvorak, etc.).",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);
        ui.separator();
        ui.label(egui::RichText::new("Tips:").strong());
        ui.label("• Use \"Esc\" mode for emacs Meta key (M-x, M-f, M-b, etc.)");
        ui.label("• Use \"Esc\" mode for vim Alt mappings");
        ui.label("• Use \"Normal\" to type special characters (ƒ, ∂, ß, etc.)");
        ui.label("• Enable physical keys if shortcuts feel wrong on non-US layouts");
    });
}

// ============================================================================
// Modifier Remapping Section
// ============================================================================

pub(super) fn show_modifier_remapping_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Modifier Remapping",
        "input_modifier_remapping",
        false,
        collapsed,
        |ui| {
            ui.label("Remap modifier keys to different functions.");
            ui.label(
                egui::RichText::new(
                    "Note: Changes apply to par-term keybindings only, not system-wide.",
                )
                .weak()
                .small(),
            );
            ui.add_space(4.0);

            // Left Ctrl
            ui.horizontal(|ui| {
                ui.label("Left Ctrl acts as:");
                let current = settings.config.modifier_remapping.left_ctrl;
                egui::ComboBox::from_id_salt("input_remap_left_ctrl")
                    .selected_text(current.display_name())
                    .show_ui(ui, |ui| {
                        for target in ModifierTarget::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.modifier_remapping.left_ctrl,
                                    *target,
                                    target.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            // Right Ctrl
            ui.horizontal(|ui| {
                ui.label("Right Ctrl acts as:");
                let current = settings.config.modifier_remapping.right_ctrl;
                egui::ComboBox::from_id_salt("input_remap_right_ctrl")
                    .selected_text(current.display_name())
                    .show_ui(ui, |ui| {
                        for target in ModifierTarget::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.modifier_remapping.right_ctrl,
                                    *target,
                                    target.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            ui.add_space(4.0);

            // Left Alt
            ui.horizontal(|ui| {
                ui.label("Left Alt acts as:");
                let current = settings.config.modifier_remapping.left_alt;
                egui::ComboBox::from_id_salt("input_remap_left_alt")
                    .selected_text(current.display_name())
                    .show_ui(ui, |ui| {
                        for target in ModifierTarget::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.modifier_remapping.left_alt,
                                    *target,
                                    target.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            // Right Alt
            ui.horizontal(|ui| {
                ui.label("Right Alt acts as:");
                let current = settings.config.modifier_remapping.right_alt;
                egui::ComboBox::from_id_salt("input_remap_right_alt")
                    .selected_text(current.display_name())
                    .show_ui(ui, |ui| {
                        for target in ModifierTarget::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.modifier_remapping.right_alt,
                                    *target,
                                    target.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            ui.add_space(4.0);

            #[cfg(target_os = "macos")]
            let super_label = "Cmd";
            #[cfg(not(target_os = "macos"))]
            let super_label = "Super";

            // Left Super/Cmd
            ui.horizontal(|ui| {
                ui.label(format!("Left {} acts as:", super_label));
                let current = settings.config.modifier_remapping.left_super;
                egui::ComboBox::from_id_salt("input_remap_left_super")
                    .selected_text(current.display_name())
                    .show_ui(ui, |ui| {
                        for target in ModifierTarget::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.modifier_remapping.left_super,
                                    *target,
                                    target.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            // Right Super/Cmd
            ui.horizontal(|ui| {
                ui.label(format!("Right {} acts as:", super_label));
                let current = settings.config.modifier_remapping.right_super;
                egui::ComboBox::from_id_salt("input_remap_right_super")
                    .selected_text(current.display_name())
                    .show_ui(ui, |ui| {
                        for target in ModifierTarget::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.modifier_remapping.right_super,
                                    *target,
                                    target.display_name(),
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

            if ui.button("Reset to defaults").clicked() {
                settings.config.modifier_remapping = Default::default();
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        },
    );
}

// ============================================================================
// Helper functions
// ============================================================================

fn option_key_mode_label(mode: OptionKeyMode) -> &'static str {
    match mode {
        OptionKeyMode::Normal => "Normal",
        OptionKeyMode::Meta => "Meta",
        OptionKeyMode::Esc => "Esc (Recommended)",
    }
}

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

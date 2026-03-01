//! Clipboard behavior section for the Content Prettifier tab.

use crate::SettingsUI;
use crate::section::collapsing_section;
use std::collections::HashSet;

pub fn show_clipboard_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Clipboard Behavior",
        "prettifier_clipboard",
        false,
        collapsed,
        |ui| {
            let clip = &mut settings.config.content_prettifier.clipboard;

            ui.horizontal(|ui| {
                ui.label("Default copy:");
                egui::ComboBox::from_id_salt("prettifier_default_copy")
                    .selected_text(clip.default_copy.as_str())
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(clip.default_copy == "rendered", "rendered")
                            .clicked()
                        {
                            clip.default_copy = "rendered".to_string();
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                        if ui
                            .selectable_label(clip.default_copy == "source", "source")
                            .clicked()
                        {
                            clip.default_copy = "source".to_string();
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });
            });
        },
    );
}

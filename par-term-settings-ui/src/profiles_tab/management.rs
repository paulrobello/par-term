//! Profile management section (inline profile list/editor).

use crate::profile_modal_ui::ProfileModalAction;
use crate::section::collapsing_section_with_state;
use crate::settings_ui::SettingsUI;
use std::collections::HashSet;

/// Show the profile management section (inline profile list and editor).
pub(super) fn show_management_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section_with_state(
        ui,
        "Profile Management",
        "profiles_management",
        true,
        collapsed,
        |ui, collapsed| {
            // Render the profile list/edit UI inline
            let action = settings.profile_modal_ui.show_inline(ui, collapsed);

            // Handle returned actions
            match action {
                ProfileModalAction::Save => {
                    settings.profile_save_requested = true;
                }
                ProfileModalAction::OpenProfile(id) => {
                    settings.profile_open_requested = Some(id);
                }
                ProfileModalAction::Cancel | ProfileModalAction::None => {}
            }
        },
    );
}

/// Show the display options section (profile drawer toggle button).
pub(super) fn show_display_options_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    use crate::section::collapsing_section;

    collapsing_section(
        ui,
        "Display Options",
        "profiles_display",
        true,
        collapsed,
        |ui| {
            if ui
            .checkbox(
                &mut settings.config.show_profile_drawer_button,
                "Show profile drawer toggle button",
            )
            .on_hover_text("Show/hide the profile drawer toggle button on the right edge of the terminal window. The drawer can still be accessed via keyboard shortcuts when hidden.")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

            ui.add_space(8.0);

            ui.label(
            egui::RichText::new("The profile drawer provides quick access to your profiles without opening the full settings window.")
                .small()
                .color(egui::Color32::GRAY),
        );
        },
    );
}

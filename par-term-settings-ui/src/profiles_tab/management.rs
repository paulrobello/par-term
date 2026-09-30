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
            settings.profile_modal_ui.global_tmux_enabled = settings.config.tmux.tmux_enabled;
            let action = settings.profile_modal_ui.show_inline(ui, collapsed);

            // Handle returned actions
            match action {
                // Only the empty-list confirmation returns Save now; it
                // runs the Settings window's Save, which persists config and
                // profiles together (UX.md SS5).
                ProfileModalAction::Save => {
                    settings.global_save_requested = true;
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
            // UX.md SC7: the profile drawer button has one control, on
            // Window › Tab Bar. This section points there instead of
            // repeating it.
            ui.horizontal(|ui| {
                let state = if settings.config.tabs.show_profile_drawer_button {
                    "shown"
                } else {
                    "hidden"
                };
                ui.label(format!("Profile drawer button: {state}."));
                if ui
                    .link("Change in Window › Tab Bar")
                    .on_hover_text("show_profile_drawer_button")
                    .clicked()
                {
                    settings.selected_tab = crate::sidebar::SettingsTab::Window;
                }
            });
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "The profile drawer gives quick access to your profiles without opening \
                     Settings.",
                )
                .small()
                .color(egui::Color32::GRAY),
            );
        },
    );
}

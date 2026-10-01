//! Profile management section (inline profile list/editor).

use crate::profile_modal_ui::ProfileModalAction;
use crate::section::{keyword_section, keyword_section_with_state};
use crate::settings_ui::SettingsUI;
use std::collections::HashSet;

/// Show the profile management section (inline profile list and editor).
pub(crate) fn show_management_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    collapsed: &mut HashSet<String>,
) {
    keyword_section_with_state(
        ui,
        "Profile Management",
        "profiles_management",
        &[
            "duplicate",
            "default profile",
            "par-mux",
            "mux",
            "tmux",
            "ssh",
            "shell",
            "tags",
            "login shell",
            "bash",
            "zsh",
            "fish",
            "powershell",
            "inheritance",
            "auto switch",
            "profile shader",
            "shader override",
            "hostname",
            "ssh host",
            "ssh user",
            "ssh port",
            "identity file",
            "tmux session",
            "auto-connect",
            "mux session",
            "session name",
            "set default",
        ],
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
pub(crate) fn show_display_options_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "Display Options",
        "profiles_display",
        &["profile indicator"],
        true,
        collapsed,
        |ui| {
            // UX.md SC7: the profile drawer button has one control, on
            // Windows & Tabs › Tab Bar. This section points there instead of
            // repeating it.
            ui.horizontal(|ui| {
                let state = if settings.config.tabs.show_profile_drawer_button {
                    "shown"
                } else {
                    "hidden"
                };
                ui.label(format!("Profile drawer button: {state}."));
                if ui
                    .link("Change in Windows & Tabs › Tab Bar")
                    .on_hover_text("The show/hide control lives on the Tab Bar page")
                    .clicked()
                {
                    settings.open_section("window_tab_bar");
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

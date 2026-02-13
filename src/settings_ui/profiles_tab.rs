//! Profiles settings tab.
//!
//! Contains:
//! - Inline profile management (create, edit, delete, reorder)
//! - Display options for the profile drawer

use super::SettingsUI;
use super::section::collapsing_section;
use crate::profile_modal_ui::ProfileModalAction;
use std::collections::HashSet;

/// Show the profiles tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // Profile management section (inline)
    if section_matches(
        &query,
        "Profile Management",
        &[
            "profile",
            "manage",
            "create",
            "edit",
            "delete",
            "duplicate",
            "default",
        ],
    ) {
        show_management_section(ui, settings, collapsed);
    }

    // Display options section
    if section_matches(
        &query,
        "Display Options",
        &[
            "drawer",
            "button",
            "toggle",
            "show",
            "hide",
            "profile indicator",
        ],
    ) {
        show_display_options_section(ui, settings, changes_this_frame, collapsed);
    }
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

// ============================================================================
// Profile Management Section (inline)
// ============================================================================

fn show_management_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Profile Management",
        "profiles_management",
        true,
        collapsed,
        |ui| {
            // Render the profile list/edit UI inline
            let action = settings.profile_modal_ui.show_inline(ui);

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

// ============================================================================
// Display Options Section
// ============================================================================

fn show_display_options_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
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

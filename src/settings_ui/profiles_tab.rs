//! Profiles settings tab.
//!
//! Contains:
//! - Overview of profile features
//! - Button to open the profile manager modal
//! - Profile list preview (read-only)

use super::SettingsUI;
use super::section::collapsing_section;

/// Show the profiles tab content.
pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    let query = settings.search_query.trim().to_lowercase();

    // Overview section
    if section_matches(
        &query,
        "Overview",
        &["profile", "manage", "create", "edit", "delete"],
    ) {
        show_overview_section(ui, settings);
    }

    // Features section
    if section_matches(
        &query,
        "Profile Features",
        &[
            "tags",
            "inheritance",
            "shortcut",
            "keyboard",
            "auto switch",
            "hostname",
            "tmux",
            "session",
            "badge",
        ],
    ) {
        show_features_section(ui);
    }

    // Display options section
    if section_matches(
        &query,
        "Display Options",
        &["drawer", "button", "toggle", "show", "hide"],
    ) {
        show_display_options_section(ui, settings, changes_this_frame);
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
// Overview Section
// ============================================================================

fn show_overview_section(ui: &mut egui::Ui, settings: &mut SettingsUI) {
    collapsing_section(ui, "Profile Management", "profiles_overview", true, |ui| {
        ui.label("Profiles allow you to save and quickly switch between different terminal configurations.");
        ui.add_space(8.0);

        ui.label(egui::RichText::new("Each profile can include:").strong());
        ui.label("  • Custom shell command and arguments");
        ui.label("  • Working directory");
        ui.label("  • Environment variables");
        ui.label("  • Visual icon for easy identification");
        ui.label("  • Per-profile badge text override");

        ui.add_space(12.0);

        // Button to open profile manager
        ui.horizontal(|ui| {
            if ui
                .button("Open Profile Manager")
                .on_hover_text(
                    "Open the profile management window to create, edit, and delete profiles",
                )
                .clicked()
            {
                settings.open_profile_manager_requested = true;
            }

            ui.label(
                egui::RichText::new("or use the profile drawer on the right side of the terminal")
                    .small()
                    .color(egui::Color32::GRAY),
            );
        });

        ui.add_space(8.0);

        ui.label(
            egui::RichText::new(
                "Tip: Double-click a profile in the drawer to open it in a new tab",
            )
            .small()
            .color(egui::Color32::GRAY),
        );
    });
}

// ============================================================================
// Features Section
// ============================================================================

fn show_features_section(ui: &mut egui::Ui) {
    collapsing_section(ui, "Profile Features", "profiles_features", true, |ui| {
        ui.label(egui::RichText::new("Tags").strong());
        ui.label("Add searchable tags to organize your profiles. Filter profiles in the drawer by typing tag names.");
        ui.add_space(8.0);

        ui.label(egui::RichText::new("Profile Inheritance").strong());
        ui.label("Create a parent profile with base settings, then override specific values in child profiles. Great for sharing common configurations.");
        ui.add_space(8.0);

        ui.label(egui::RichText::new("Keyboard Shortcuts").strong());
        ui.label("Assign a keyboard shortcut to quickly launch a profile. Use combinations like Ctrl+Shift+1.");
        ui.add_space(8.0);

        ui.label(egui::RichText::new("Automatic Profile Switching").strong());
        ui.label("Configure hostname patterns to automatically switch profiles when connecting to specific servers via SSH. Uses glob patterns like \"*.prod.example.com\".");
        ui.add_space(8.0);

        ui.label(egui::RichText::new("Tmux Session Auto-Switching").strong());
        ui.label("Configure tmux session name patterns to automatically apply profiles when connecting via tmux control mode. Uses glob patterns like \"work-*\" or \"*-production\".");
        ui.add_space(8.0);

        ui.label(egui::RichText::new("Per-Profile Badge").strong());
        ui.label("Override the global badge format for specific profiles. Useful for displaying different info for production vs development servers.");
    });
}

// ============================================================================
// Display Options Section
// ============================================================================

fn show_display_options_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Display Options", "profiles_display", true, |ui| {
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
    });
}

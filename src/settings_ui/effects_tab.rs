//! Effects settings tab.
//!
//! Consolidates: background_tab (refactored)
//!
//! Contains:
//! - Background mode (default/color/image)
//! - Background image settings
//! - Background shader settings
//! - Shader channel textures
//! - Cursor shader settings

use super::SettingsUI;

/// Show the effects tab content.
pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    let query = settings.search_query.trim().to_lowercase();

    // Background section
    if section_matches(
        &query,
        "Background",
        &["background", "image", "color", "mode"],
    ) {
        // Delegate to the existing background_tab implementation
        super::background_tab::show_background(ui, settings, changes_this_frame);
    }

    // Cursor Shader section
    if section_matches(
        &query,
        "Cursor Shader",
        &["cursor shader", "trail", "glow", "cursor effect"],
    ) {
        // Delegate to the existing cursor shader implementation
        super::background_tab::show_cursor_shader(ui, settings, changes_this_frame);
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

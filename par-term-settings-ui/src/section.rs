//! Helper functions for collapsible sections in the settings UI.
//!
//! Provides consistent styling and behavior for settings sections.

use std::collections::HashSet;

/// Standard width for text input controls
pub const INPUT_WIDTH: f32 = 300.0;

/// Standard width for slider controls
pub const SLIDER_WIDTH: f32 = 250.0;

/// Standard slider height
pub const SLIDER_HEIGHT: f32 = 18.0;

/// Standard width for combo boxes
pub const COMBO_WIDTH: f32 = 200.0;

/// A helper for managing collapsible sections with consistent styling.
pub struct CollapsibleSection<'a> {
    title: &'a str,
    id: &'a str,
    default_open: bool,
    collapsed_sections: &'a mut HashSet<String>,
    search_query: &'a str,
    keywords: &'a [&'a str],
}

impl<'a> CollapsibleSection<'a> {
    /// Create a new collapsible section.
    pub fn new(
        title: &'a str,
        id: &'a str,
        collapsed_sections: &'a mut HashSet<String>,
        search_query: &'a str,
    ) -> Self {
        Self {
            title,
            id,
            default_open: true,
            collapsed_sections,
            search_query,
            keywords: &[],
        }
    }

    /// Set whether the section is open by default.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Set keywords for search matching.
    pub fn keywords(mut self, keywords: &'a [&'a str]) -> Self {
        self.keywords = keywords;
        self
    }

    /// Check if this section matches the search query.
    pub fn matches_search(&self) -> bool {
        if self.search_query.is_empty() {
            return true;
        }

        let query = self.search_query.to_lowercase();

        // Check title
        if self.title.to_lowercase().contains(&query) {
            return true;
        }

        // Check keywords
        self.keywords
            .iter()
            .any(|k| k.to_lowercase().contains(&query))
    }

    /// Show the collapsible section.
    ///
    /// Returns `Some(CollapsingResponse)` if the section should be shown, `None` otherwise.
    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> Option<egui::CollapsingResponse<R>> {
        // Skip if search doesn't match
        if !self.matches_search() {
            return None;
        }

        // Determine if section is collapsed
        let is_collapsed = self.collapsed_sections.contains(self.id);
        let should_be_open = if self.search_query.is_empty() {
            !is_collapsed && self.default_open || is_collapsed && !self.default_open
        } else {
            // When searching, always expand matching sections
            true
        };

        // Create the collapsing header
        let header = egui::CollapsingHeader::new(self.title)
            .id_salt(self.id)
            .default_open(should_be_open);

        let response = header.show(ui, add_contents);

        // Track collapsed state
        let section_id = self.id.to_string();
        if response.header_response.clicked() {
            if self.collapsed_sections.contains(&section_id) {
                self.collapsed_sections.remove(&section_id);
            } else {
                self.collapsed_sections.insert(section_id);
            }
        }

        Some(response)
    }
}

/// Helper to show a collapsible section with persistent state tracking.
///
/// The `collapsed_sections` set stores section IDs that have been toggled from
/// their default state. This allows the collapse state to be persisted across
/// settings window open/close cycles and app restarts.
pub fn collapsing_section<R>(
    ui: &mut egui::Ui,
    title: &str,
    id: &str,
    default_open: bool,
    collapsed_sections: &mut HashSet<String>,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::CollapsingResponse<R> {
    // The set stores IDs that have been toggled from their default.
    // XOR logic: toggled + default_open => closed, toggled + !default_open => open
    let is_toggled = collapsed_sections.contains(id);
    let should_be_open = is_toggled != default_open;

    let response = egui::CollapsingHeader::new(title)
        .id_salt(id)
        .default_open(should_be_open)
        .show(ui, add_contents);

    // Track click toggles
    if response.header_response.clicked() {
        let section_id = id.to_string();
        if collapsed_sections.contains(&section_id) {
            collapsed_sections.remove(&section_id);
        } else {
            collapsed_sections.insert(section_id);
        }
    }

    response
}

/// Helper to show a section heading with consistent styling.
pub fn section_heading(ui: &mut egui::Ui, title: &str) {
    ui.add_space(8.0);
    ui.heading(title);
    ui.add_space(4.0);
}

/// Helper to show a sub-section label with consistent styling.
pub fn subsection_label(ui: &mut egui::Ui, title: &str) {
    ui.add_space(8.0);
    ui.label(egui::RichText::new(title).strong());
    ui.add_space(4.0);
}

/// Helper to add spacing after a section.
pub fn section_spacing(ui: &mut egui::Ui) {
    ui.add_space(12.0);
}

/// A helper for indented content blocks.
pub fn indented<R>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    ui.indent(id, add_contents)
}

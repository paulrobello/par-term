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

/// Helper to show a collapsible section with persistent state tracking.
///
/// The `collapsed_sections` set stores section IDs that have been toggled from
/// their default state. This allows the collapse state to be persisted across
/// settings window open/close cycles and app restarts.
///
/// This is also the section's one search declaration (UX.md SQ1): its title
/// and id register it with Settings search, and its controls' captions and
/// tooltips are harvested from what it draws. While a search is active the
/// section is hidden when it has no match and opened when it has one.
/// Returns `None` when search hides it. Use [`keyword_section`] to add terms
/// that appear in no label or tooltip.
pub fn collapsing_section<R>(
    ui: &mut egui::Ui,
    title: &str,
    id: &str,
    default_open: bool,
    collapsed_sections: &mut HashSet<String>,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<egui::CollapsingResponse<R>> {
    section(
        ui,
        title,
        id,
        &[],
        default_open,
        collapsed_sections,
        |ui, _| add_contents(ui),
    )
}

/// Like [`collapsing_section`] but passes `collapsed_sections` into the content
/// closure so nested collapsible sections can also use persistent state tracking.
pub fn collapsing_section_with_state<R>(
    ui: &mut egui::Ui,
    title: &str,
    id: &str,
    default_open: bool,
    collapsed_sections: &mut HashSet<String>,
    add_contents: impl FnOnce(&mut egui::Ui, &mut HashSet<String>) -> R,
) -> Option<egui::CollapsingResponse<R>> {
    section(
        ui,
        title,
        id,
        &[],
        default_open,
        collapsed_sections,
        add_contents,
    )
}

/// Like [`collapsing_section`], with search keywords: terms that appear in no
/// label or tooltip of the section (synonyms, jargon, old names).
pub fn keyword_section<R>(
    ui: &mut egui::Ui,
    title: &str,
    id: &str,
    keywords: &[&str],
    default_open: bool,
    collapsed_sections: &mut HashSet<String>,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<egui::CollapsingResponse<R>> {
    section(
        ui,
        title,
        id,
        keywords,
        default_open,
        collapsed_sections,
        |ui, _| add_contents(ui),
    )
}

/// [`keyword_section`] with the collapse set passed into the body.
pub fn keyword_section_with_state<R>(
    ui: &mut egui::Ui,
    title: &str,
    id: &str,
    keywords: &[&str],
    default_open: bool,
    collapsed_sections: &mut HashSet<String>,
    add_contents: impl FnOnce(&mut egui::Ui, &mut HashSet<String>) -> R,
) -> Option<egui::CollapsingResponse<R>> {
    section(
        ui,
        title,
        id,
        keywords,
        default_open,
        collapsed_sections,
        add_contents,
    )
}

fn section<R>(
    ui: &mut egui::Ui,
    title: &str,
    id: &str,
    keywords: &[&str],
    default_open: bool,
    collapsed_sections: &mut HashSet<String>,
    add_contents: impl FnOnce(&mut egui::Ui, &mut HashSet<String>) -> R,
) -> Option<egui::CollapsingResponse<R>> {
    let view = crate::search::live_view(ui.ctx());
    if view.as_ref().is_some_and(|v| v.hides(id)) {
        return None;
    }
    let searching = view.as_ref().is_some_and(|v| v.active());
    let force_open = crate::search::forces_open(ui.ctx(), view.as_deref(), id);

    // While searching, the header keeps a separate open state (matches
    // start open, SQ4) so the user's saved collapse state is untouched.
    let header = if searching {
        egui::CollapsingHeader::new(title)
            .id_salt((id, "search"))
            .default_open(true)
    } else {
        // The set stores IDs that have been toggled from their default.
        // XOR logic: toggled + default_open => closed, toggled + !default_open => open
        let should_be_open = collapsed_sections.contains(id) != default_open;
        egui::CollapsingHeader::new(title)
            .id_salt(id)
            .default_open(should_be_open)
    }
    .open(force_open.then_some(true));

    let mut body = None;
    let response = header.show(ui, |ui| {
        let r = add_contents(ui, collapsed_sections);
        body = Some((ui.unique_id(), ui.min_rect()));
        r
    });

    if !searching {
        // Toggle on click. A jump opens the section; it stays open after
        // the jump, as if the user had opened it.
        let was_open = collapsed_sections.contains(id) != default_open;
        let toggled = response.header_response.clicked() || (force_open && !was_open);
        if toggled && !collapsed_sections.remove(id) {
            collapsed_sections.insert(id.to_string());
        }
    }

    crate::search::note_section(
        ui,
        crate::search::DrawnSection {
            id,
            title,
            keywords,
            header: response.header_response.clone(),
            body,
            fully_open: response.fully_open(),
        },
    );

    Some(response)
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
    id: impl std::hash::Hash + std::fmt::Debug,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    ui.indent(id, add_contents)
}

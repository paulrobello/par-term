//! Snippets (Automation › Snippets). Placement is set in [`crate::layout`].
//!
//! - `list`: Snippet list rendering grouped by folder, with edit/delete/toggle actions
//! - `editor`: Snippet edit form with variable substitution support
//! - `io`: Import/export functionality (YAML)
//! - `variables_reference`: Built-in variable documentation panel

mod editor;
mod io;
mod list;
mod state;
pub(crate) mod variables_reference;

pub use state::SnippetsTabState;

use super::SettingsUI;
use super::section::keyword_section_with_state;
use std::collections::HashSet;

// ============================================================================
// Snippets Section
// ============================================================================

pub(crate) fn show_snippets_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section_with_state(
        ui,
        "Snippets",
        "snippets_list",
        &[
            "snippets",
            "template",
            "folder",
            "auto-execute",
            "title",
            "content",
            "category",
            "shortcut",
        ],
        true,
        collapsed,
        |ui, collapsed| {
            ui.label("Saved text blocks for quick insertion. Supports variable substitution.");
            ui.add_space(4.0);

            list::render_snippet_list(ui, settings, changes_this_frame, collapsed);

            ui.separator();

            // Add new snippet button or form
            if settings.snippets_tab.adding_new_snippet {
                editor::show_snippet_edit_form(ui, settings, changes_this_frame, None, collapsed);
            } else {
                list::render_add_import_bar(ui, settings, changes_this_frame);
            }
        },
    );
}

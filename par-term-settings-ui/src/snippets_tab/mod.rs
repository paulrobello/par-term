//! Snippets & Actions settings tab.
//!
//! Contains:
//! - [`list`]: Snippet list rendering grouped by folder, with edit/delete/toggle actions
//! - [`editor`]: Snippet edit form with variable substitution support
//! - [`io`]: Import/export functionality (YAML)
//! - [`variables_reference`]: Built-in variable documentation panel
//! - Custom actions (shell commands, text insertion, key sequences) — absorbed from actions_tab

mod editor;
mod io;
mod list;
mod variables_reference;

use super::SettingsUI;
use super::section::{collapsing_section_with_state, section_matches};
use std::collections::HashSet;

/// Show the snippets tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // Snippets section
    if section_matches(
        &query,
        "Snippets",
        &[
            "snippet",
            "text",
            "insert",
            "template",
            "variable",
            "keybinding",
            "folder",
            "shortcut",
            "quick insert",
            "auto-execute",
        ],
    ) {
        show_snippets_section(ui, settings, changes_this_frame, collapsed);
    }

    // Variables reference section (collapsed by default)
    if section_matches(
        &query,
        "Variables Reference",
        &[
            "variable",
            "builtin",
            "built-in",
            "reference",
            "date",
            "time",
            "hostname",
        ],
    ) {
        variables_reference::show_variables_reference_section(ui, settings, collapsed);
    }

    // Actions section (absorbed from actions_tab)
    crate::actions_tab::show(ui, settings, changes_this_frame, collapsed);
}

// ============================================================================
// Snippets Section
// ============================================================================

fn show_snippets_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section_with_state(
        ui,
        "Snippets",
        "snippets_list",
        true,
        collapsed,
        |ui, collapsed| {
            ui.label("Saved text blocks for quick insertion. Supports variable substitution.");
            ui.add_space(4.0);

            list::render_snippet_list(ui, settings, changes_this_frame, collapsed);

            ui.separator();

            // Add new snippet button or form
            if settings.adding_new_snippet {
                editor::show_snippet_edit_form(ui, settings, changes_this_frame, None, collapsed);
            } else {
                list::render_add_import_bar(ui, settings, changes_this_frame);
            }
        },
    );
}

/// Search keywords for the Snippets & Actions settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        "snippet",
        "snippets",
        "text",
        "insert",
        "template",
        "variable",
        "keybinding",
        "folder",
        "substitution",
        "date",
        "time",
        "hostname",
        "path",
        // Snippet management
        "title",
        "name",
        "content",
        "body",
        "description",
        "category",
        "auto-execute",
        "auto execute",
        "record",
        // Import/export
        "export",
        "import",
        "yaml",
        // Actions (absorbed from actions_tab)
        "action",
        "actions",
        "custom action",
        "shell command",
        "text insert",
        "key sequence",
        "macro",
        "automation",
        "shortcut",
        "binding",
        "arguments",
    ]
}

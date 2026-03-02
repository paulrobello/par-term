//! Scripts settings tab.
//!
//! Contains management for external observer scripts that receive terminal events
//! via JSON protocol and can send commands back.
//!
//! ## Sub-module layout
//!
//! | File | Contents |
//! |------|----------|
//! | `mod.rs` (this file) | `show()` dispatcher and `keywords()` |
//! | `list.rs` | Script list section (status, controls, output viewer, panel viewer) |
//! | `editor.rs` | Script edit form (name, path, permissions, save/cancel) |

use super::SettingsUI;
use super::section::section_matches;
use std::collections::HashSet;

mod editor;
mod list;

/// Show the scripts tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    if section_matches(
        &query,
        "Scripts",
        &[
            "script",
            "scripting",
            "observer",
            "event",
            "subprocess",
            "python",
            "panel",
        ],
    ) {
        list::show_scripts_section(ui, settings, changes_this_frame, collapsed);
    }
}

/// Search keywords for the Scripts settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        "script",
        "scripting",
        "python",
        "automation",
        "observer",
        "event",
        "subprocess",
        "external",
        "panel",
        "subscriptions",
        // Script management
        "script path",
        "arguments",
        "args",
        "start",
        "stop",
        "auto-start",
        "auto start",
        "auto-launch",
        "restart",
        "restart policy",
        "restart delay",
        // Permissions
        "permission",
        "allow",
        "write text",
        "run command",
        "change config",
        "rate limit",
    ]
}

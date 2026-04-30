//! Profiles settings tab.
//!
//! Contains:
//! - Inline profile management (create, edit, delete, reorder)
//! - Display options for the profile drawer
//! - Dynamic profile sources management
//!
//! ## Sub-module layout
//!
//! | File | Contents |
//! |------|----------|
//! | `mod.rs` (this file) | `show()` dispatcher and `keywords()` |
//! | `management.rs` | Profile management section + display options section |
//! | `dynamic_sources.rs` | Dynamic profile sources list, enable/disable, edit form |

use super::SettingsUI;
use super::section::section_matches;
use std::collections::HashSet;

mod dynamic_sources;
mod management;

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
        management::show_management_section(ui, settings, collapsed);
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
        management::show_display_options_section(ui, settings, changes_this_frame, collapsed);
    }

    // Dynamic profile sources section
    if section_matches(
        &query,
        "Dynamic Profile Sources",
        &[
            "dynamic", "remote", "url", "fetch", "refresh", "team", "shared", "download", "sync",
        ],
    ) {
        dynamic_sources::show_dynamic_sources_section(ui, settings, changes_this_frame, collapsed);
    }
}

/// Search keywords for the Profiles settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        "profile",
        "profiles",
        "shell",
        "shell selection",
        "login shell",
        "login",
        "bash",
        "zsh",
        "fish",
        "powershell",
        "tags",
        "inheritance",
        "shortcut",
        "auto switch",
        "shader override",
        "profile shader",
        "shader brightness",
        "shader texture set",
        "hostname",
        "ssh",
        "ssh host",
        "ssh user",
        "ssh port",
        "identity file",
        "remote",
        "connection",
        "profile drawer",
        "dynamic",
        "dynamic profiles",
        "remote url",
        "fetch",
        "refresh",
        "team",
        "shared",
        "download",
        "sync",
        // Profile management
        "duplicate",
        "default profile",
        "set default",
        // Dynamic profile extras
        "conflict resolution",
        "http headers",
        "headers",
        "max download",
        "download size",
        "fetch timeout",
        // Tmux auto-connect
        "tmux",
        "tmux session",
        "auto-connect",
    ]
}

//! Terminal settings tab.
//!
//! Consolidates: terminal_tab (original), shell_tab, search_tab, scrollbar_tab
//!
//! Contains:
//! - Behavior settings (scrollback, exit behavior)
//! - Unicode settings (version, ambiguous width, answerback)
//! - Shell settings (custom shell, args, working directory)
//! - Startup settings (initial text)
//! - Search settings (highlight colors, defaults)
//! - Scrollbar settings (width, colors, autohide)
//!
//! ## Sub-module layout
//!
//! | File | Contents |
//! |------|----------|
//! | `mod.rs` (this file) | `show()` dispatcher — calls each section in order |
//! | `behavior.rs` | Behavior section (scrollback, shell exit, close confirmation) |
//! | `unicode.rs` | Unicode section (version, ambiguous width, normalization, answerback) |
//! | `shell.rs` | Shell section (custom shell, args, login shell, startup directory) |
//! | `startup.rs` | Startup section (restore session, undo close, initial text) |
//! | `search.rs` | Search, Scrollbar, Command History, and Command Separator sections |
//! | `semantic_history.rs` | Semantic History section (link handler, file path detection, editor) |

mod behavior;
mod search;
mod semantic_history;
mod shell;
mod startup;
mod unicode;

use crate::SettingsUI;
use crate::section::section_matches;
use std::collections::HashSet;

/// Show the terminal tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // Behavior section
    if section_matches(
        &query,
        "Behavior",
        &[
            "scrollback",
            "exit",
            "shell exit",
            "jobs",
            "confirm",
            "confirm close",
            "close",
            "running jobs",
            "process names",
            "ignore",
            "close tab",
        ],
    ) {
        behavior::show_behavior_section(ui, settings, changes_this_frame, collapsed);
    }

    // Unicode section (collapsed by default)
    if section_matches(
        &query,
        "Unicode",
        &[
            "unicode",
            "width",
            "answerback",
            "ambiguous",
            "normalization",
            "nfc",
            "nfd",
            "emoji",
            "east asian",
            "cjk",
        ],
    ) {
        unicode::show_unicode_section(ui, settings, changes_this_frame, collapsed);
    }

    // Shell section
    if section_matches(
        &query,
        "Shell",
        &[
            "shell",
            "custom shell",
            "shell args",
            "working directory",
            "login",
            "startup",
            "previous",
            "home",
            "directory mode",
            "last working directory",
            "custom directory",
        ],
    ) {
        shell::show_shell_section(ui, settings, changes_this_frame, collapsed);
    }

    // Startup section (collapsed by default)
    if section_matches(
        &query,
        "Startup",
        &[
            "initial text",
            "startup",
            "delay",
            "newline",
            "restore",
            "session",
            "restore tabs",
            "restore panes",
            "session state",
            "escape sequences",
            "undo",
            "undo close",
            "reopen",
            "reopen tab",
            "closed tab",
        ],
    ) {
        startup::show_startup_section(ui, settings, changes_this_frame, collapsed);
    }

    // Search section
    if section_matches(
        &query,
        "Search",
        &[
            "search",
            "highlight",
            "case sensitive",
            "regex",
            "wrap",
            "wrap around",
            "current match",
        ],
    ) {
        search::show_search_section(ui, settings, changes_this_frame, collapsed);
    }

    // Semantic History section
    if section_matches(
        &query,
        "Semantic History",
        &[
            "semantic",
            "history",
            "file",
            "editor",
            "path",
            "click",
            "vs code",
            "sublime",
            "vim",
            "editor mode",
            "system default",
        ],
    ) {
        semantic_history::show_semantic_history_section(
            ui,
            settings,
            changes_this_frame,
            collapsed,
        );
    }

    // Scrollbar section
    if section_matches(
        &query,
        "Scrollbar",
        &[
            "scrollbar",
            "thumb",
            "track",
            "autohide",
            "marker",
            "command markers",
            "shell integration",
            "tooltips",
        ],
    ) {
        search::show_scrollbar_section(ui, settings, changes_this_frame, collapsed);
    }

    // Command History section
    if section_matches(
        &query,
        "Command History",
        &[
            "command",
            "history",
            "fuzzy",
            "search",
            "entries",
            "max entries",
        ],
    ) {
        search::show_command_history_section(ui, settings, changes_this_frame, collapsed);
    }

    // Command Separators section
    if section_matches(
        &query,
        "Command Separators",
        &[
            "separator",
            "command",
            "line",
            "divider",
            "prompt",
            "exit code",
            "success",
            "failure",
        ],
    ) {
        search::show_command_separator_section(ui, settings, changes_this_frame, collapsed);
    }
}

/// Search keywords for the Terminal settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        // Behavior
        "shell",
        "scrollback",
        "scrollback lines",
        "exit",
        "shell exit",
        "exit action",
        "confirm",
        "confirm close",
        "running jobs",
        "jobs",
        "jobs to ignore",
        // Unicode
        "unicode",
        "unicode version",
        "width",
        "ambiguous",
        "ambiguous width",
        "answerback",
        // Shell
        "custom shell",
        "shell args",
        "login shell",
        "login",
        "working directory",
        "startup directory",
        "previous session",
        "home",
        // Startup
        "initial text",
        "startup",
        "delay",
        "newline",
        "undo",
        "undo close",
        "reopen",
        "reopen tab",
        "closed tab",
        "preserve shell",
        "preserve",
        "hide tab",
        // Search
        "search",
        "highlight",
        "search highlight",
        "case sensitive",
        "regex",
        "wrap",
        "wrap around",
        // Semantic history
        "semantic",
        "semantic history",
        "file path",
        "click file",
        "editor",
        "editor mode",
        "editor command",
        "link handler",
        "link highlight color",
        "link highlight underline",
        "link underline style",
        "stipple",
        "link color",
        "url color",
        "browser",
        "open url",
        "open links",
        "url handler",
        // Scrollbar
        "scrollbar",
        "thumb",
        "track",
        "autohide",
        "command marks",
        "marker",
        "mark",
        "tooltips",
        "scrollbar width",
        // Unicode extras
        "normalization",
        "text normalization",
        "nfc",
        "nfd",
        // Command history
        "command history",
        "history entries",
        "max history",
        // Command separators
        "command separator",
        "separator",
        "separator line",
        "separator thickness",
        "separator opacity",
        "exit code",
        // Session restore
        "restore session",
        "undo timeout",
        "undo entries",
    ]
}

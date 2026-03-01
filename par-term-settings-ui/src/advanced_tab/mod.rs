//! Advanced settings tab.
//!
//! Consolidates: tmux_tab, logging_tab, screenshot_tab, update_tab
//!
//! Contains:
//! - Import/export preferences
//! - tmux integration settings
//! - Session logging settings
//! - Screenshot settings
//! - Update settings
//! - File transfer settings
//! - Debug logging settings
//! - Security settings (env var allowlist)
//!
//! ## Sub-module layout
//!
//! | File | Contents |
//! |------|----------|
//! | `mod.rs` (this file) | `show()` dispatcher — calls each section in order |
//! | `import_export.rs` | Import/Export section + `merge_config` helper |
//! | `tmux.rs` | tmux Integration section |
//! | `logging.rs` | Session Logging section |
//! | `system.rs` | Screenshots, Updates, File Transfers, Debug Logging, Security sections |

mod import_export;
mod logging;
mod system;
mod tmux;

use crate::SettingsUI;
use crate::section::section_matches;
use std::collections::HashSet;

// Re-export merge_config — it is part of the public API of advanced_tab.
pub use import_export::merge_config;

/// Show the advanced tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // Import/Export Preferences section
    if section_matches(
        &query,
        "Import/Export Preferences",
        &[
            "import",
            "export",
            "preferences",
            "backup",
            "restore",
            "config",
            "yaml",
            "url import",
            "merge",
        ],
    ) {
        import_export::show_import_export_section(ui, settings, changes_this_frame, collapsed);
    }

    // tmux Integration section
    if section_matches(
        &query,
        "tmux Integration",
        &[
            "tmux",
            "control mode",
            "session",
            "attach",
            "prefix key",
            "status bar",
            "clipboard sync",
            "auto-attach",
        ],
    ) {
        tmux::show_tmux_section(ui, settings, changes_this_frame, collapsed);
    }

    // Session Logging section
    if section_matches(
        &query,
        "Session Logging",
        &[
            "logging",
            "recording",
            "asciicast",
            "asciinema",
            "plain text",
            "html",
            "auto-log",
            "log directory",
        ],
    ) {
        logging::show_logging_section(ui, settings, changes_this_frame, collapsed);
    }

    // Screenshots section (collapsed by default)
    if section_matches(
        &query,
        "Screenshots",
        &["screenshot", "format", "png", "jpeg", "svg", "capture"],
    ) {
        system::show_screenshot_section(ui, settings, changes_this_frame, collapsed);
    }

    // Updates section
    if section_matches(
        &query,
        "Updates",
        &[
            "update",
            "version",
            "check",
            "release",
            "frequency",
            "homebrew",
            "cargo",
            "self-update",
            "daily",
            "weekly",
            "monthly",
        ],
    ) {
        system::show_updates_section(ui, settings, changes_this_frame, collapsed);
    }

    // File Transfers section
    if section_matches(
        &query,
        "File Transfers",
        &[
            "download",
            "upload",
            "transfer",
            "file transfer",
            "save location",
            "save directory",
        ],
    ) {
        system::show_file_transfers_section(ui, settings, changes_this_frame, collapsed);
    }

    // Debug Logging section
    if section_matches(
        &query,
        "Debug Logging",
        &[
            "debug",
            "log",
            "log level",
            "log file",
            "trace",
            "verbose",
            "diagnostics",
        ],
    ) {
        system::show_debug_logging_section(ui, settings, changes_this_frame, collapsed);
    }

    // Security section
    if section_matches(
        &query,
        "Security",
        &[
            "security",
            "environment",
            "env var",
            "allowlist",
            "allow all env",
            "variable substitution",
        ],
    ) {
        system::show_security_section(ui, settings, changes_this_frame, collapsed);
    }
}

/// Search keywords for the Advanced settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        // tmux
        "tmux",
        "tmux integration",
        "tmux path",
        "control mode",
        "session",
        "default session",
        "auto-attach",
        "attach",
        "clipboard sync",
        "tmux clipboard",
        "status bar",
        "tmux status",
        "refresh interval",
        "prefix key",
        "prefix",
        // Session logging
        "logging",
        "session logging",
        "auto log",
        "auto-log",
        "recording",
        "asciicast",
        "asciinema",
        "log format",
        "log directory",
        "archive",
        "archive on close",
        "redact",
        "redact passwords",
        "password",
        "sensitive",
        "credentials",
        // Screenshots
        "screenshot",
        "screenshot format",
        "png",
        "jpeg",
        "svg",
        "html",
        // Updates
        "update",
        "version",
        "check",
        "release",
        "update check",
        "hourly",
        "skipped version",
        // File Transfers
        "download",
        "upload",
        "transfer",
        "file transfer",
        "save location",
        "save directory",
        // Debug Logging
        "debug",
        "debug logging",
        "log level",
        "log file",
        "trace",
        "verbose",
        "diagnostics",
        // Import/export preferences
        "import",
        "export",
        "preferences",
        "merge",
        "url",
        // Logging format extras
        "plain",
        "plain text",
        // tmux status format
        "left format",
        "right format",
        // Updates extras
        "check now",
        "daily",
        "weekly",
        "monthly",
        "homebrew",
        "brew",
        "cargo",
        "self-update",
        // Import/export extras
        "backup",
        "config",
        "url import",
        // Security
        "security",
        "environment",
        "env var",
        "allowlist",
        "allow all env",
        "variable substitution",
    ]
}

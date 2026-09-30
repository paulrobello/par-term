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
//! | `search.rs` | Search, Command History, and Command Separator sections |
//! | `semantic_history.rs` | Semantic History section (link handler, file path detection, editor) |

mod behavior;
mod search;
mod semantic_history;
mod shell;
mod startup;
mod unicode;

use crate::SettingsUI;
use std::collections::HashSet;

/// Show the terminal tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    // Behavior section
    behavior::show_behavior_section(ui, settings, changes_this_frame, collapsed);

    // Unicode section (collapsed by default)
    unicode::show_unicode_section(ui, settings, changes_this_frame, collapsed);

    // Shell section
    shell::show_shell_section(ui, settings, changes_this_frame, collapsed);

    // Startup section (collapsed by default)
    startup::show_startup_section(ui, settings, changes_this_frame, collapsed);

    // Search section
    search::show_search_section(ui, settings, changes_this_frame, collapsed);

    // Semantic History section
    semantic_history::show_semantic_history_section(ui, settings, changes_this_frame, collapsed);

    // Command History section
    search::show_command_history_section(ui, settings, changes_this_frame, collapsed);

    // Command Separators section
    search::show_command_separator_section(ui, settings, changes_this_frame, collapsed);
}

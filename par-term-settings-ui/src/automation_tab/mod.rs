//! Automation settings tab.
//!
//! Contains:
//! - Trigger definitions (regex patterns with actions)
//! - Coprocess definitions (external processes piped to terminal)
//! - External observer scripts (absorbed from scripts_tab)
//! - Local plugins (trust surface, enable toggle, schema editor)
//!
//! ## Sub-module layout
//!
//! | File | Contents |
//! |------|----------|
//! | `mod.rs` (this file) | `show()` dispatcher and `keywords()` |
//! | `triggers_section.rs` | Trigger list, edit form, action field rendering |
//! | `coprocesses_section.rs` | Coprocess list, edit form, output viewer |
//! | `plugins_section.rs` | Plugin list, trust surface, enable toggle, schema editor |
//! | `plugin_git_ui.rs` | Plugin git distribution: add from URL, update review, remove |

use crate::SettingsUI;
use std::collections::HashSet;

mod coprocesses_section;
mod plugin_git_ui;
mod plugins_section;
mod state;
mod triggers_section;

pub use state::AutomationTabState;

/// Show the automation tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    triggers_section::show_triggers_section(ui, settings, changes_this_frame, collapsed);
    coprocesses_section::show_coprocesses_section(ui, settings, changes_this_frame, collapsed);
    // Scripts section (absorbed from scripts_tab)
    crate::scripts_tab::show(ui, settings, changes_this_frame, collapsed);
    plugins_section::show_plugins_section(ui, settings, changes_this_frame, collapsed);
}

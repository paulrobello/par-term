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
use std::collections::HashSet;

mod editor;
mod list;
mod state;

pub use state::ScriptsTabState;

/// Show the scripts tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    list::show_scripts_section(ui, settings, changes_this_frame, collapsed);
}

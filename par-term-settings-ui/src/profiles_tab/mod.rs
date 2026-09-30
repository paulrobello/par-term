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
use std::collections::HashSet;

mod dynamic_sources;
mod management;
mod state;

pub use state::ProfilesTabState;

/// Show the profiles tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    // Profile management section (inline)
    management::show_management_section(ui, settings, collapsed);

    // Display options section
    management::show_display_options_section(ui, settings, collapsed);

    // Dynamic profile sources section
    dynamic_sources::show_dynamic_sources_section(ui, settings, changes_this_frame, collapsed);
}

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
mod state;
mod system;
mod tmux;

pub use state::AdvancedTabState;

use crate::SettingsUI;
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
    // Import/Export Preferences section
    import_export::show_import_export_section(ui, settings, changes_this_frame, collapsed);

    // tmux Integration section
    tmux::show_tmux_section(ui, settings, changes_this_frame, collapsed);

    // Session Logging section
    logging::show_logging_section(ui, settings, changes_this_frame, collapsed);

    // Screenshots section (collapsed by default)
    system::show_screenshot_section(ui, settings, changes_this_frame, collapsed);

    // Updates section
    system::show_updates_section(ui, settings, changes_this_frame, collapsed);

    // File Transfers section
    system::show_file_transfers_section(ui, settings, changes_this_frame, collapsed);

    // Debug Logging section
    system::show_debug_logging_section(ui, settings, changes_this_frame, collapsed);

    // Security section
    system::show_security_section(ui, settings, changes_this_frame, collapsed);
}

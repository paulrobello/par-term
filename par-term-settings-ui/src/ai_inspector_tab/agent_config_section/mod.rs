//! Agent configuration and permissions sections of the AI Inspector settings tab.
//!
//! Covers: Custom Agents (identity, run commands, env vars, Ollama context)
//! and Permissions (auto-approve / yolo mode, terminal access, screenshot access).
//!
//! ## Sub-module layout
//!
//! | File | Contents |
//! |------|----------|
//! | `mod.rs` (this file) | Public re-exports for parent `ai_inspector_tab/mod.rs` |
//! | `custom_agents.rs` | Custom Agents section (identity, run commands, env vars) |
//! | `permissions.rs` | Permissions section (yolo mode, terminal access, screenshots) |

use crate::SettingsUI;
use std::collections::HashSet;

mod custom_agents;
mod permissions;

pub(super) fn show_custom_agents_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    custom_agents::show_custom_agents_section(ui, settings, changes_this_frame, collapsed);
}

pub(super) fn show_permissions_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    permissions::show_permissions_section(ui, settings, changes_this_frame, collapsed);
}

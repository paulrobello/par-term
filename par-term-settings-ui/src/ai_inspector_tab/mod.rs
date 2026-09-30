//! AI Inspector settings tab.
//!
//! Contains:
//! - Panel settings (enabled, width, scope, view mode)
//! - Agent settings (default agent, auto-launch, auto-context)
//! - Custom Agent definitions (identity, run commands, env vars)
//! - Permission settings (auto-approve / yolo mode, terminal access, screenshot access)
//!
//! ## Sub-module layout
//!
//! | File | Contents |
//! |------|----------|
//! | `mod.rs` (this file) | `show()` dispatcher and `keywords()` |
//! | `context_section.rs` | Panel section + Agent section (scope, view mode, auto-context) |
//! | `agent_config_section.rs` | Custom Agents section + Permissions section |

use crate::SettingsUI;
use std::collections::HashSet;

mod agent_config_section;
mod context_section;
mod prompt_library;
mod state;

pub use state::AiInspectorTabState;

/// Show the AI Inspector tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    context_section::show_panel_section(ui, settings, changes_this_frame, collapsed);
    context_section::show_agent_section(ui, settings, changes_this_frame, collapsed);
    prompt_library::show_prompt_library_section(ui, settings, collapsed);
    agent_config_section::show_custom_agents_section(ui, settings, changes_this_frame, collapsed);
    agent_config_section::show_permissions_section(ui, settings, changes_this_frame, collapsed);
}

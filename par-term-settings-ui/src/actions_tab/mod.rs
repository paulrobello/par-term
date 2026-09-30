//! Actions settings tab.
//!
//! Contains:
//! - Custom action management (shell commands, text insertion, key sequences)
//! - Action editor with type selection
//! - Keybinding assignment for actions

mod action_editor;
mod action_forms;
mod action_list;
mod agent_commands_section;
mod agents_section;
mod state;

pub use state::ActionsTabState;

use crate::SettingsUI;
use std::collections::HashSet;

use action_list::show_actions_section;
use agent_commands_section::show_agent_commands_section;
use agents_section::show_agents_section;

/// Show the actions tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    // Actions section
    show_actions_section(ui, settings, changes_this_frame, collapsed);

    show_agent_commands_section(ui, settings, collapsed);

    show_agents_section(ui, settings, changes_this_frame, collapsed);
}

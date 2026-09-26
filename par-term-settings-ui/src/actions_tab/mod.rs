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
use crate::section::section_matches;
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
    let query = settings.search_query.trim().to_lowercase();

    // Actions section
    if section_matches(
        &query,
        "Actions",
        &[
            "action",
            "custom",
            "shell",
            "command",
            "text",
            "insert",
            "key",
            "sequence",
            "macro",
            "keybinding",
            "prefix",
            "execute",
            "run",
            "split",
            "pane",
        ],
    ) {
        show_actions_section(ui, settings, changes_this_frame, collapsed);
    }

    if section_matches(
        &query,
        "Agent Commands",
        &[
            "agent",
            "command",
            "agent-cmd",
            "script",
            "macro",
            "mcp",
            "delete",
        ],
    ) {
        show_agent_commands_section(ui, settings, collapsed);
    }

    if section_matches(
        &query,
        "Agents",
        &[
            "agent",
            "agents",
            "launcher",
            "launch",
            "palette",
            "autonomy",
            "autonomous",
            "default agent",
            "cli",
        ],
    ) {
        show_agents_section(ui, settings, changes_this_frame, collapsed);
    }
}

/// Search keywords for the Actions settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        "action",
        "actions",
        "custom action",
        "shell command",
        "new tab",
        "text insert",
        "key sequence",
        "macro",
        "automation",
        "shortcut",
        // Action details
        "keybinding",
        "binding",
        "record",
        "title",
        "name",
        "arguments",
        "split",
        "split pane",
        "pane",
        "horizontal",
        "vertical",
        // Workflow action types
        "workflow",
        "sequence",
        "condition",
        "repeat",
        "capture output",
        "capture_output",
        "exit code",
        "step",
        // Agent commands
        "agent command",
        "agent-cmd",
        "command file",
        "delete command",
        // Agent launcher
        "agents",
        "agent launcher",
        "launch agent",
        "launch default agent",
        "autonomy",
        "autonomous",
        "default agent",
        "cli agent",
    ]
}

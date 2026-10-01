//! Custom actions (Automation › Custom Actions), CLI agent launchers, and
//! agent commands (Assistant & Agents). Placement is set in
//! [`crate::layout`].
//!
//! - Custom action management (shell commands, text insertion, key sequences)
//! - Action editor with type selection
//! - Keybinding assignment for actions

mod action_editor;
mod action_forms;
pub(crate) mod action_list;
pub(crate) mod agent_commands_section;
pub(crate) mod agents_section;
mod state;

pub use state::ActionsTabState;

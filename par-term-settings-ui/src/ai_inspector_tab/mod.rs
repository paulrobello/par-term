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

/// Show the AI Inspector tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    context_section::show_panel_section(ui, settings, changes_this_frame, collapsed);
    context_section::show_agent_section(ui, settings, changes_this_frame, collapsed);
    agent_config_section::show_custom_agents_section(ui, settings, changes_this_frame, collapsed);
    agent_config_section::show_permissions_section(ui, settings, changes_this_frame, collapsed);
}

/// Search keywords for the AI Inspector (Assistant) settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        "ai",
        "inspector",
        "agent",
        "acp",
        "llm",
        "assistant",
        "zone",
        "command",
        "history",
        "context",
        "auto",
        "approve",
        "yolo",
        "terminal",
        "access",
        "drive",
        "execute",
        "live",
        "update",
        "scope",
        "cards",
        "timeline",
        "tree",
        "env",
        "environment",
        "anthropic",
        "ollama",
        "startup",
        "open",
        "width",
        // Panel
        "panel",
        // Permissions
        "permissions",
        "auto-approve",
        "auto approve",
        "screenshot",
        "screenshot access",
        // Agent extras
        "auto-send",
        "auto send",
        "max context",
        "context lines",
        "auto-launch",
        "auto launch",
        // View modes
        "list",
        "list detail",
        "recent",
        // Custom agents
        "custom",
        "identity",
        "short name",
        "install command",
        "connector",
        "run command",
        "active",
        "protocol",
        // Platform-specific
        "macos",
        "linux",
        "windows",
    ]
}

//! Automation settings tab.
//!
//! Contains:
//! - Trigger definitions (regex patterns with actions)
//! - Coprocess definitions (external processes piped to terminal)
//! - External observer scripts (absorbed from scripts_tab)
//!
//! ## Sub-module layout
//!
//! | File | Contents |
//! |------|----------|
//! | `mod.rs` (this file) | `show()` dispatcher and `keywords()` |
//! | `triggers_section.rs` | Trigger list, edit form, action field rendering |
//! | `coprocesses_section.rs` | Coprocess list, edit form, output viewer |

use crate::SettingsUI;
use std::collections::HashSet;

mod coprocesses_section;
mod triggers_section;

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
}

/// Search keywords for the Automation settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        "trigger",
        "triggers",
        "regex",
        "pattern",
        "match",
        "automation",
        "automate",
        "action",
        "highlight",
        "notify",
        "notification",
        "run command",
        "play sound",
        "send text",
        "coprocess",
        "coprocesses",
        "pipe",
        "subprocess",
        "auto start",
        "auto-start",
        // Trigger action extras
        "mark line",
        "set variable",
        "variable",
        "foreground",
        "foreground color",
        // Prettify action
        "prettify",
        "prettifier",
        "scope",
        "command output",
        // Trigger security
        "prompt before run",
        "prompt",
        "confirm",
        "dialog",
        "split pane",
        "split",
        "pane",
        "security",
        "denylist",
        "rate limit",
        "dangerous",
        "safe",
        // Coprocess extras
        "restart",
        "restart policy",
        "restart delay",
        // Scripts (absorbed from scripts_tab)
        "script",
        "scripting",
        "python",
        "observer",
        "event",
        "external",
        "panel",
        "subscriptions",
        "script path",
        "arguments",
        "args",
        "start",
        "stop",
        "auto-launch",
        "permission",
        "allow",
        "write text",
        "change config",
    ]
}

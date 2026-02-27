//! JSON protocol types for communication between the terminal and script subprocesses.
//!
//! Scripts read [`ScriptEvent`] objects from stdin (one JSON object per line) and write
//! [`ScriptCommand`] objects to stdout (one JSON object per line).
//!
//! # Security Model
//!
//! ## Trust Assumptions
//!
//! Scripts are user-configured subprocesses launched from `ScriptConfig` entries in
//! `~/.config/par-term/config.yaml`. The script binary is implicitly trusted (it was
//! placed there by the user). However, this trust must be bounded because:
//!
//! 1. **Supply-chain attacks**: A malicious package could replace a trusted script
//!    with one that emits dangerous command payloads.
//! 2. **Injection through event data**: Malicious terminal sequences could produce
//!    events whose payloads are forwarded to the script, which could reflect them
//!    back in commands (terminal injection risk).
//! 3. **Compromised scripts**: A script may be modified after initial deployment.
//!
//! ## Command Categories
//!
//! Script commands fall into three security categories:
//!
//! ### Safe Commands (no permission required)
//! - `Log`: Write to the script's output buffer (UI only)
//! - `SetPanel` / `ClearPanel`: Display markdown content in a panel
//! - `Notify`: Show a desktop notification
//! - `SetBadge`: Set the tab badge text
//! - `SetVariable`: Set a user variable
//!
//! ### Restricted Commands (require permission flags)
//! These commands require explicit opt-in via `ScriptConfig` permission fields:
//! - `WriteText`: Inject text into the PTY (requires `allow_write_text: true`)
//!   - Must strip VT/ANSI escape sequences before writing
//!   - Subject to rate limiting
//! - `RunCommand`: Spawn an external process (requires `allow_run_command: true`)
//!   - Must check against `check_command_denylist()` from par-term-config
//!   - Must use shell tokenization (not `/bin/sh -c`) to prevent metacharacter injection
//!   - Subject to rate limiting
//! - `ChangeConfig`: Modify terminal configuration (requires `allow_change_config: true`)
//!   - Must validate config keys against an allowlist
//!
//! ## Implementation Status
//!
//! As of the current version:
//! - `Log`, `SetPanel`, `ClearPanel`: **Implemented** (safe, always allowed)
//! - `WriteText`, `RunCommand`, `ChangeConfig`: **Not yet implemented**
//!   - These require the permission infrastructure described above
//!   - See `par-term-scripting/SECURITY.md` for full implementation requirements
//!
//! ## Dispatcher Responsibility
//!
//! The command dispatcher in `src/app/window_manager/scripting.rs` is responsible for:
//! 1. Checking `command.requires_permission()` before executing restricted commands
//! 2. Verifying the corresponding `ScriptConfig.allow_*` flag is set
//! 3. Applying rate limits, denylists, and input sanitization
//!
//! See `par-term-scripting/SECURITY.md` for the complete security model.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An event sent from the terminal to a script subprocess (via stdin).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScriptEvent {
    /// Event kind name (e.g., "bell_rang", "cwd_changed", "command_complete").
    pub kind: String,
    /// Event-specific payload.
    pub data: ScriptEventData,
}

/// Event-specific payload data.
///
/// Tagged with `data_type` so the JSON includes a discriminant field for Python scripts
/// to easily dispatch on.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "data_type")]
pub enum ScriptEventData {
    /// Empty payload for events that carry no additional data (e.g., BellRang).
    Empty {},

    /// The current working directory changed.
    CwdChanged {
        /// New working directory path.
        cwd: String,
    },

    /// A command completed execution.
    CommandComplete {
        /// The command that completed.
        command: String,
        /// Exit code, if available.
        exit_code: Option<i32>,
    },

    /// The terminal title changed.
    TitleChanged {
        /// New terminal title.
        title: String,
    },

    /// The terminal size changed.
    SizeChanged {
        /// Number of columns.
        cols: usize,
        /// Number of rows.
        rows: usize,
    },

    /// A user variable changed.
    VariableChanged {
        /// Variable name.
        name: String,
        /// New value.
        value: String,
        /// Previous value, if any.
        old_value: Option<String>,
    },

    /// An environment variable changed.
    EnvironmentChanged {
        /// Environment variable key.
        key: String,
        /// New value.
        value: String,
        /// Previous value, if any.
        old_value: Option<String>,
    },

    /// The badge text changed.
    BadgeChanged {
        /// New badge text, or None if cleared.
        text: Option<String>,
    },

    /// A trigger pattern was matched.
    TriggerMatched {
        /// The trigger pattern that matched.
        pattern: String,
        /// The text that matched.
        matched_text: String,
        /// Line number where the match occurred.
        line: usize,
    },

    /// A semantic zone event occurred.
    ZoneEvent {
        /// Zone identifier.
        zone_id: u64,
        /// Type of zone.
        zone_type: String,
        /// Event type (e.g., "enter", "exit").
        event: String,
    },

    /// Fallback for unmapped events. Carries arbitrary key-value fields.
    Generic {
        /// Arbitrary event fields.
        fields: HashMap<String, serde_json::Value>,
    },
}

/// A command sent from a script subprocess to the terminal (via stdout).
///
/// Tagged with `type` for easy JSON dispatch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ScriptCommand {
    /// Write text to the PTY.
    WriteText {
        /// Text to write.
        text: String,
    },

    /// Show a desktop notification.
    Notify {
        /// Notification title.
        title: String,
        /// Notification body.
        body: String,
    },

    /// Set the tab badge text.
    SetBadge {
        /// Badge text to display.
        text: String,
    },

    /// Set a user variable.
    SetVariable {
        /// Variable name.
        name: String,
        /// Variable value.
        value: String,
    },

    /// Execute a shell command.
    RunCommand {
        /// Command to execute.
        command: String,
    },

    /// Change a configuration value.
    ChangeConfig {
        /// Configuration key.
        key: String,
        /// New value.
        value: serde_json::Value,
    },

    /// Log a message.
    Log {
        /// Log level (e.g., "info", "warn", "error", "debug").
        level: String,
        /// Log message.
        message: String,
    },

    /// Set a markdown panel.
    SetPanel {
        /// Panel title.
        title: String,
        /// Markdown content.
        content: String,
    },

    /// Clear the markdown panel.
    ClearPanel {},
}

impl ScriptCommand {
    /// Returns `true` if this command requires explicit permission in the script config.
    ///
    /// Commands that return `true` must have their corresponding `allow_*` flag set
    /// in `ScriptConfig` before the dispatcher will execute them.
    ///
    /// # Security Classification
    ///
    /// | Command | Requires Permission | Risk Level |
    /// |---------|--------------------| -----------|
    /// | `Log` | No | Low (UI output only) |
    /// | `SetPanel` / `ClearPanel` | No | Low (UI display only) |
    /// | `Notify` | No | Low (desktop notification) |
    /// | `SetBadge` | No | Low (tab badge display) |
    /// | `SetVariable` | No | Low (user variable storage) |
    /// | `WriteText` | **Yes** | High (PTY injection, command execution) |
    /// | `RunCommand` | **Yes** | Critical (arbitrary process spawn) |
    /// | `ChangeConfig` | **Yes** | High (config modification) |
    pub fn requires_permission(&self) -> bool {
        matches!(
            self,
            ScriptCommand::RunCommand { .. }
                | ScriptCommand::WriteText { .. }
                | ScriptCommand::ChangeConfig { .. }
        )
    }

    /// Returns the name of the permission flag required to execute this command.
    ///
    /// Returns `None` for commands that don't require permission.
    /// The returned string corresponds to a field in `ScriptConfig`:
    /// - `"allow_run_command"` for `RunCommand`
    /// - `"allow_write_text"` for `WriteText`
    /// - `"allow_change_config"` for `ChangeConfig`
    pub fn permission_flag_name(&self) -> Option<&'static str> {
        match self {
            ScriptCommand::RunCommand { .. } => Some("allow_run_command"),
            ScriptCommand::WriteText { .. } => Some("allow_write_text"),
            ScriptCommand::ChangeConfig { .. } => Some("allow_change_config"),
            _ => None,
        }
    }

    /// Returns `true` if this command can safely be executed without rate limiting.
    ///
    /// Commands that may be emitted frequently (like `Log`) should not be rate-limited
    /// to avoid dropping important debug output. High-impact commands (`WriteText`,
    /// `RunCommand`) must be rate-limited to prevent abuse.
    pub fn is_rate_limited(&self) -> bool {
        matches!(
            self,
            ScriptCommand::RunCommand { .. } | ScriptCommand::WriteText { .. }
        )
    }

    /// Returns a human-readable name for this command type (for logging/errors).
    pub fn command_name(&self) -> &'static str {
        match self {
            ScriptCommand::WriteText { .. } => "WriteText",
            ScriptCommand::Notify { .. } => "Notify",
            ScriptCommand::SetBadge { .. } => "SetBadge",
            ScriptCommand::SetVariable { .. } => "SetVariable",
            ScriptCommand::RunCommand { .. } => "RunCommand",
            ScriptCommand::ChangeConfig { .. } => "ChangeConfig",
            ScriptCommand::Log { .. } => "Log",
            ScriptCommand::SetPanel { .. } => "SetPanel",
            ScriptCommand::ClearPanel {} => "ClearPanel",
        }
    }
}

//! JSON protocol types for communication between the terminal and script subprocesses.
//!
//! Scripts read [`ScriptEvent`] objects from stdin (one JSON object per line) and write
//! [`ScriptCommand`] objects to stdout (one JSON object per line).

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

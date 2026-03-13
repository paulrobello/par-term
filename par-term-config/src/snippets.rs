//! Configuration types for snippets and custom actions.
//!
//! This module provides:
//! - Snippet definitions with variable substitution
//! - Custom action definitions (shell commands, text insertion, key sequences)
//! - Built-in and custom variable support

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Default timeout for shell commands (30 seconds).
const fn default_shell_command_timeout_secs() -> u64 {
    30
}

/// A text snippet that can be inserted into the terminal.
///
/// Snippets support variable substitution using \(variable\) syntax.
/// Example: "echo 'Today is \(date)'" will replace \(date) with the current date.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SnippetConfig {
    /// Unique identifier for the snippet
    pub id: String,

    /// Human-readable title for the snippet
    pub title: String,

    /// The text content to insert (may contain variables)
    pub content: String,

    /// Optional keyboard shortcut to trigger the snippet (e.g., "Ctrl+Shift+D")
    #[serde(default)]
    pub keybinding: Option<String>,

    /// Whether the keybinding is enabled (default: true)
    /// If false, the keybinding won't be registered even if keybinding is set
    #[serde(default = "crate::defaults::bool_true")]
    pub keybinding_enabled: bool,

    /// Optional folder/collection for organization (e.g., "Git", "Docker")
    #[serde(default)]
    pub folder: Option<String>,

    /// Whether this snippet is enabled
    #[serde(default = "crate::defaults::bool_true")]
    pub enabled: bool,

    /// Optional description of what the snippet does
    #[serde(default)]
    pub description: Option<String>,

    /// Whether to automatically send Enter after inserting the snippet (default: false)
    /// If true, a newline character is appended to execute the command immediately
    #[serde(default)]
    pub auto_execute: bool,

    /// Custom variables defined for this snippet
    #[serde(default)]
    pub variables: HashMap<String, String>,
}

impl SnippetConfig {
    /// Create a new snippet with the given ID and title.
    pub fn new(id: String, title: String, content: String) -> Self {
        Self {
            id,
            title,
            content,
            keybinding: None,
            keybinding_enabled: true,
            folder: None,
            enabled: true,
            description: None,
            auto_execute: false,
            variables: HashMap::new(),
        }
    }

    /// Add a keybinding to the snippet.
    pub fn with_keybinding(mut self, keybinding: String) -> Self {
        self.keybinding = Some(keybinding);
        self
    }

    /// Disable the keybinding for this snippet.
    pub fn with_keybinding_disabled(mut self) -> Self {
        self.keybinding_enabled = false;
        self
    }

    /// Add a folder to the snippet.
    pub fn with_folder(mut self, folder: String) -> Self {
        self.folder = Some(folder);
        self
    }

    /// Add a custom variable to the snippet.
    pub fn with_variable(mut self, name: String, value: String) -> Self {
        self.variables.insert(name, value);
        self
    }

    /// Enable auto-execute (send Enter after inserting the snippet).
    pub fn with_auto_execute(mut self) -> Self {
        self.auto_execute = true;
        self
    }
}

/// A portable snippet library for import/export.
///
/// Wraps a list of snippets for serialization to/from YAML files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetLibrary {
    /// The snippets in this library
    pub snippets: Vec<SnippetConfig>,
}

/// Default delay in ms before sending text to a newly split pane.
const fn default_split_pane_delay_ms() -> u64 {
    200
}

/// Normalize an action prefix character for matching and conflict detection.
///
/// ASCII letters are matched case-insensitively; all other characters remain exact.
pub fn normalize_action_prefix_char(ch: char) -> char {
    if ch.is_ascii_alphabetic() {
        ch.to_ascii_lowercase()
    } else {
        ch
    }
}

/// Default split percent: existing pane keeps 66% of the space.
const fn default_split_percent() -> u8 {
    66
}

/// Split direction for a custom action pane split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ActionSplitDirection {
    /// New pane below (panes stacked top/bottom)
    #[default]
    Horizontal,
    /// New pane to the right (side by side)
    Vertical,
}

impl ActionSplitDirection {
    /// All directions for UI dropdowns.
    pub fn all() -> &'static [ActionSplitDirection] {
        &[Self::Horizontal, Self::Vertical]
    }

    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Horizontal => "Horizontal (below)",
            Self::Vertical => "Vertical (right)",
        }
    }
}

/// What to do when a sequence step "fails".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SequenceStepBehavior {
    /// Halt sequence and show error toast (default).
    #[default]
    Abort,
    /// Halt sequence silently.
    Stop,
    /// Ignore failure and continue to the next step.
    Continue,
}

/// A single step in a Sequence action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SequenceStep {
    /// ID of the action to execute.
    pub action_id: String,
    /// Delay in milliseconds before this step runs (default: 0).
    #[serde(default)]
    pub delay_ms: u64,
    /// What to do if this step fails (default: Abort).
    #[serde(default)]
    pub on_failure: SequenceStepBehavior,
}

/// Condition to check for a Condition action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConditionCheck {
    /// Check the exit code of the last captured ShellCommand.
    ExitCode { value: i32 },
    /// Check whether the last captured output contains a pattern.
    OutputContains {
        pattern: String,
        #[serde(default)]
        case_sensitive: bool,
    },
    /// Check an environment variable (None value = existence check only).
    EnvVar {
        name: String,
        #[serde(default)]
        value: Option<String>,
    },
    /// Glob match on the current terminal CWD.
    DirMatches { pattern: String },
    /// Glob match on the current git branch.
    GitBranch { pattern: String },
}

/// A custom action that can be triggered via keybinding.
///
/// Actions can execute shell commands, open a new tab, insert text, simulate key
/// sequences, or split the active pane.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CustomActionConfig {
    /// Execute a shell command
    ShellCommand {
        /// Action identifier (for keybinding reference)
        id: String,

        /// Human-readable title
        title: String,

        /// Command to execute (e.g., "git", "npm")
        command: String,

        /// Command arguments (e.g., ["status", "--short"])
        #[serde(default)]
        args: Vec<String>,

        /// Whether to show command output in a notification
        #[serde(default)]
        notify_on_success: bool,

        /// Timeout in seconds for the command (default: 30)
        #[serde(default = "default_shell_command_timeout_secs")]
        timeout_secs: u64,

        /// Capture stdout+stderr into WorkflowContext for use by Sequence/Condition actions.
        /// When true, output is capped at 64 KB. Default: false.
        #[serde(default)]
        capture_output: bool,

        /// Optional keyboard shortcut to trigger the action (e.g., "Ctrl+Shift+R")
        #[serde(default)]
        keybinding: Option<String>,

        /// Optional single character triggered after the global custom action prefix key.
        #[serde(default)]
        prefix_char: Option<char>,

        /// Whether the keybinding is enabled (default: true)
        #[serde(default = "crate::defaults::bool_true")]
        keybinding_enabled: bool,

        /// Optional description
        #[serde(default)]
        description: Option<String>,
    },

    /// Open a new tab and optionally run a command in its shell
    NewTab {
        /// Action identifier
        id: String,

        /// Human-readable title
        title: String,

        /// Optional command to send to the new tab's shell after it opens
        #[serde(default)]
        command: Option<String>,

        /// Optional keyboard shortcut to trigger the action
        #[serde(default)]
        keybinding: Option<String>,

        /// Optional single character triggered after the global custom action prefix key.
        #[serde(default)]
        prefix_char: Option<char>,

        /// Whether the keybinding is enabled (default: true)
        #[serde(default = "crate::defaults::bool_true")]
        keybinding_enabled: bool,

        /// Optional description
        #[serde(default)]
        description: Option<String>,
    },

    /// Insert text into the terminal (like a snippet but no editing UI)
    InsertText {
        /// Action identifier
        id: String,

        /// Human-readable title
        title: String,

        /// Text to insert (supports variable substitution)
        text: String,

        /// Custom variables for substitution
        #[serde(default)]
        variables: HashMap<String, String>,

        /// Optional keyboard shortcut to trigger the action
        #[serde(default)]
        keybinding: Option<String>,

        /// Optional single character triggered after the global custom action prefix key.
        #[serde(default)]
        prefix_char: Option<char>,

        /// Whether the keybinding is enabled (default: true)
        #[serde(default = "crate::defaults::bool_true")]
        keybinding_enabled: bool,

        /// Optional description
        #[serde(default)]
        description: Option<String>,
    },

    /// Simulate a key sequence
    KeySequence {
        /// Action identifier
        id: String,

        /// Human-readable title
        title: String,

        /// Key sequence to simulate (e.g., "Ctrl+C", "Up Up Down Down")
        keys: String,

        /// Optional keyboard shortcut to trigger the action
        #[serde(default)]
        keybinding: Option<String>,

        /// Optional single character triggered after the global custom action prefix key.
        #[serde(default)]
        prefix_char: Option<char>,

        /// Whether the keybinding is enabled (default: true)
        #[serde(default = "crate::defaults::bool_true")]
        keybinding_enabled: bool,

        /// Optional description
        #[serde(default)]
        description: Option<String>,
    },

    /// Split the active pane and optionally send a command to the new pane
    SplitPane {
        /// Action identifier
        id: String,

        /// Human-readable title
        title: String,

        /// Split direction: horizontal (new pane below) or vertical (new pane right)
        #[serde(default)]
        direction: ActionSplitDirection,

        /// Command for the new pane.
        ///
        /// Behaviour depends on `command_is_direct`:
        /// - `false` (default): text is sent to the shell with a trailing newline after `delay_ms`.
        /// - `true`: the string is split on whitespace and used as the pane's initial process
        ///   (like running `htop` directly). The pane closes when the process exits.
        #[serde(default)]
        command: Option<String>,

        /// When `true`, `command` is the pane's initial process (argv), not a shell command.
        /// The pane closes when the process exits. `delay_ms` is ignored.
        /// When `false` (default), `command` is sent as text to the shell.
        #[serde(default)]
        command_is_direct: bool,

        /// Whether to focus the new pane after splitting (default: true)
        #[serde(default = "crate::defaults::bool_true")]
        focus_new_pane: bool,

        /// Delay in ms before sending the command text to the new pane (default: 200).
        /// Only used when `command_is_direct` is `false`.
        #[serde(default = "default_split_pane_delay_ms")]
        delay_ms: u64,

        /// Percent of the current pane that the existing pane retains after the split.
        /// Range 10–90. Default: 66 (existing pane keeps 66%, new pane gets 34%).
        #[serde(default = "default_split_percent")]
        split_percent: u8,

        /// Optional keyboard shortcut to trigger the action
        #[serde(default)]
        keybinding: Option<String>,

        /// Optional single character triggered after the global custom action prefix key.
        #[serde(default)]
        prefix_char: Option<char>,

        /// Whether the keybinding is enabled (default: true)
        #[serde(default = "crate::defaults::bool_true")]
        keybinding_enabled: bool,

        /// Optional description
        #[serde(default)]
        description: Option<String>,
    },

    /// Run an ordered list of actions (steps) in sequence.
    Sequence {
        /// Action identifier
        id: String,
        /// Human-readable title
        title: String,
        /// Optional keyboard shortcut
        #[serde(default)]
        keybinding: Option<String>,
        /// Optional single character triggered after the global custom action prefix key.
        #[serde(default)]
        prefix_char: Option<char>,
        /// Whether the keybinding is enabled (default: true)
        #[serde(default = "crate::defaults::bool_true")]
        keybinding_enabled: bool,
        /// Optional description
        #[serde(default)]
        description: Option<String>,
        /// Ordered list of steps to execute.
        #[serde(default)]
        steps: Vec<SequenceStep>,
    },

    /// Evaluate a condition and branch to different actions.
    Condition {
        /// Action identifier
        id: String,
        /// Human-readable title
        title: String,
        /// Optional keyboard shortcut
        #[serde(default)]
        keybinding: Option<String>,
        /// Optional single character triggered after the global custom action prefix key.
        #[serde(default)]
        prefix_char: Option<char>,
        /// Whether the keybinding is enabled (default: true)
        #[serde(default = "crate::defaults::bool_true")]
        keybinding_enabled: bool,
        /// Optional description
        #[serde(default)]
        description: Option<String>,
        /// The condition to evaluate.
        check: ConditionCheck,
        /// Action ID to execute when check is true (standalone use only; ignored in Sequence).
        #[serde(default)]
        on_true_id: Option<String>,
        /// Action ID to execute when check is false (standalone use only; ignored in Sequence).
        #[serde(default)]
        on_false_id: Option<String>,
    },

    /// Execute an action repeatedly up to N times.
    Repeat {
        /// Action identifier
        id: String,
        /// Human-readable title
        title: String,
        /// Optional keyboard shortcut
        #[serde(default)]
        keybinding: Option<String>,
        /// Optional single character triggered after the global custom action prefix key.
        #[serde(default)]
        prefix_char: Option<char>,
        /// Whether the keybinding is enabled (default: true)
        #[serde(default = "crate::defaults::bool_true")]
        keybinding_enabled: bool,
        /// Optional description
        #[serde(default)]
        description: Option<String>,
        /// ID of the action to repeat.
        action_id: String,
        /// Maximum number of repetitions (1–100).
        count: u32,
        /// Delay in milliseconds between repetitions (default: 0).
        #[serde(default)]
        delay_ms: u64,
        /// Stop early when the action succeeds (default: false).
        #[serde(default)]
        stop_on_success: bool,
        /// Stop early when the action fails (default: false).
        #[serde(default)]
        stop_on_failure: bool,
    },
}

impl CustomActionConfig {
    /// Get the action ID (for keybinding reference).
    pub fn id(&self) -> &str {
        match self {
            Self::ShellCommand { id, .. }
            | Self::NewTab { id, .. }
            | Self::InsertText { id, .. }
            | Self::KeySequence { id, .. }
            | Self::SplitPane { id, .. }
            | Self::Sequence { id, .. }
            | Self::Condition { id, .. }
            | Self::Repeat { id, .. } => id,
        }
    }

    /// Get the action title (for UI display).
    pub fn title(&self) -> &str {
        match self {
            Self::ShellCommand { title, .. }
            | Self::NewTab { title, .. }
            | Self::InsertText { title, .. }
            | Self::KeySequence { title, .. }
            | Self::SplitPane { title, .. }
            | Self::Sequence { title, .. }
            | Self::Condition { title, .. }
            | Self::Repeat { title, .. } => title,
        }
    }

    /// Get the optional keybinding for this action.
    pub fn keybinding(&self) -> Option<&str> {
        match self {
            Self::ShellCommand { keybinding, .. }
            | Self::NewTab { keybinding, .. }
            | Self::InsertText { keybinding, .. }
            | Self::KeySequence { keybinding, .. }
            | Self::SplitPane { keybinding, .. }
            | Self::Sequence { keybinding, .. }
            | Self::Condition { keybinding, .. }
            | Self::Repeat { keybinding, .. } => keybinding.as_deref(),
        }
    }

    /// Get the optional prefix character for this action.
    pub fn prefix_char(&self) -> Option<char> {
        match self {
            Self::ShellCommand { prefix_char, .. }
            | Self::NewTab { prefix_char, .. }
            | Self::InsertText { prefix_char, .. }
            | Self::KeySequence { prefix_char, .. }
            | Self::SplitPane { prefix_char, .. }
            | Self::Sequence { prefix_char, .. }
            | Self::Condition { prefix_char, .. }
            | Self::Repeat { prefix_char, .. } => *prefix_char,
        }
    }

    /// Get the normalized prefix character for this action, if configured.
    pub fn normalized_prefix_char(&self) -> Option<char> {
        self.prefix_char().map(normalize_action_prefix_char)
    }

    /// Check if the keybinding is enabled.
    pub fn keybinding_enabled(&self) -> bool {
        match self {
            Self::ShellCommand {
                keybinding_enabled, ..
            }
            | Self::NewTab {
                keybinding_enabled, ..
            }
            | Self::InsertText {
                keybinding_enabled, ..
            }
            | Self::KeySequence {
                keybinding_enabled, ..
            }
            | Self::SplitPane {
                keybinding_enabled, ..
            }
            | Self::Sequence {
                keybinding_enabled, ..
            }
            | Self::Condition {
                keybinding_enabled, ..
            }
            | Self::Repeat {
                keybinding_enabled, ..
            } => *keybinding_enabled,
        }
    }

    /// Set the keybinding for this action.
    pub fn set_keybinding(&mut self, kb: Option<String>) {
        match self {
            Self::ShellCommand { keybinding, .. }
            | Self::NewTab { keybinding, .. }
            | Self::InsertText { keybinding, .. }
            | Self::KeySequence { keybinding, .. }
            | Self::SplitPane { keybinding, .. }
            | Self::Sequence { keybinding, .. }
            | Self::Condition { keybinding, .. }
            | Self::Repeat { keybinding, .. } => *keybinding = kb,
        }
    }

    /// Set the prefix character for this action.
    pub fn set_prefix_char(&mut self, prefix_char: Option<char>) {
        match self {
            Self::ShellCommand {
                prefix_char: current,
                ..
            }
            | Self::NewTab {
                prefix_char: current,
                ..
            }
            | Self::InsertText {
                prefix_char: current,
                ..
            }
            | Self::KeySequence {
                prefix_char: current,
                ..
            }
            | Self::SplitPane {
                prefix_char: current,
                ..
            }
            | Self::Sequence {
                prefix_char: current,
                ..
            }
            | Self::Condition {
                prefix_char: current,
                ..
            }
            | Self::Repeat {
                prefix_char: current,
                ..
            } => *current = prefix_char,
        }
    }

    /// Set whether the keybinding is enabled.
    pub fn set_keybinding_enabled(&mut self, enabled: bool) {
        match self {
            Self::ShellCommand {
                keybinding_enabled, ..
            }
            | Self::NewTab {
                keybinding_enabled, ..
            }
            | Self::InsertText {
                keybinding_enabled, ..
            }
            | Self::KeySequence {
                keybinding_enabled, ..
            }
            | Self::SplitPane {
                keybinding_enabled, ..
            }
            | Self::Sequence {
                keybinding_enabled, ..
            }
            | Self::Condition {
                keybinding_enabled, ..
            }
            | Self::Repeat {
                keybinding_enabled, ..
            } => *keybinding_enabled = enabled,
        }
    }

    /// Check if this is a shell command action.
    pub fn is_shell_command(&self) -> bool {
        matches!(self, Self::ShellCommand { .. })
    }

    /// Check if this is a new tab action.
    pub fn is_new_tab(&self) -> bool {
        matches!(self, Self::NewTab { .. })
    }

    /// Check if this is an insert text action.
    pub fn is_insert_text(&self) -> bool {
        matches!(self, Self::InsertText { .. })
    }

    /// Check if this is a key sequence action.
    pub fn is_key_sequence(&self) -> bool {
        matches!(self, Self::KeySequence { .. })
    }

    /// Check if this is a split pane action.
    pub fn is_split_pane(&self) -> bool {
        matches!(self, Self::SplitPane { .. })
    }
}

/// Built-in variables available for snippet substitution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltInVariable {
    /// Current date (YYYY-MM-DD)
    Date,
    /// Current time (HH:MM:SS)
    Time,
    /// Current date and time
    DateTime,
    /// System hostname
    Hostname,
    /// Current username
    User,
    /// Current working directory
    Path,
    /// Current git branch (if in a git repository)
    GitBranch,
    /// Current git commit hash (if in a git repository)
    GitCommit,
    /// Random UUID
    Uuid,
    /// Random number (0-999999)
    Random,
}

impl BuiltInVariable {
    /// Get all built-in variables for UI display.
    pub fn all() -> &'static [(&'static str, &'static str)] {
        &[
            ("date", "Current date (YYYY-MM-DD)"),
            ("time", "Current time (HH:MM:SS)"),
            ("datetime", "Current date and time"),
            ("hostname", "System hostname"),
            ("user", "Current username"),
            ("path", "Current working directory"),
            ("git_branch", "Current git branch"),
            ("git_commit", "Current git commit hash"),
            ("uuid", "Random UUID"),
            ("random", "Random number (0-999999)"),
        ]
    }

    /// Parse a variable name into a BuiltInVariable.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "date" => Some(Self::Date),
            "time" => Some(Self::Time),
            "datetime" => Some(Self::DateTime),
            "hostname" => Some(Self::Hostname),
            "user" => Some(Self::User),
            "path" => Some(Self::Path),
            "git_branch" => Some(Self::GitBranch),
            "git_commit" => Some(Self::GitCommit),
            "uuid" => Some(Self::Uuid),
            "random" => Some(Self::Random),
            _ => None,
        }
    }

    /// Resolve the variable to its string value.
    pub fn resolve(&self) -> String {
        match self {
            Self::Date => {
                use std::time::{SystemTime, UNIX_EPOCH};
                let duration = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default();
                let secs = duration.as_secs();
                let days_since_epoch = secs / 86400;

                // Simple date calculation (days since 1970-01-01)
                let years = 1970 + days_since_epoch / 365;
                let day_of_year = (days_since_epoch % 365) as u32;
                let month = (day_of_year / 30) + 1;
                let day = (day_of_year % 30) + 1;

                format!("{:04}-{:02}-{:02}", years, month, day)
            }
            Self::Time => {
                use std::time::{SystemTime, UNIX_EPOCH};
                let duration = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default();
                let secs = duration.as_secs();
                let hours = (secs % 86400) / 3600;
                let minutes = (secs % 3600) / 60;
                let seconds = secs % 60;

                format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
            }
            Self::DateTime => {
                format!("{} {}", Self::Date.resolve(), Self::Time.resolve())
            }
            Self::Hostname => {
                std::env::var("HOSTNAME")
                    .or_else(|_| std::env::var("HOST"))
                    .unwrap_or_else(|_| {
                        // Fallback to system hostname
                        hostname::get()
                            .ok()
                            .and_then(|s| s.into_string().ok())
                            .unwrap_or_else(|| "unknown".to_string())
                    })
            }
            Self::User => std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_else(|_| "unknown".to_string()),
            Self::Path => std::env::current_dir()
                .ok()
                .and_then(|p| p.to_str().map(|s| s.to_string()))
                .unwrap_or_else(|| ".".to_string()),
            Self::GitBranch => {
                // Try to get git branch from environment or command
                match std::env::var("GIT_BRANCH") {
                    Ok(branch) => branch,
                    Err(_) => {
                        // Try running git command
                        std::process::Command::new("git")
                            .args(["rev-parse", "--abbrev-ref", "HEAD"])
                            .output()
                            .ok()
                            .and_then(|o| String::from_utf8(o.stdout).ok())
                            .map(|s| s.trim().to_string())
                            .unwrap_or_default()
                    }
                }
            }
            Self::GitCommit => {
                // Try to get git commit from environment or command
                match std::env::var("GIT_COMMIT") {
                    Ok(commit) => commit,
                    Err(_) => std::process::Command::new("git")
                        .args(["rev-parse", "--short", "HEAD"])
                        .output()
                        .ok()
                        .and_then(|o| String::from_utf8(o.stdout).ok())
                        .map(|s| s.trim().to_string())
                        .unwrap_or_default(),
                }
            }
            Self::Uuid => uuid::Uuid::new_v4().to_string(),
            Self::Random => {
                use std::time::{SystemTime, UNIX_EPOCH};
                let duration = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default();
                format!("{}", (duration.as_nanos() % 1_000_000) as u32)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snippet_new() {
        let snippet = SnippetConfig::new(
            "test".to_string(),
            "Test Snippet".to_string(),
            "echo 'hello'".to_string(),
        );

        assert_eq!(snippet.id, "test");
        assert_eq!(snippet.title, "Test Snippet");
        assert_eq!(snippet.content, "echo 'hello'");
        assert!(snippet.enabled);
        assert!(snippet.keybinding.is_none());
        assert!(snippet.folder.is_none());
        assert!(snippet.variables.is_empty());
    }

    #[test]
    fn test_snippet_builder() {
        let snippet = SnippetConfig::new(
            "test".to_string(),
            "Test Snippet".to_string(),
            "echo 'hello'".to_string(),
        )
        .with_keybinding("Ctrl+Shift+T".to_string())
        .with_folder("Test".to_string())
        .with_variable("name".to_string(), "value".to_string());

        assert_eq!(snippet.keybinding, Some("Ctrl+Shift+T".to_string()));
        assert_eq!(snippet.folder, Some("Test".to_string()));
        assert_eq!(snippet.variables.get("name"), Some(&"value".to_string()));
    }

    #[test]
    fn test_builtin_variable_resolution() {
        // These should not panic
        let date = BuiltInVariable::Date.resolve();
        assert!(!date.is_empty());

        let time = BuiltInVariable::Time.resolve();
        assert!(!time.is_empty());

        let user = BuiltInVariable::User.resolve();
        assert!(!user.is_empty());

        let path = BuiltInVariable::Path.resolve();
        assert!(!path.is_empty());
    }

    #[test]
    fn test_builtin_variable_parse() {
        assert_eq!(BuiltInVariable::parse("date"), Some(BuiltInVariable::Date));
        assert_eq!(BuiltInVariable::parse("time"), Some(BuiltInVariable::Time));
        assert_eq!(BuiltInVariable::parse("unknown"), None);
    }

    #[test]
    fn test_custom_action_id() {
        let action = CustomActionConfig::ShellCommand {
            id: "test-action".to_string(),
            title: "Test Action".to_string(),
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            notify_on_success: false,
            timeout_secs: 30,
            capture_output: false,
            keybinding: None,
            prefix_char: Some('G'),
            keybinding_enabled: true,
            description: None,
        };

        assert_eq!(action.id(), "test-action");
        assert_eq!(action.title(), "Test Action");
        assert!(action.is_shell_command());
        assert!(!action.is_new_tab());
        assert!(!action.is_insert_text());
        assert!(!action.is_key_sequence());
        assert!(!action.is_split_pane());
        assert_eq!(action.prefix_char(), Some('G'));
        assert_eq!(action.normalized_prefix_char(), Some('g'));
    }

    #[test]
    fn test_split_pane_action() {
        let action = CustomActionConfig::SplitPane {
            id: "split-htop".to_string(),
            title: "Split and run htop".to_string(),
            direction: ActionSplitDirection::Vertical,
            command: Some("htop".to_string()),
            command_is_direct: true,
            focus_new_pane: true,
            delay_ms: 200,
            split_percent: 66,
            keybinding: Some("Ctrl+Shift+H".to_string()),
            prefix_char: None,
            keybinding_enabled: true,
            description: None,
        };

        assert_eq!(action.id(), "split-htop");
        assert_eq!(action.title(), "Split and run htop");
        assert!(action.is_split_pane());
        assert!(!action.is_shell_command());
        assert_eq!(action.keybinding(), Some("Ctrl+Shift+H"));
    }

    #[test]
    fn test_new_tab_action() {
        let action = CustomActionConfig::NewTab {
            id: "new-tab-lazygit".to_string(),
            title: "Open lazygit tab".to_string(),
            command: Some("lazygit".to_string()),
            keybinding: Some("Ctrl+Shift+G".to_string()),
            prefix_char: Some('g'),
            keybinding_enabled: true,
            description: None,
        };

        assert_eq!(action.id(), "new-tab-lazygit");
        assert_eq!(action.title(), "Open lazygit tab");
        assert!(action.is_new_tab());
        assert!(!action.is_shell_command());
        assert!(!action.is_split_pane());
        assert_eq!(action.keybinding(), Some("Ctrl+Shift+G"));
        assert_eq!(action.normalized_prefix_char(), Some('g'));
    }

    #[test]
    fn test_sequence_action_round_trip() {
        use crate::snippets::{SequenceStep, SequenceStepBehavior};
        let action = CustomActionConfig::Sequence {
            id: "build-and-test".to_string(),
            title: "Build and Test".to_string(),
            keybinding: None,
            prefix_char: None,
            keybinding_enabled: true,
            description: None,
            steps: vec![
                SequenceStep {
                    action_id: "build".to_string(),
                    delay_ms: 0,
                    on_failure: SequenceStepBehavior::Abort,
                },
                SequenceStep {
                    action_id: "test".to_string(),
                    delay_ms: 500,
                    on_failure: SequenceStepBehavior::Continue,
                },
            ],
        };
        let yaml = serde_yaml_ng::to_string(&action).unwrap();
        let roundtrip: CustomActionConfig = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(action, roundtrip);
        assert_eq!(action.id(), "build-and-test");
        assert_eq!(action.title(), "Build and Test");
    }

    #[test]
    fn test_condition_action_round_trip() {
        use crate::snippets::ConditionCheck;
        let action = CustomActionConfig::Condition {
            id: "check-main".to_string(),
            title: "Check Main Branch".to_string(),
            keybinding: None,
            prefix_char: None,
            keybinding_enabled: true,
            description: None,
            check: ConditionCheck::GitBranch {
                pattern: "main".to_string(),
            },
            on_true_id: Some("deploy".to_string()),
            on_false_id: None,
        };
        let yaml = serde_yaml_ng::to_string(&action).unwrap();
        let roundtrip: CustomActionConfig = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(action, roundtrip);
        assert_eq!(action.id(), "check-main");
    }

    #[test]
    fn test_repeat_action_round_trip() {
        let action = CustomActionConfig::Repeat {
            id: "retry-deploy".to_string(),
            title: "Retry Deploy".to_string(),
            keybinding: None,
            prefix_char: None,
            keybinding_enabled: true,
            description: None,
            action_id: "deploy".to_string(),
            count: 3,
            delay_ms: 1000,
            stop_on_success: true,
            stop_on_failure: false,
        };
        let yaml = serde_yaml_ng::to_string(&action).unwrap();
        let roundtrip: CustomActionConfig = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(action, roundtrip);
        assert_eq!(action.id(), "retry-deploy");
    }

    #[test]
    fn test_shell_command_capture_output_default_false() {
        let yaml = r#"
type: shell_command
id: test
title: Test
command: echo
"#;
        let action: CustomActionConfig = serde_yaml_ng::from_str(yaml).unwrap();
        if let CustomActionConfig::ShellCommand { capture_output, .. } = action {
            assert!(!capture_output);
        } else {
            panic!("expected ShellCommand");
        }
    }
}

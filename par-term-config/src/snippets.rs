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

/// A custom action that can be triggered via keybinding.
///
/// Actions can execute shell commands, insert text, or simulate key sequences.
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

        /// Optional keyboard shortcut to trigger the action (e.g., "Ctrl+Shift+R")
        #[serde(default)]
        keybinding: Option<String>,

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

        /// Whether the keybinding is enabled (default: true)
        #[serde(default = "crate::defaults::bool_true")]
        keybinding_enabled: bool,

        /// Optional description
        #[serde(default)]
        description: Option<String>,
    },
}

impl CustomActionConfig {
    /// Get the action ID (for keybinding reference).
    pub fn id(&self) -> &str {
        match self {
            Self::ShellCommand { id, .. } => id,
            Self::InsertText { id, .. } => id,
            Self::KeySequence { id, .. } => id,
        }
    }

    /// Get the action title (for UI display).
    pub fn title(&self) -> &str {
        match self {
            Self::ShellCommand { title, .. } => title,
            Self::InsertText { title, .. } => title,
            Self::KeySequence { title, .. } => title,
        }
    }

    /// Get the optional keybinding for this action.
    pub fn keybinding(&self) -> Option<&str> {
        match self {
            Self::ShellCommand { keybinding, .. }
            | Self::InsertText { keybinding, .. }
            | Self::KeySequence { keybinding, .. } => keybinding.as_deref(),
        }
    }

    /// Check if the keybinding is enabled.
    pub fn keybinding_enabled(&self) -> bool {
        match self {
            Self::ShellCommand {
                keybinding_enabled, ..
            }
            | Self::InsertText {
                keybinding_enabled, ..
            }
            | Self::KeySequence {
                keybinding_enabled, ..
            } => *keybinding_enabled,
        }
    }

    /// Set the keybinding for this action.
    pub fn set_keybinding(&mut self, kb: Option<String>) {
        match self {
            Self::ShellCommand { keybinding, .. }
            | Self::InsertText { keybinding, .. }
            | Self::KeySequence { keybinding, .. } => *keybinding = kb,
        }
    }

    /// Set whether the keybinding is enabled.
    pub fn set_keybinding_enabled(&mut self, enabled: bool) {
        match self {
            Self::ShellCommand {
                keybinding_enabled, ..
            }
            | Self::InsertText {
                keybinding_enabled, ..
            }
            | Self::KeySequence {
                keybinding_enabled, ..
            } => *keybinding_enabled = enabled,
        }
    }

    /// Check if this is a shell command action.
    pub fn is_shell_command(&self) -> bool {
        matches!(self, Self::ShellCommand { .. })
    }

    /// Check if this is an insert text action.
    pub fn is_insert_text(&self) -> bool {
        matches!(self, Self::InsertText { .. })
    }

    /// Check if this is a key sequence action.
    pub fn is_key_sequence(&self) -> bool {
        matches!(self, Self::KeySequence { .. })
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
            keybinding: None,
            keybinding_enabled: true,
            description: None,
        };

        assert_eq!(action.id(), "test-action");
        assert_eq!(action.title(), "Test Action");
        assert!(action.is_shell_command());
        assert!(!action.is_insert_text());
        assert!(!action.is_key_sequence());
    }
}

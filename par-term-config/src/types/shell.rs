//! Shell detection and shell behavior configuration types.

use serde::{Deserialize, Serialize};

// ============================================================================
// Shell Types
// ============================================================================

/// Detected shell type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    #[default]
    Unknown,
}

impl ShellType {
    /// Classify a shell path string into a `ShellType`.
    fn from_path(path: &str) -> Self {
        if path.contains("zsh") {
            Self::Zsh
        } else if path.contains("bash") {
            Self::Bash
        } else if path.contains("fish") {
            Self::Fish
        } else {
            Self::Unknown
        }
    }

    /// Detect shell type using multiple strategies.
    ///
    /// 1. `$SHELL` environment variable (works in terminals).
    /// 2. macOS Directory Services (`dscl`) — works for app-bundle launches.
    /// 3. `/etc/passwd` entry for the current user (Linux / older macOS).
    pub fn detect() -> Self {
        // 1. $SHELL env var
        if let Ok(shell) = std::env::var("SHELL") {
            let t = Self::from_path(&shell);
            if t != Self::Unknown {
                return t;
            }
        }

        // 2. macOS: query Directory Services for the login shell
        #[cfg(target_os = "macos")]
        {
            if let Some(t) = Self::detect_via_dscl() {
                return t;
            }
        }

        // 3. Parse /etc/passwd for the current user's shell
        #[cfg(unix)]
        {
            if let Some(t) = Self::detect_from_passwd() {
                return t;
            }
        }

        Self::Unknown
    }

    /// macOS: run `dscl . -read /Users/<user> UserShell` to get the login shell.
    #[cfg(target_os = "macos")]
    fn detect_via_dscl() -> Option<Self> {
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .ok()?;
        let output = std::process::Command::new("dscl")
            .args([".", "-read", &format!("/Users/{}", user), "UserShell"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        // Output looks like: "UserShell: /bin/zsh"
        let shell_path = text.split_whitespace().last()?;
        let t = Self::from_path(shell_path);
        if t != Self::Unknown { Some(t) } else { None }
    }

    /// Unix: parse `/etc/passwd` for the current user's configured shell.
    #[cfg(unix)]
    fn detect_from_passwd() -> Option<Self> {
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .ok()?;
        let contents = std::fs::read_to_string("/etc/passwd").ok()?;
        for line in contents.lines() {
            let parts: Vec<&str> = line.splitn(7, ':').collect();
            if parts.len() == 7 && parts[0] == user {
                let t = Self::from_path(parts[6]);
                if t != Self::Unknown {
                    return Some(t);
                }
            }
        }
        None
    }

    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Bash => "Bash",
            Self::Zsh => "Zsh",
            Self::Fish => "Fish",
            Self::Unknown => "Unknown",
        }
    }

    /// File extension for integration script
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
            Self::Unknown => "sh",
        }
    }
}

/// Action to take when the shell process exits
///
/// Controls what happens when a shell session terminates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ShellExitAction {
    /// Close the tab/pane when shell exits (default)
    #[default]
    Close,
    /// Keep the pane open showing the terminated shell
    Keep,
    /// Immediately restart the shell
    RestartImmediately,
    /// Show a prompt message and wait for Enter before restarting
    RestartWithPrompt,
    /// Restart the shell after a 1 second delay
    RestartAfterDelay,
}

impl ShellExitAction {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Close => "Close tab/pane",
            Self::Keep => "Keep open",
            Self::RestartImmediately => "Restart immediately",
            Self::RestartWithPrompt => "Restart with prompt",
            Self::RestartAfterDelay => "Restart after 1s delay",
        }
    }

    /// All available actions for UI iteration
    pub fn all() -> &'static [ShellExitAction] {
        &[
            ShellExitAction::Close,
            ShellExitAction::Keep,
            ShellExitAction::RestartImmediately,
            ShellExitAction::RestartWithPrompt,
            ShellExitAction::RestartAfterDelay,
        ]
    }

    /// Returns true if this action involves restarting the shell
    pub fn is_restart(&self) -> bool {
        matches!(
            self,
            Self::RestartImmediately | Self::RestartWithPrompt | Self::RestartAfterDelay
        )
    }
}

/// Startup directory mode
///
/// Controls where the terminal starts its working directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StartupDirectoryMode {
    /// Start in the user's home directory (default)
    #[default]
    Home,
    /// Remember and restore the last working directory from the previous session
    Previous,
    /// Start in a user-specified custom path
    Custom,
}

impl StartupDirectoryMode {
    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Home => "Home Directory",
            Self::Previous => "Previous Session",
            Self::Custom => "Custom Directory",
        }
    }

    /// All available modes for UI iteration
    pub fn all() -> &'static [StartupDirectoryMode] {
        &[
            StartupDirectoryMode::Home,
            StartupDirectoryMode::Previous,
            StartupDirectoryMode::Custom,
        ]
    }
}

//! Shell launch and lifecycle settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use crate::types::{ShellExitAction, StartupDirectoryMode};
use serde::{Deserialize, Serialize};

/// Which shell runs, where it starts, what it is sent, and what happens when it exits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellConfig {
    /// Action to take when the shell process exits
    /// Supports: close, keep, restart_immediately, restart_with_prompt, restart_after_delay
    /// For backward compatibility, also accepts boolean values (true=close, false=keep)
    #[serde(
        default,
        deserialize_with = "super::deserialize_shell_exit_action",
        alias = "exit_on_shell_exit",
        alias = "close_on_shell_exit"
    )]
    pub shell_exit_action: ShellExitAction,

    /// Custom shell command (defaults to system shell if not specified)
    #[serde(default)]
    pub custom_shell: Option<String>,

    /// Arguments to pass to the shell
    #[serde(default)]
    pub shell_args: Option<Vec<String>>,

    /// Working directory for the shell (legacy, use startup_directory_mode instead)
    /// When set, overrides startup_directory_mode for backward compatibility
    #[serde(default)]
    pub working_directory: Option<String>,

    /// Startup directory mode: controls where new sessions start
    /// - home: Start in user's home directory (default)
    /// - previous: Remember and restore last working directory from previous session
    /// - custom: Start in the directory specified by startup_directory
    #[serde(default)]
    pub startup_directory_mode: StartupDirectoryMode,

    /// Custom startup directory (used when startup_directory_mode is "custom")
    /// Supports ~ for home directory expansion
    #[serde(default)]
    pub startup_directory: Option<String>,

    /// Last working directory from previous session (auto-managed)
    /// Used when startup_directory_mode is "previous"
    #[serde(default)]
    pub last_working_directory: Option<String>,

    /// Environment variables to set for the shell
    #[serde(default)]
    pub shell_env: Option<std::collections::HashMap<String, String>>,

    /// Whether to spawn the shell as a login shell (passes -l flag)
    /// This is important on macOS to properly initialize PATH from Homebrew, /etc/paths.d, etc.
    /// Default: true
    #[serde(default = "crate::defaults::login_shell")]
    pub login_shell: bool,

    /// Text to send automatically when a terminal session starts
    /// Supports escape sequences: \n (newline), \r (carriage return), \t (tab), \xHH (hex), \e (ESC)
    #[serde(default = "crate::defaults::initial_text")]
    pub initial_text: String,

    /// Delay in milliseconds before sending the initial text (to allow shell to be ready)
    #[serde(default = "crate::defaults::initial_text_delay_ms")]
    pub initial_text_delay_ms: u64,

    /// Whether to append a newline after sending the initial text
    #[serde(default = "crate::defaults::initial_text_send_newline")]
    pub initial_text_send_newline: bool,

    /// Answerback string sent in response to ENQ (0x05) control character
    /// This is a legacy terminal feature used for terminal identification.
    /// Default: empty (disabled) for security
    /// Common values: "par-term", "vt100", or custom identification
    /// Security note: Setting this may expose terminal identification to applications
    #[serde(default = "crate::defaults::answerback_string")]
    pub answerback_string: String,

    /// Show confirmation dialog before quitting the application
    /// When enabled, closing the window will show a confirmation dialog
    /// if there are any open terminal sessions.
    /// Default: false (close immediately without confirmation)
    #[serde(default = "crate::defaults::bool_false")]
    pub prompt_on_quit: bool,

    /// Show confirmation dialog before closing a tab with running jobs
    /// When enabled, closing a tab that has a running command will show a confirmation dialog.
    /// Default: false (close immediately without confirmation)
    #[serde(default = "crate::defaults::bool_false")]
    pub confirm_close_running_jobs: bool,

    /// List of job/process names to ignore when checking for running jobs
    /// These jobs will not trigger a close confirmation dialog.
    /// Common examples: "bash", "zsh", "fish", "cat", "less", "man", "sleep"
    /// Default: common shell names that shouldn't block tab close
    #[serde(default = "crate::defaults::jobs_to_ignore")]
    pub jobs_to_ignore: Vec<String>,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            shell_exit_action: ShellExitAction::default(),
            custom_shell: None,
            shell_args: None,
            working_directory: None,
            startup_directory_mode: StartupDirectoryMode::default(),
            startup_directory: None,
            last_working_directory: None,
            shell_env: None,
            login_shell: crate::defaults::login_shell(),
            initial_text: crate::defaults::initial_text(),
            initial_text_delay_ms: crate::defaults::initial_text_delay_ms(),
            initial_text_send_newline: crate::defaults::initial_text_send_newline(),
            answerback_string: crate::defaults::answerback_string(),
            prompt_on_quit: crate::defaults::bool_false(),
            confirm_close_running_jobs: crate::defaults::bool_false(),
            jobs_to_ignore: crate::defaults::jobs_to_ignore(),
        }
    }
}

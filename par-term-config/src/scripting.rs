//! Configuration types for external observer scripts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::automation::RestartPolicy;

/// Configuration for an external observer script that receives terminal events via JSON protocol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScriptConfig {
    /// Human-readable name for this script
    pub name: String,

    /// Whether this script is enabled (default: true)
    #[serde(default = "crate::defaults::bool_true")]
    pub enabled: bool,

    /// Path to the script executable
    pub script_path: String,

    /// Arguments to pass to the script
    #[serde(default)]
    pub args: Vec<String>,

    /// Whether to start this script automatically when a tab opens
    #[serde(default)]
    pub auto_start: bool,

    /// Policy for restarting the script when it exits
    #[serde(default)]
    pub restart_policy: RestartPolicy,

    /// Delay in milliseconds before restarting (when restart_policy is not Never)
    #[serde(default)]
    pub restart_delay_ms: u64,

    /// Event types to subscribe to (empty = all events)
    #[serde(default)]
    pub subscriptions: Vec<String>,

    /// Additional environment variables to set for the script process
    #[serde(default)]
    pub env_vars: HashMap<String, String>,

    /// Allow this script to inject text into the active PTY via `WriteText`.
    ///
    /// Defaults to `false`. Must be explicitly set to `true` to enable.
    /// When enabled, VT/ANSI escape sequences are stripped before writing
    /// and a rate limit is applied (see `write_text_rate_limit`).
    #[serde(default)]
    pub allow_write_text: bool,

    /// Allow this script to spawn external processes via `RunCommand`.
    ///
    /// Defaults to `false`. Must be explicitly set to `true` to enable.
    /// When enabled, the command is checked against the denylist and a
    /// rate limit is applied (see `run_command_rate_limit`).
    #[serde(default)]
    pub allow_run_command: bool,

    /// Allow this script to modify runtime configuration via `ChangeConfig`.
    ///
    /// Defaults to `false`. Must be explicitly set to `true` to enable.
    /// Only keys in the runtime allowlist may be changed.
    #[serde(default)]
    pub allow_change_config: bool,

    /// Maximum `WriteText` writes per second (0 = use default of 10/s).
    #[serde(default)]
    pub write_text_rate_limit: u32,

    /// Maximum `RunCommand` executions per second (0 = use default of 1/s).
    #[serde(default)]
    pub run_command_rate_limit: u32,
}

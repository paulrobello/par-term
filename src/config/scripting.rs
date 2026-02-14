//! Configuration types for external observer scripts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::automation::RestartPolicy;

/// Configuration for an external observer script that receives terminal events via JSON protocol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScriptConfig {
    /// Human-readable name for this script
    pub name: String,

    /// Whether this script is enabled (default: true)
    #[serde(default = "super::defaults::bool_true")]
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
}

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

    /// Ask before each `WriteText` injection from this script.
    ///
    /// Defaults to `true`, mirroring `prompt_before_run` on triggers. Stripping
    /// escape sequences does not make an injection safe — the payload that runs
    /// a command is ordinary printable text plus a newline — so confirmation,
    /// not filtering, is the control on this path. Set to `false` to write
    /// immediately; the write is still sanitized and rate limited.
    #[serde(default = "crate::defaults::bool_true")]
    pub prompt_before_write_text: bool,

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

impl ScriptConfig {
    /// Whether this script should be started automatically when a tab is created.
    ///
    /// A script auto-starts only when it is both enabled and opted in via
    /// `auto_start`; `enabled: false` disables the script entirely, including
    /// auto-start, matching the manual start path in the Settings UI.
    pub fn should_auto_start(&self) -> bool {
        self.enabled && self.auto_start
    }
}

#[cfg(test)]
mod tests {
    use super::ScriptConfig;

    const MINIMAL: &str = "name: observer\nscript_path: /tmp/observer.py\n";

    fn parse(yaml: &str) -> ScriptConfig {
        serde_yaml_ng::from_str(yaml).expect("deserialize ScriptConfig")
    }

    #[test]
    fn write_text_confirmation_defaults_on_for_configs_that_predate_the_field() {
        let script = parse(MINIMAL);
        assert!(script.prompt_before_write_text);
        // The capability itself stays opt-out by default; the prompt only
        // matters once the user has granted it.
        assert!(!script.allow_write_text);
    }

    #[test]
    fn write_text_confirmation_can_be_switched_off_explicitly() {
        let script = parse(&format!(
            "{MINIMAL}allow_write_text: true\nprompt_before_write_text: false\n"
        ));
        assert!(script.allow_write_text);
        assert!(!script.prompt_before_write_text);
    }

    #[test]
    fn write_text_confirmation_survives_a_round_trip() {
        let mut script = parse(MINIMAL);
        script.prompt_before_write_text = false;
        let yaml = serde_yaml_ng::to_string(&script).expect("serialize");
        assert_eq!(parse(&yaml), script);
    }
}

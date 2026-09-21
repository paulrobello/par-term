//! [`AgentUsageConfig`]: agent usage panel settings.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Configuration for the agent-usage status-bar widget and popup panel.
///
/// The panel is a display over a records directory any collector can write
/// (see `docs/features/AGENT_USAGE.md`); these keys arm the subsystem and its
/// optional refresh command, they never identify collectors themselves.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentUsageConfig {
    /// Enable the agent-usage subsystem (directory watch + panel). The
    /// status-bar widget still self-hides when no records are displayable;
    /// this switch is the master arm for the whole feature.
    #[serde(default = "default_agent_usage_enabled")]
    pub agent_usage_enabled: bool,

    /// Optional command run through `sh -c` on the refresh interval and on
    /// manual refresh, expected to rewrite the records directory (omarchy's
    /// `omarchy-agent-usage-update` division of labor). Empty/absent means
    /// pure watch mode — par-term runs nothing and only reacts to file
    /// changes. par-term ships no collectors of its own.
    #[serde(default)]
    pub agent_usage_update_command: Option<String>,

    /// Seconds between refresh-driven rescans (and update-command runs, when
    /// configured). Clamped to a 30 s floor at the call site.
    #[serde(default = "default_agent_usage_refresh_interval_sec")]
    pub agent_usage_refresh_interval_sec: u64,

    /// Agent ids to hide from the widget and panel (the hide-list form of
    /// the design's default-true per-agent enable map).
    #[serde(default)]
    pub agent_usage_hidden_agents: Vec<String>,
}

fn default_agent_usage_enabled() -> bool {
    true
}

fn default_agent_usage_refresh_interval_sec() -> u64 {
    300
}

impl Default for AgentUsageConfig {
    fn default() -> Self {
        Self {
            agent_usage_enabled: default_agent_usage_enabled(),
            agent_usage_update_command: None,
            agent_usage_refresh_interval_sec: default_agent_usage_refresh_interval_sec(),
            agent_usage_hidden_agents: Vec::new(),
        }
    }
}

impl AgentUsageConfig {
    /// The hidden-agent set, as the usage store consumes it.
    pub fn hidden_set(&self) -> HashSet<String> {
        self.agent_usage_hidden_agents.iter().cloned().collect()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_pure_watch_mode() {
        let config = AgentUsageConfig::default();
        assert!(config.agent_usage_enabled);
        assert_eq!(config.agent_usage_update_command, None);
        assert_eq!(config.agent_usage_refresh_interval_sec, 300);
        assert!(config.agent_usage_hidden_agents.is_empty());
    }

    #[test]
    fn yaml_keys_parse_at_top_level() {
        // The sub-config is #[serde(flatten)]-ed into Config, so its keys sit
        // at the YAML top level; verify they parse with the exact spellings
        // the docs will publish.
        let yaml = r#"
agent_usage_enabled: false
agent_usage_update_command: "my-collector --update"
agent_usage_refresh_interval_sec: 60
agent_usage_hidden_agents:
  - claude
  - codex
"#;
        let parsed: AgentUsageConfig = serde_yaml_ng::from_str(yaml).expect("keys parse");
        assert!(!parsed.agent_usage_enabled);
        assert_eq!(
            parsed.agent_usage_update_command.as_deref(),
            Some("my-collector --update")
        );
        assert_eq!(parsed.agent_usage_refresh_interval_sec, 60);
        assert_eq!(parsed.agent_usage_hidden_agents, vec!["claude", "codex"]);
        assert!(parsed.hidden_set().contains("claude"));
    }
}

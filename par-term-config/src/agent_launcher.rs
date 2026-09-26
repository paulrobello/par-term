//! Configured launchable agents (the agent-launcher palette).
//!
//! Omarchy-F5 port: a user-authored list of CLI coding agents exposed as
//! "Launch <agent>" palette entries. Each entry names a command; an optional
//! `autonomy_args` string arms a second, explicitly labelled "(autonomous)"
//! palette row — autonomy is offered, never defaulted.

use serde::{Deserialize, Serialize};

/// One configured agent in the top-level `agents:` config list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentLaunchConfig {
    /// Stable slug used in `launch-agent:<id>` action names.
    pub id: String,
    /// Human-readable palette label.
    pub name: String,
    /// The command line typed into the new pane's shell.
    pub command: String,
    /// Extra arguments appended for the "(autonomous)" palette row.
    /// Empty/absent means no autonomous variant is offered.
    #[serde(default)]
    pub autonomy_args: String,
    /// Whether "Launch Default Agent" resolves here. Multiple `default:
    /// true` entries are legal; the first wins.
    #[serde(default)]
    pub default: bool,
}

impl AgentLaunchConfig {
    /// The command line for a launch: plain, or with autonomy args appended.
    pub fn command_line(&self, autonomous: bool) -> String {
        if autonomous && !self.autonomy_args.trim().is_empty() {
            format!("{} {}", self.command, self.autonomy_args)
        } else {
            self.command.clone()
        }
    }
}

/// The first entry marked `default`, if any.
pub fn default_agent(agents: &[AgentLaunchConfig]) -> Option<&AgentLaunchConfig> {
    agents.iter().find(|agent| agent.default)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(id: &str, autonomy: &str, default: bool) -> AgentLaunchConfig {
        AgentLaunchConfig {
            id: id.to_string(),
            name: id.to_string(),
            command: id.to_string(),
            autonomy_args: autonomy.to_string(),
            default,
        }
    }

    #[test]
    fn command_line_appends_autonomy_only_when_asked() {
        let a = agent("claude", "--permission-mode auto", false);
        assert_eq!(a.command_line(false), "claude");
        assert_eq!(a.command_line(true), "claude --permission-mode auto");
    }

    #[test]
    fn command_line_ignores_whitespace_autonomy_args() {
        let a = agent("codex", "   ", false);
        assert_eq!(a.command_line(true), "codex");
    }

    #[test]
    fn default_agent_picks_first_flagged() {
        let agents = vec![
            agent("claude", "", false),
            agent("codex", "", true),
            agent("omp", "", true),
        ];
        assert_eq!(default_agent(&agents).unwrap().id, "codex");
    }

    #[test]
    fn default_agent_none_when_unflagged() {
        assert!(default_agent(&[agent("claude", "", false)]).is_none());
    }

    #[test]
    fn yaml_round_trip() {
        let yaml = "- id: claude\n  name: Claude\n  command: claude\n  autonomy_args: --permission-mode auto\n  default: true\n";
        let agents: Vec<AgentLaunchConfig> = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(
            agents[0].command_line(true),
            "claude --permission-mode auto"
        );
        assert!(agents[0].default);
        // Absent optional fields default.
        let minimal: Vec<AgentLaunchConfig> =
            serde_yaml_ng::from_str("- id: sh\n  name: Shell\n  command: zsh\n").unwrap();
        assert_eq!(minimal[0].autonomy_args, "");
        assert!(!minimal[0].default);
    }
}

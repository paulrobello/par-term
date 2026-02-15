use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Agent configuration loaded from TOML.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    pub identity: String,
    pub name: String,
    pub short_name: String,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    #[serde(default = "default_type")]
    pub r#type: String,
    #[serde(default)]
    pub active: Option<bool>,
    pub run_command: HashMap<String, String>,
    #[serde(default)]
    pub actions: HashMap<String, HashMap<String, ActionConfig>>,
}

fn default_protocol() -> String {
    "acp".to_string()
}

fn default_type() -> String {
    "coding".to_string()
}

/// Configuration for an agent action.
#[derive(Debug, Clone, Deserialize)]
pub struct ActionConfig {
    pub command: Option<String>,
    pub description: Option<String>,
}

impl AgentConfig {
    /// Returns the run command for the current platform.
    /// Falls back to the wildcard `"*"` entry if the platform-specific key is absent.
    pub fn run_command_for_platform(&self) -> Option<&str> {
        let platform = if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "linux"
        };
        self.run_command
            .get(platform)
            .or_else(|| self.run_command.get("*"))
            .map(|s| s.as_str())
    }

    /// Returns whether this agent is active. Defaults to `true` if not specified.
    pub fn is_active(&self) -> bool {
        self.active.unwrap_or(true)
    }
}

/// Discover available agents from bundled and user config directories.
///
/// Bundled agents are loaded from the `agents/` directory next to the executable.
/// User agents are loaded from `<user_config_dir>/agents/` and override bundled
/// agents with the same identity. Inactive agents are filtered out.
pub fn discover_agents(user_config_dir: &Path) -> Vec<AgentConfig> {
    let mut agents = Vec::new();

    // Load bundled agents from next to the executable
    let bundled_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("agents")))
        .unwrap_or_else(|| PathBuf::from("agents"));
    load_agents_from_dir(&bundled_dir, &mut agents);

    // Load user agents (override bundled with same identity)
    let user_agents_dir = user_config_dir.join("agents");
    load_agents_from_dir(&user_agents_dir, &mut agents);

    agents.retain(|a| a.is_active());
    agents
}

/// Load all `.toml` agent config files from a directory.
/// If an agent with the same identity already exists in the list, it is replaced.
fn load_agents_from_dir(dir: &Path, agents: &mut Vec<AgentConfig>) {
    if !dir.exists() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "toml") {
            match std::fs::read_to_string(&path) {
                Ok(content) => match toml::from_str::<AgentConfig>(&content) {
                    Ok(config) => {
                        // Remove any existing agent with the same identity (allows user override)
                        agents.retain(|a| a.identity != config.identity);
                        agents.push(config);
                    }
                    Err(e) => {
                        log::error!("Failed to parse agent config {}: {e}", path.display());
                    }
                },
                Err(e) => log::error!("Failed to read agent config {}: {e}", path.display()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_agent_toml() {
        let toml_str = r#"
identity = "claude.com"
name = "Claude Code"
short_name = "claude"
protocol = "acp"
type = "coding"

[run_command]
"*" = "claude-code-acp"
macos = "claude-code-acp"
"#;
        let config: AgentConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.identity, "claude.com");
        assert_eq!(config.name, "Claude Code");
        assert_eq!(config.short_name, "claude");
        assert_eq!(config.protocol, "acp");
        assert_eq!(config.r#type, "coding");
        assert!(config.is_active());
        assert!(config.run_command_for_platform().is_some());
    }

    #[test]
    fn test_inactive_agent() {
        let toml_str = r#"
identity = "test.agent"
name = "Test"
short_name = "test"
active = false

[run_command]
"*" = "test-agent"
"#;
        let config: AgentConfig = toml::from_str(toml_str).unwrap();
        assert!(!config.is_active());
    }

    #[test]
    fn test_default_protocol_and_type() {
        let toml_str = r#"
identity = "minimal.agent"
name = "Minimal"
short_name = "min"

[run_command]
"*" = "minimal-agent"
"#;
        let config: AgentConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.protocol, "acp");
        assert_eq!(config.r#type, "coding");
    }

    #[test]
    fn test_platform_fallback_to_wildcard() {
        let toml_str = r#"
identity = "wildcard.agent"
name = "Wildcard"
short_name = "wc"

[run_command]
"*" = "wildcard-cmd"
"#;
        let config: AgentConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.run_command_for_platform(), Some("wildcard-cmd"));
    }

    #[test]
    fn test_discover_agents_nonexistent_dir() {
        let dir = PathBuf::from("/tmp/par_term_test_nonexistent_agents_dir");
        let agents = discover_agents(&dir);
        assert!(agents.is_empty());
    }

    #[test]
    fn test_discover_agents_from_temp_dir() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let agents_dir = tmp_dir.path().join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        let toml_content = r#"
identity = "test.disco"
name = "Discovery Test"
short_name = "disco"

[run_command]
"*" = "disco-agent"
"#;
        std::fs::write(agents_dir.join("test.disco.toml"), toml_content).unwrap();

        let agents = discover_agents(tmp_dir.path());
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].identity, "test.disco");
        assert_eq!(agents[0].name, "Discovery Test");
    }

    #[test]
    fn test_discover_agents_filters_inactive() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let agents_dir = tmp_dir.path().join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        let active_toml = r#"
identity = "active.agent"
name = "Active"
short_name = "act"

[run_command]
"*" = "active-cmd"
"#;
        let inactive_toml = r#"
identity = "inactive.agent"
name = "Inactive"
short_name = "inact"
active = false

[run_command]
"*" = "inactive-cmd"
"#;
        std::fs::write(agents_dir.join("active.toml"), active_toml).unwrap();
        std::fs::write(agents_dir.join("inactive.toml"), inactive_toml).unwrap();

        let agents = discover_agents(tmp_dir.path());
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].identity, "active.agent");
    }
}

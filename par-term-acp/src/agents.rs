use std::collections::HashMap;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

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
    pub env: HashMap<String, String>,
    /// Optional command to install the ACP connector for this agent.
    #[serde(default)]
    pub install_command: Option<String>,
    #[serde(default)]
    pub actions: HashMap<String, HashMap<String, ActionConfig>>,
    /// Whether the ACP connector binary was found in PATH during discovery.
    /// Populated by [`discover_agents`], not deserialized.
    #[serde(skip)]
    pub connector_installed: bool,
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

    /// Check if the run command binary exists in `PATH` and update
    /// [`connector_installed`](Self::connector_installed).
    pub fn detect_connector(&mut self) {
        self.connector_installed = self
            .run_command_for_platform()
            .map(|cmd| {
                // Extract the first token (the binary name)
                let binary = cmd.split_whitespace().next().unwrap_or("");
                binary_in_path(binary)
            })
            .unwrap_or(false);
    }
}

/// Check whether a binary name exists in any directory on `PATH`.
fn binary_in_path(binary: &str) -> bool {
    resolve_binary_in_path(binary).is_some()
}

/// Resolve a binary name to its absolute path by searching `PATH`.
///
/// Returns `None` if the binary is not found or PATH is not set.
pub fn resolve_binary_in_path(binary: &str) -> Option<std::path::PathBuf> {
    resolve_binary_in_path_str(binary, &std::env::var("PATH").ok()?)
}

/// Resolve a binary name to its absolute path by searching the given PATH string.
pub fn resolve_binary_in_path_str(binary: &str, path_var: &str) -> Option<std::path::PathBuf> {
    if binary.is_empty() {
        return None;
    }
    // If it's already an absolute path, just check it exists.
    let path = std::path::Path::new(binary);
    if path.is_absolute() {
        return if path.is_file() {
            Some(path.to_path_buf())
        } else {
            None
        };
    }
    for dir in std::env::split_paths(path_var) {
        let candidate = dir.join(binary);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Known-good shell basenames that are accepted by [`resolve_shell_path`] and
/// [`connect`]-style callers.
///
/// This allowlist prevents an attacker-controlled `$SHELL` environment variable
/// from causing par-term to execute an arbitrary binary when probing for `PATH`.
///
/// Only the **basename** of the shell binary is checked (e.g. `zsh` from
/// `/usr/local/bin/zsh`). The full path is preserved for the spawn call so that
/// non-standard installation prefixes (e.g. Homebrew `/opt/homebrew/bin/zsh`)
/// still work, while names like `../../usr/bin/evil` are rejected.
const KNOWN_SHELLS: &[&str] = &[
    "sh", "bash", "zsh", "fish", "dash", "ksh", "tcsh", "csh", "elvish", "nu",
];

/// Validate that `shell_path` is an acceptable shell binary.
///
/// Returns `true` when the **basename** of `shell_path` is in [`KNOWN_SHELLS`].
/// The full path may be absolute or relative; only the last component is checked.
///
/// # Security
///
/// This is a defense-in-depth measure against a tampered `$SHELL` environment
/// variable. It does not verify that the binary at the path is actually a shell —
/// that would require executing it. The goal is to prevent obviously malicious
/// values (e.g. a path to a custom binary, or a shell name with embedded flags).
pub fn is_known_shell(shell_path: &str) -> bool {
    let basename = std::path::Path::new(shell_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    KNOWN_SHELLS.contains(&basename)
}

/// Get the full PATH from the user's login interactive shell.
///
/// This is necessary because app-bundle launches (Finder, Dock, Spotlight)
/// start with a minimal environment.  The user's shell profile (`.bashrc`,
/// `.zshrc`) often configures PATH inside an interactive-only guard
/// (`case $- in *i*) ...`), so a non-interactive login shell (`-lc`) won't
/// pick up tools installed via nvm, homebrew, etc.
///
/// We spawn `$SHELL -lic 'printf "%s" "$PATH"'` which is both login (`-l`)
/// and interactive (`-i`), causing all profile files to be sourced.  Because
/// stdio is piped (no tty), readline does not emit control sequences.
///
/// # Security
///
/// The value of `$SHELL` is validated against [`KNOWN_SHELLS`] before use.
/// If the value is absent, empty, or not in the allowlist, `/bin/sh` is used
/// as a safe fallback. This prevents a tampered environment variable from
/// causing an arbitrary binary to be executed.
pub fn resolve_shell_path() -> Option<String> {
    let raw_shell = std::env::var("SHELL").unwrap_or_default();
    let shell = if !raw_shell.is_empty() && is_known_shell(&raw_shell) {
        raw_shell
    } else {
        if !raw_shell.is_empty() {
            log::warn!(
                "resolve_shell_path: $SHELL={raw_shell:?} is not in the known-shells allowlist; \
                 falling back to /bin/sh"
            );
        }
        "/bin/sh".to_string()
    };
    let output = std::process::Command::new(&shell)
        .args(["-lic", r#"printf "%s" "$PATH""#])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Some(path);
        }
    }
    None
}

/// Discover available agents from bundled and user config directories.
///
/// Bundled agents are loaded from the `agents/` directory next to the executable.
/// User agents are loaded from `<user_config_dir>/agents/` and override bundled
/// agents with the same identity. Inactive agents are filtered out.
/// Default agent configurations embedded at compile time.
/// These ensure agents are always available regardless of
/// installation method or launch context (e.g., macOS app bundle).
const EMBEDDED_AGENTS: &[&str] = &[
    r#"
identity = "claude.com"
name = "Claude Code"
short_name = "claude"
protocol = "acp"
type = "coding"
install_command = "npm install -g @zed-industries/claude-agent-acp"

[run_command]
"*" = "claude-agent-acp"
"#,
    r#"
identity = "openai.com"
name = "Codex CLI"
short_name = "codex"
protocol = "acp"
type = "coding"
install_command = "npm install -g @zed-industries/codex-acp"

[run_command]
"*" = "npx @zed-industries/codex-acp"
"#,
    r#"
identity = "geminicli.com"
name = "Gemini CLI"
short_name = "gemini"
protocol = "acp"
type = "coding"

[run_command]
"*" = "gemini --experimental-acp"
"#,
    r#"
identity = "copilot.github.com"
name = "Copilot"
short_name = "copilot"
protocol = "acp"
type = "coding"

[run_command]
"*" = "copilot --acp"
"#,
    r#"
identity = "ampcode.com"
name = "Amp (AmpCode)"
short_name = "amp"
protocol = "acp"
type = "coding"

[run_command]
"*" = "npx -y amp-acp"
"#,
    r#"
identity = "augmentcode.com"
name = "Auggie (Augment Code)"
short_name = "auggie"
protocol = "acp"
type = "coding"

[run_command]
"*" = "auggie --acp"
"#,
    r#"
identity = "docker.com"
name = "Docker cagent"
short_name = "cagent"
protocol = "acp"
type = "coding"

[run_command]
"*" = "cagent acp"
"#,
    r#"
identity = "openhands.dev"
name = "OpenHands"
short_name = "openhands"
protocol = "acp"
type = "coding"

[run_command]
"*" = "openhands acp"
"#,
];

/// The set of built-in agent identities defined in [`EMBEDDED_AGENTS`].
///
/// Used by [`load_agents_from_dir`] to detect when a user-supplied TOML file
/// overrides a built-in identity and emit a security warning.
const BUILT_IN_IDENTITIES: &[&str] = &[
    "claude.com",
    "openai.com",
    "geminicli.com",
    "copilot.github.com",
    "ampcode.com",
    "augmentcode.com",
    "docker.com",
    "openhands.dev",
];

pub fn discover_agents(user_config_dir: &Path) -> Vec<AgentConfig> {
    let mut agents = Vec::new();

    // 1. Load embedded default agents (always available)
    for embedded in EMBEDDED_AGENTS {
        if let Ok(config) = toml::from_str::<AgentConfig>(embedded) {
            agents.push(config);
        }
    }

    // 2. Load bundled agents from next to the executable (installed app)
    let bundled_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("agents")));
    if let Some(ref dir) = bundled_dir {
        load_agents_from_dir(dir, &mut agents, false);
    }

    // 3. Load user agents (override bundled/embedded with same identity).
    //
    // SEC-003: User-config-dir agents are **trusted user code** — they are
    // loaded from `<user_config_dir>/agents/` and executed with the same
    // privileges as par-term itself. No cryptographic integrity verification
    // is performed. A warning is emitted when a user TOML overrides a
    // built-in identity so that the change is visible in logs.
    let user_agents_dir = user_config_dir.join("agents");
    load_agents_from_dir(&user_agents_dir, &mut agents, true);

    agents.retain(|a| a.is_active());

    // Detect which agents have their connector binary available in PATH.
    for agent in &mut agents {
        agent.detect_connector();
    }

    agents
}

/// Load all `.toml` agent config files from a directory.
/// If an agent with the same identity already exists in the list, it is replaced.
///
/// When `is_user_config` is `true`, a warning is logged whenever a loaded
/// agent replaces a built-in identity. This makes it visible in logs when
/// user-supplied TOML alters trusted built-in agent definitions (SEC-003).
fn load_agents_from_dir(dir: &Path, agents: &mut Vec<AgentConfig>, is_user_config: bool) {
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
                        // SEC-003: Warn when a user-config-dir agent overrides a built-in identity.
                        // User agents are trusted user code but the override should be visible in logs.
                        if is_user_config && BUILT_IN_IDENTITIES.contains(&config.identity.as_str())
                        {
                            log::warn!(
                                "ACP agent config '{}' overrides built-in identity '{}'.\n\
                                 User-config-dir agents are executed with par-term's privileges.\n\
                                 Verify that '{}' is a trusted file you created intentionally.",
                                path.display(),
                                config.identity,
                                path.display(),
                            );
                        }
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
"*" = "claude-agent-acp"
macos = "claude-agent-acp"
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
    fn test_all_embedded_agents_parse() {
        for (i, toml_str) in EMBEDDED_AGENTS.iter().enumerate() {
            let config = toml::from_str::<AgentConfig>(toml_str)
                .unwrap_or_else(|e| panic!("Embedded agent {i} failed to parse: {e}"));
            assert!(!config.identity.is_empty(), "Agent {i} has empty identity");
            assert!(!config.name.is_empty(), "Agent {i} has empty name");
            assert!(
                !config.short_name.is_empty(),
                "Agent {i} has empty short_name"
            );
            assert!(
                config.run_command_for_platform().is_some(),
                "Agent {} ({}) has no run command for this platform",
                i,
                config.identity
            );
        }
    }

    #[test]
    fn test_embedded_agents_include_known_identities() {
        let agents: Vec<AgentConfig> = EMBEDDED_AGENTS
            .iter()
            .map(|s| toml::from_str(s).unwrap())
            .collect();
        let identities: Vec<&str> = agents.iter().map(|a| a.identity.as_str()).collect();
        assert!(identities.contains(&"claude.com"), "Missing claude.com");
        assert!(
            identities.contains(&"openai.com"),
            "Missing openai.com (codex)"
        );
        assert!(
            identities.contains(&"geminicli.com"),
            "Missing geminicli.com (gemini)"
        );
    }

    #[test]
    fn test_discover_agents_nonexistent_dir() {
        let dir = PathBuf::from("/tmp/par_term_test_nonexistent_agents_dir");
        let agents = discover_agents(&dir);
        // May find agents from cwd or bundled dir; just verify no panic.
        // The nonexistent user config dir itself contributes nothing.
        for agent in &agents {
            assert!(agent.is_active());
        }
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
        let disco = agents.iter().find(|a| a.identity == "test.disco");
        assert!(
            disco.is_some(),
            "Expected test.disco agent to be discovered"
        );
        assert_eq!(disco.unwrap().name, "Discovery Test");
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
        assert!(
            agents.iter().any(|a| a.identity == "active.agent"),
            "Expected active.agent to be present"
        );
        assert!(
            !agents.iter().any(|a| a.identity == "inactive.agent"),
            "Expected inactive.agent to be filtered out"
        );
    }

    #[test]
    fn test_binary_in_path_finds_common_binary() {
        // "ls" should be available on all platforms
        assert!(binary_in_path("ls"));
    }

    #[test]
    fn test_binary_in_path_not_found() {
        assert!(!binary_in_path("nonexistent-binary-12345"));
    }

    #[test]
    fn test_binary_in_path_empty() {
        assert!(!binary_in_path(""));
    }

    #[test]
    fn test_detect_connector_for_known_binary() {
        let mut config: AgentConfig = toml::from_str(
            r#"
identity = "test.agent"
name = "Test"
short_name = "test"

[run_command]
"*" = "ls"
"#,
        )
        .unwrap();
        config.detect_connector();
        assert!(config.connector_installed);
    }

    #[test]
    fn test_detect_connector_for_unknown_binary() {
        let mut config: AgentConfig = toml::from_str(
            r#"
identity = "test.agent"
name = "Test"
short_name = "test"

[run_command]
"*" = "nonexistent-binary-12345"
"#,
        )
        .unwrap();
        config.detect_connector();
        assert!(!config.connector_installed);
    }

    #[test]
    fn test_detect_connector_extracts_first_token() {
        let mut config: AgentConfig = toml::from_str(
            r#"
identity = "test.agent"
name = "Test"
short_name = "test"

[run_command]
"*" = "ls --some-flag"
"#,
        )
        .unwrap();
        config.detect_connector();
        assert!(config.connector_installed);
    }
}

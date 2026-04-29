//! ACP agent connection and lifecycle methods for `WindowState`.

use super::WindowState;
use crate::config::{Config, CustomAcpAgentConfig};
use par_term_acp::{
    Agent, AgentConfig, AgentMessage, AgentStatus, ClientCapabilities, FsCapabilities, SafePaths,
    discover_agents,
};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Resolve the project root displayed for the connected ACP agent session.
fn resolve_agent_project_root(cwd: &Path) -> PathBuf {
    let start = if cwd.is_dir() {
        cwd.to_path_buf()
    } else {
        cwd.parent().unwrap_or(cwd).to_path_buf()
    };

    for dir in start.ancestors() {
        if dir.join(".git").exists() {
            return dir.to_path_buf();
        }
    }

    start
}

fn normalize_absolute_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
            Component::RootDir | Component::Prefix(_) => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn expand_agent_root(root: &str, cwd: &Path) -> Option<PathBuf> {
    let trimmed = root.trim();
    if trimmed.is_empty() {
        return None;
    }

    let expanded = if trimmed == "~" {
        dirs::home_dir()?
    } else if let Some(rest) = trimmed.strip_prefix("~/") {
        dirs::home_dir()?.join(rest)
    } else {
        PathBuf::from(trimmed)
    };

    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        cwd.join(expanded)
    };
    Some(normalize_absolute_path(&absolute))
}

fn resolve_extra_agent_roots(
    configured_roots: &[String],
    cwd: &Path,
    shader_dir: &Path,
) -> Vec<String> {
    let mut roots = Vec::new();
    for path in configured_roots
        .iter()
        .filter_map(|root| expand_agent_root(root, cwd))
        .chain(std::iter::once(normalize_absolute_path(shader_dir)))
    {
        let root = path.to_string_lossy().to_string();
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    roots
}

/// Reconstruct a merged list of ACP agent configs by combining discovered built-in agents
/// with user-defined custom agents from the config file.
pub(super) fn merge_custom_ai_inspector_agents(
    mut agents: Vec<AgentConfig>,
    custom_agents: &[CustomAcpAgentConfig],
) -> Vec<AgentConfig> {
    for custom in custom_agents {
        if custom.identity.trim().is_empty()
            || custom.short_name.trim().is_empty()
            || custom.name.trim().is_empty()
            || custom.run_command.is_empty()
        {
            log::warn!(
                "Skipping invalid custom ACP agent entry identity='{}' short_name='{}'",
                custom.identity,
                custom.short_name
            );
            continue;
        }

        let actions: std::collections::HashMap<
            String,
            std::collections::HashMap<String, par_term_acp::agents::ActionConfig>,
        > = custom
            .actions
            .iter()
            .map(|(action_name, variants)| {
                let mapped_variants = variants
                    .iter()
                    .map(|(variant_name, action)| {
                        (
                            variant_name.clone(),
                            par_term_acp::agents::ActionConfig {
                                command: action.command.clone(),
                                description: action.description.clone(),
                            },
                        )
                    })
                    .collect::<std::collections::HashMap<_, _>>();
                (action_name.clone(), mapped_variants)
            })
            .collect::<std::collections::HashMap<_, _>>();

        let mut env = custom.env.clone();
        if !env.contains_key("OLLAMA_CONTEXT_LENGTH")
            && let Some(ctx) = custom.ollama_context_length
            && ctx > 0
        {
            env.insert("OLLAMA_CONTEXT_LENGTH".to_string(), ctx.to_string());
        }

        let mut custom_agent = AgentConfig {
            identity: custom.identity.clone(),
            name: custom.name.clone(),
            short_name: custom.short_name.clone(),
            protocol: if custom.protocol.trim().is_empty() {
                "acp".to_string()
            } else {
                custom.protocol.clone()
            },
            r#type: if custom.r#type.trim().is_empty() {
                "coding".to_string()
            } else {
                custom.r#type.clone()
            },
            active: custom.active,
            run_command: custom.run_command.clone(),
            env,
            install_command: custom.install_command.clone(),
            actions,
            connector_installed: false,
        };

        custom_agent.detect_connector();
        agents.retain(|existing| existing.identity != custom_agent.identity);
        agents.push(custom_agent);
    }

    agents.retain(|agent| agent.is_active());
    if let Some(codex_index) = agents
        .iter()
        .position(|agent| agent.identity == "openai.com")
    {
        let codex = agents.remove(codex_index);
        agents.insert(0, codex);
    }
    agents
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn agent(identity: &str, name: &str) -> AgentConfig {
        AgentConfig {
            identity: identity.to_string(),
            name: name.to_string(),
            short_name: identity.to_string(),
            protocol: "acp".to_string(),
            r#type: "coding".to_string(),
            active: None,
            run_command: HashMap::from([("*".to_string(), "agent-acp".to_string())]),
            env: HashMap::new(),
            install_command: None,
            actions: HashMap::new(),
            connector_installed: false,
        }
    }

    #[test]
    fn merge_custom_ai_inspector_agents_puts_codex_first() {
        let agents = vec![
            agent("claude.com", "Claude Code"),
            agent("geminicli.com", "Gemini CLI"),
            agent("openai.com", "Codex CLI"),
        ];

        let merged = merge_custom_ai_inspector_agents(agents, &[]);

        let identities: Vec<&str> = merged.iter().map(|agent| agent.identity.as_str()).collect();
        assert_eq!(
            identities,
            vec!["openai.com", "claude.com", "geminicli.com"]
        );
    }

    #[test]
    fn resolve_agent_project_root_uses_nearest_git_marker() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo = temp.path().join("repo");
        let nested = repo.join("src/app");
        std::fs::create_dir_all(repo.join(".git")).expect("create .git");
        std::fs::create_dir_all(&nested).expect("create nested dir");

        assert_eq!(resolve_agent_project_root(&nested), repo);
    }

    #[test]
    fn resolve_agent_project_root_falls_back_to_cwd_without_git_marker() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project = temp.path().join("plain");
        std::fs::create_dir_all(&project).expect("create project dir");

        assert_eq!(resolve_agent_project_root(&project), project);
    }

    #[test]
    fn resolve_extra_agent_roots_always_includes_shader_dir_and_dedupes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cwd = temp.path().join("repo");
        let shared = temp.path().join("shared");
        let shaders = temp.path().join("shaders");
        std::fs::create_dir_all(&cwd).expect("create cwd");
        std::fs::create_dir_all(&shared).expect("create shared");
        std::fs::create_dir_all(&shaders).expect("create shaders");

        let roots = resolve_extra_agent_roots(
            &[
                shared.to_string_lossy().to_string(),
                shaders.to_string_lossy().to_string(),
            ],
            &cwd,
            &shaders,
        );

        assert_eq!(
            roots,
            vec![
                shared.to_string_lossy().to_string(),
                shaders.to_string_lossy().to_string()
            ]
        );
    }

    #[test]
    fn resolve_extra_agent_roots_resolves_relative_paths_against_session_cwd() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cwd = temp.path().join("repo");
        let shaders = temp.path().join("shaders");
        std::fs::create_dir_all(&cwd).expect("create cwd");
        std::fs::create_dir_all(&shaders).expect("create shaders");

        let roots = resolve_extra_agent_roots(&["../shared".to_string()], &cwd, &shaders);

        assert_eq!(roots[0], temp.path().join("shared").to_string_lossy());
        assert_eq!(roots[1], shaders.to_string_lossy());
    }
}

impl WindowState {
    /// Recompute available ACP agents from discovered + custom definitions.
    pub(crate) fn refresh_available_agents(&mut self) {
        let config_dir = dirs::config_dir().unwrap_or_default().join("par-term");
        let discovered_agents = discover_agents(&config_dir);
        self.agent_state.available_agents = merge_custom_ai_inspector_agents(
            discovered_agents,
            &self.config.ai_inspector.ai_inspector_custom_agents,
        );
    }

    /// Connect to an ACP agent by identity string.
    ///
    /// This extracts the agent connection logic so it can be called both from
    /// `InspectorAction::ConnectAgent` and from the auto-connect-on-open path.
    pub(crate) fn connect_agent(&mut self, identity: &str) {
        if let Some(agent_config) = self
            .agent_state
            .available_agents
            .iter()
            .find(|a| a.identity == identity)
        {
            self.agent_state.pending_agent_context_replay = self
                .overlay_ui
                .ai_inspector
                .chat
                .build_context_replay_prompt();
            self.overlay_ui.ai_inspector.connected_agent_name = Some(agent_config.name.clone());
            self.overlay_ui.ai_inspector.connected_agent_identity =
                Some(agent_config.identity.clone());

            // Clean up any previous agent before starting a new connection.
            if let Some(old_agent) = self.agent_state.agent.take() {
                let runtime = self.runtime.clone();
                runtime.spawn(async move {
                    let mut agent = old_agent.lock().await;
                    agent.disconnect().await;
                });
            }
            self.agent_state.agent_rx = None;
            self.agent_state.agent_tx = None;
            self.agent_state.agent_client = None;

            let (tx, rx) = mpsc::unbounded_channel();
            self.agent_state.agent_rx = Some(rx);
            self.agent_state.agent_tx = Some(tx.clone());
            let ui_tx = tx.clone();
            let safe_paths = SafePaths {
                config_dir: Config::config_dir(),
                shaders_dir: Config::shaders_dir(),
            };
            let mcp_server_bin =
                std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("par-term"));
            let agent = Agent::new(agent_config.clone(), tx, safe_paths, mcp_server_bin);
            agent.auto_approve.store(
                self.config.ai_inspector.ai_inspector_auto_approve,
                std::sync::atomic::Ordering::Relaxed,
            );
            let agent = Arc::new(tokio::sync::Mutex::new(agent));
            self.agent_state.agent = Some(agent.clone());

            // Determine CWD for the agent session
            let fallback_cwd = std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let cwd = if let Some(tab) = self.tab_manager.active_tab() {
                if let Ok(term) = tab.terminal.try_write() {
                    term.shell_integration_cwd()
                        .unwrap_or_else(|| fallback_cwd.clone())
                } else {
                    fallback_cwd.clone()
                }
            } else {
                fallback_cwd
            };
            self.overlay_ui.ai_inspector.connected_agent_cwd = Some(cwd.clone());
            self.overlay_ui.ai_inspector.connected_agent_project_root = Some(
                resolve_agent_project_root(Path::new(&cwd))
                    .to_string_lossy()
                    .to_string(),
            );
            let shader_dir = Config::shaders_dir();
            let _ = std::fs::create_dir_all(&shader_dir);
            let extra_roots = resolve_extra_agent_roots(
                &self.config.ai_inspector.ai_inspector_extra_agent_roots,
                Path::new(&cwd),
                &shader_dir,
            );

            let capabilities = ClientCapabilities {
                fs: FsCapabilities {
                    read_text_file: true,
                    write_text_file: true,
                    list_directory: true,
                    find: true,
                },
                terminal: self.config.ai_inspector.ai_inspector_agent_terminal_access,
                config: true,
            };

            let auto_approve = self.config.ai_inspector.ai_inspector_auto_approve;
            let runtime = self.runtime.clone();
            runtime.spawn(async move {
                let mut agent = agent.lock().await;
                if let Err(e) = agent.connect(&cwd, capabilities, &extra_roots).await {
                    log::error!("ACP: failed to connect to agent: {e}");
                    return;
                }
                if let Some(client) = &agent.client {
                    let _ = ui_tx.send(AgentMessage::ClientReady(Arc::clone(client)));
                }
                if auto_approve && let Err(e) = agent.set_mode("bypassPermissions").await {
                    log::error!("ACP: failed to set bypassPermissions mode: {e}");
                }
            });
        }
    }

    /// Auto-connect to the configured agent if auto-launch is enabled and no agent is connected.
    pub(crate) fn try_auto_connect_agent(&mut self) {
        if self.config.ai_inspector.ai_inspector_auto_launch
            && self.overlay_ui.ai_inspector.agent_status == AgentStatus::Disconnected
            && self.agent_state.agent.is_none()
        {
            let identity = self.config.ai_inspector.ai_inspector_agent.clone();
            if !identity.is_empty() {
                log::info!("ACP: auto-connecting to agent '{}'", identity);
                self.connect_agent(&identity);
            }
        }
    }
}

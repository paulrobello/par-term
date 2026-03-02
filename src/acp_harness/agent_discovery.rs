//! Agent discovery and merging utilities for the ACP harness binary.
//!
//! Handles loading agents from the filesystem and merging them with
//! any custom agent definitions supplied via `config.yaml`.

use std::collections::HashMap;

use par_term_acp::agents::ActionConfig;
use par_term_acp::{AgentConfig, discover_agents};
use par_term_config::{Config, CustomAcpAgentConfig};

/// Discover all available ACP agents and merge in any custom agents from `config`.
///
/// Custom agents override (replace) built-in agents with the same `identity`.
/// Inactive agents are filtered out before returning.
pub fn discover_and_merge_agents(config: &Config) -> Vec<AgentConfig> {
    let config_dir = Config::config_dir();
    let discovered = discover_agents(&config_dir);
    merge_custom_agents(discovered, &config.ai_inspector.ai_inspector_custom_agents)
}

/// Merge custom agent definitions from the config into a discovered agent list.
///
/// Invalid custom agents (empty identity/name/command) are silently skipped.
/// Agents with a matching `identity` in the existing list are replaced.
/// The returned list is sorted by name and contains only active agents.
pub fn merge_custom_agents(
    mut agents: Vec<AgentConfig>,
    custom_agents: &[CustomAcpAgentConfig],
) -> Vec<AgentConfig> {
    for custom in custom_agents {
        if custom.identity.trim().is_empty()
            || custom.short_name.trim().is_empty()
            || custom.name.trim().is_empty()
            || custom.run_command.is_empty()
        {
            continue;
        }

        let actions: HashMap<String, HashMap<String, ActionConfig>> = custom
            .actions
            .iter()
            .map(|(action_name, variants)| {
                let mapped = variants
                    .iter()
                    .map(|(variant_name, action)| {
                        (
                            variant_name.clone(),
                            ActionConfig {
                                command: action.command.clone(),
                                description: action.description.clone(),
                            },
                        )
                    })
                    .collect::<HashMap<_, _>>();
                (action_name.clone(), mapped)
            })
            .collect();

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
    agents.sort_by(|a, b| a.name.cmp(&b.name));
    agents
}

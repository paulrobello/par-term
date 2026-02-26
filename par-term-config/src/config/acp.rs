//! ACP (Agent Communication Protocol) agent configuration types.
//!
//! These types are used to configure custom ACP agents in `config.yaml`
//! under the `ai_inspector_custom_agents` key.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_acp_protocol() -> String {
    "acp".to_string()
}

fn default_acp_type() -> String {
    "coding".to_string()
}

/// Action metadata for a custom ACP agent entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomAcpAgentActionConfig {
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// User-defined ACP agent configuration sourced from `config.yaml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomAcpAgentConfig {
    pub identity: String,
    pub name: String,
    pub short_name: String,
    #[serde(default = "default_acp_protocol")]
    pub protocol: String,
    #[serde(default = "default_acp_type")]
    pub r#type: String,
    #[serde(default)]
    pub active: Option<bool>,
    pub run_command: HashMap<String, String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub ollama_context_length: Option<u32>,
    #[serde(default)]
    pub install_command: Option<String>,
    #[serde(default)]
    pub actions: HashMap<String, HashMap<String, CustomAcpAgentActionConfig>>,
}

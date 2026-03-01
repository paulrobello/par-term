//! Initialize handshake types for the ACP protocol.
//!
//! Covers the `initialize` request/response exchange between host and agent.

use serde::{Deserialize, Serialize};

/// Parameters for the `initialize` request sent from host to agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: u32,
    pub client_capabilities: ClientCapabilities,
    pub client_info: ClientInfo,
}

/// Capabilities the host advertises to the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    pub fs: FsCapabilities,
    pub terminal: bool,
    /// Whether the host supports the `config/update` RPC call.
    #[serde(default)]
    pub config: bool,
}

/// File-system capabilities exposed by the host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCapabilities {
    pub read_text_file: bool,
    pub write_text_file: bool,
    #[serde(default)]
    pub list_directory: bool,
    #[serde(default)]
    pub find: bool,
}

/// Identifying information about the host client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub name: String,
    pub title: String,
    pub version: String,
}

/// Result returned by the agent in response to `initialize`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_capabilities: Option<AgentCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_methods: Option<Vec<AuthMethod>>,
}

/// Capabilities the agent advertises back to the host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_session: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_capabilities: Option<PromptCapabilities>,
}

/// Content modalities the agent can handle in prompts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedded_content: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<bool>,
}

/// An authentication method the agent supports.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthMethod {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

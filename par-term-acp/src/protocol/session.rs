//! Session management types for the ACP protocol.
//!
//! Covers `session/new`, `session/load`, `session/prompt`, and `session/update`
//! messages including all session-update discriminated variants.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Parameters for the `session/new` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNewParams {
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Vec<Value>>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Parameters for the `session/load` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLoadParams {
    pub cwd: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Vec<Value>>,
}

/// Result returned after creating or loading a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResult {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modes: Option<ModesInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<ModelsInfo>,
}

/// Available interaction modes reported by the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModesInfo {
    pub available_modes: Vec<ModeEntry>,
    pub current_mode_id: String,
}

/// A single interaction mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeEntry {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Available models reported by the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsInfo {
    pub available_models: Vec<ModelEntry>,
    pub current_model_id: String,
}

/// A single model the agent can use.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntry {
    pub model_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Parameters for the `session/prompt` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPromptParams {
    pub session_id: String,
    pub prompt: Vec<super::content::ContentBlock>,
}

/// Result returned after a prompt completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPromptResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

/// Parameters for the `session/update` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpdateParams {
    pub session_id: String,
    pub update: Value,
}

/// A parsed session update. These are **not** serde-derived because the
/// `sessionUpdate` discriminator field requires manual dispatch.
#[derive(Debug, Clone)]
pub enum SessionUpdate {
    /// A chunk of the agent's response text.
    AgentMessageChunk { text: String },
    /// A chunk of the agent's internal reasoning.
    AgentThoughtChunk { text: String },
    /// A chunk echoing the user's message.
    UserMessageChunk { text: String },
    /// A new or updated tool call.
    ToolCall(ToolCallInfo),
    /// An incremental update to an existing tool call.
    ToolCallUpdate(ToolCallUpdateInfo),
    /// The agent's current plan.
    Plan(PlanInfo),
    /// Updated list of available slash commands.
    AvailableCommandsUpdate(Vec<AgentCommand>),
    /// The agent switched interaction mode.
    CurrentModeUpdate { mode_id: String },
    /// Unrecognized update type — preserved as raw JSON.
    Unknown(Value),
}

impl SessionUpdate {
    /// Parse a session update from its raw JSON [`Value`].
    ///
    /// The value is expected to have a `"sessionUpdate"` string field that
    /// acts as a type discriminator.
    pub fn from_value(value: &Value) -> Self {
        let update_type = value
            .get("sessionUpdate")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match update_type {
            "agent_message_chunk" => {
                let text = value
                    .get("content")
                    .and_then(|c| c.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                Self::AgentMessageChunk { text }
            }
            "agent_thought_chunk" => {
                let text = value
                    .get("content")
                    .and_then(|c| c.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                Self::AgentThoughtChunk { text }
            }
            "user_message_chunk" => {
                let text = value
                    .get("content")
                    .and_then(|c| c.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                Self::UserMessageChunk { text }
            }
            "tool_call" => {
                let tool_call_id = value
                    .get("toolCallId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let title = value
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let kind = value
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let status = value
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let content = value.get("content").cloned();
                Self::ToolCall(ToolCallInfo {
                    tool_call_id,
                    title,
                    kind,
                    status,
                    content,
                })
            }
            "tool_call_update" => {
                let tool_call_id = value
                    .get("toolCallId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let status = value
                    .get("status")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let title = value
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let content = value.get("content").cloned();
                Self::ToolCallUpdate(ToolCallUpdateInfo {
                    tool_call_id,
                    status,
                    title,
                    content,
                })
            }
            "plan" => {
                let entries = value
                    .get("entries")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(|entry| PlanEntry {
                                content: entry
                                    .get("content")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                status: entry
                                    .get("status")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Self::Plan(PlanInfo { entries })
            }
            "available_commands_update" => {
                let commands = value
                    .get("commands")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(|cmd| AgentCommand {
                                name: cmd
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                description: cmd
                                    .get("description")
                                    .and_then(|v| v.as_str())
                                    .map(String::from),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Self::AvailableCommandsUpdate(commands)
            }
            "current_mode_update" => {
                let mode_id = value
                    .get("modeId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Self::CurrentModeUpdate { mode_id }
            }
            _ => Self::Unknown(value.clone()),
        }
    }
}

/// Information about a tool call initiated by the agent.
#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    pub tool_call_id: String,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub content: Option<Value>,
}

/// An incremental update to an in-progress tool call.
#[derive(Debug, Clone)]
pub struct ToolCallUpdateInfo {
    pub tool_call_id: String,
    pub status: Option<String>,
    pub title: Option<String>,
    pub content: Option<Value>,
}

/// The agent's current execution plan.
#[derive(Debug, Clone)]
pub struct PlanInfo {
    pub entries: Vec<PlanEntry>,
}

/// A single step in the agent's plan.
#[derive(Debug, Clone)]
pub struct PlanEntry {
    pub content: String,
    pub status: String,
}

/// A slash-command or action the agent exposes.
#[derive(Debug, Clone)]
pub struct AgentCommand {
    pub name: String,
    pub description: Option<String>,
}

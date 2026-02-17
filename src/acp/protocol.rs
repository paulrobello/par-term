//! ACP (Agent Communication Protocol) message type definitions.
//!
//! These types model the JSON-RPC parameter and result objects exchanged
//! between the par-term host and an ACP-compatible agent (e.g. Claude Code).

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Initialize
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

/// Parameters for the `session/new` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNewParams {
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Vec<Value>>,
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

// ---------------------------------------------------------------------------
// Content blocks
// ---------------------------------------------------------------------------

/// A typed content block used in prompts and responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text content.
    Text { text: String },
    /// An embedded resource (file content, blob, etc.).
    Resource { resource: ResourceContent },
}

/// The payload of a `Resource` content block.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceContent {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Prompt
// ---------------------------------------------------------------------------

/// Parameters for the `session/prompt` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPromptParams {
    pub session_id: String,
    pub prompt: Vec<ContentBlock>,
}

/// Result returned after a prompt completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPromptResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Session updates (notifications from agent)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Permission
// ---------------------------------------------------------------------------

/// Parameters for the `session/request_permission` RPC call from agent to host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPermissionParams {
    pub session_id: String,
    pub tool_call: Value,
    pub options: Vec<PermissionOption>,
}

/// A single permission option the host can choose from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOption {
    pub option_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// The host's response to a permission request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPermissionResponse {
    pub outcome: PermissionOutcome,
}

/// The chosen outcome of a permission request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOutcome {
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option_id: Option<String>,
}

// ---------------------------------------------------------------------------
// File system operations
// ---------------------------------------------------------------------------

/// Parameters for the `fs/readTextFile` RPC call from agent to host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadParams {
    pub session_id: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
}

/// Parameters for the `fs/writeTextFile` RPC call from agent to host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsWriteParams {
    pub session_id: String,
    pub path: String,
    pub content: String,
}

/// Parameters for the `fs/listDirectory` RPC call from agent to host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsListDirectoryParams {
    pub session_id: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

/// Parameters for the `fs/find` RPC call from agent to host.
///
/// This is a par-term extension (not part of the core ACP spec) that provides
/// recursive glob-based file search, similar to Claude Code's built-in Glob tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsFindParams {
    pub session_id: String,
    pub path: String,
    pub pattern: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_update_parse_agent_message() {
        let value = serde_json::json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": "Hello from agent" }
        });
        match SessionUpdate::from_value(&value) {
            SessionUpdate::AgentMessageChunk { text } => assert_eq!(text, "Hello from agent"),
            other => panic!("Expected AgentMessageChunk, got {:?}", other),
        }
    }

    #[test]
    fn test_session_update_parse_tool_call() {
        let value = serde_json::json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "tc-1",
            "title": "Read file",
            "kind": "read",
            "status": "in_progress"
        });
        match SessionUpdate::from_value(&value) {
            SessionUpdate::ToolCall(info) => {
                assert_eq!(info.tool_call_id, "tc-1");
                assert_eq!(info.kind, "read");
            }
            other => panic!("Expected ToolCall, got {:?}", other),
        }
    }

    #[test]
    fn test_initialize_params_serialization() {
        let params = InitializeParams {
            protocol_version: 1,
            client_capabilities: ClientCapabilities {
                fs: FsCapabilities {
                    read_text_file: true,
                    write_text_file: false,
                    list_directory: false,
                    find: false,
                },
                terminal: false,
            },
            client_info: ClientInfo {
                name: "par-term".to_string(),
                title: "Par Term".to_string(),
                version: "0.16.0".to_string(),
            },
        };
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("protocolVersion"));
        assert!(json.contains("par-term"));
    }

    #[test]
    fn test_content_block_serialization() {
        let block = ContentBlock::Text {
            text: "Hello".to_string(),
        };
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"text"#));
    }

    #[test]
    fn test_session_update_parse_thought_chunk() {
        let value = serde_json::json!({
            "sessionUpdate": "agent_thought_chunk",
            "content": { "type": "text", "text": "Thinking..." }
        });
        match SessionUpdate::from_value(&value) {
            SessionUpdate::AgentThoughtChunk { text } => assert_eq!(text, "Thinking..."),
            other => panic!("Expected AgentThoughtChunk, got {:?}", other),
        }
    }

    #[test]
    fn test_session_update_parse_tool_call_update() {
        let value = serde_json::json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": "tc-2",
            "status": "completed",
            "title": "Write file"
        });
        match SessionUpdate::from_value(&value) {
            SessionUpdate::ToolCallUpdate(info) => {
                assert_eq!(info.tool_call_id, "tc-2");
                assert_eq!(info.status.as_deref(), Some("completed"));
                assert_eq!(info.title.as_deref(), Some("Write file"));
            }
            other => panic!("Expected ToolCallUpdate, got {:?}", other),
        }
    }

    #[test]
    fn test_session_update_parse_plan() {
        let value = serde_json::json!({
            "sessionUpdate": "plan",
            "entries": [
                { "content": "Step 1: Read file", "status": "completed" },
                { "content": "Step 2: Edit code", "status": "in_progress" }
            ]
        });
        match SessionUpdate::from_value(&value) {
            SessionUpdate::Plan(info) => {
                assert_eq!(info.entries.len(), 2);
                assert_eq!(info.entries[0].content, "Step 1: Read file");
                assert_eq!(info.entries[0].status, "completed");
                assert_eq!(info.entries[1].status, "in_progress");
            }
            other => panic!("Expected Plan, got {:?}", other),
        }
    }

    #[test]
    fn test_session_update_parse_available_commands() {
        let value = serde_json::json!({
            "sessionUpdate": "available_commands_update",
            "commands": [
                { "name": "/help", "description": "Show help" },
                { "name": "/clear" }
            ]
        });
        match SessionUpdate::from_value(&value) {
            SessionUpdate::AvailableCommandsUpdate(cmds) => {
                assert_eq!(cmds.len(), 2);
                assert_eq!(cmds[0].name, "/help");
                assert_eq!(cmds[0].description.as_deref(), Some("Show help"));
                assert_eq!(cmds[1].name, "/clear");
                assert!(cmds[1].description.is_none());
            }
            other => panic!("Expected AvailableCommandsUpdate, got {:?}", other),
        }
    }

    #[test]
    fn test_session_update_parse_current_mode() {
        let value = serde_json::json!({
            "sessionUpdate": "current_mode_update",
            "modeId": "agent"
        });
        match SessionUpdate::from_value(&value) {
            SessionUpdate::CurrentModeUpdate { mode_id } => assert_eq!(mode_id, "agent"),
            other => panic!("Expected CurrentModeUpdate, got {:?}", other),
        }
    }

    #[test]
    fn test_session_update_parse_unknown() {
        let value = serde_json::json!({
            "sessionUpdate": "some_future_type",
            "data": 42
        });
        match SessionUpdate::from_value(&value) {
            SessionUpdate::Unknown(v) => {
                assert_eq!(v.get("data").and_then(|d| d.as_u64()), Some(42));
            }
            other => panic!("Expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn test_initialize_result_deserialization() {
        let json = r#"{
            "protocolVersion": 1,
            "agentCapabilities": {
                "loadSession": true,
                "promptCapabilities": { "audio": false, "image": true }
            },
            "authMethods": [
                { "id": "oauth", "name": "OAuth 2.0" }
            ]
        }"#;
        let result: InitializeResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.protocol_version, 1);
        let caps = result.agent_capabilities.unwrap();
        assert_eq!(caps.load_session, Some(true));
        let prompt = caps.prompt_capabilities.unwrap();
        assert_eq!(prompt.image, Some(true));
        assert_eq!(prompt.audio, Some(false));
        assert!(prompt.embedded_content.is_none());
        let auth = result.auth_methods.unwrap();
        assert_eq!(auth.len(), 1);
        assert_eq!(auth[0].id, "oauth");
        assert!(auth[0].description.is_none());
    }

    #[test]
    fn test_session_result_deserialization() {
        let json = r#"{
            "sessionId": "sess-abc123",
            "modes": {
                "availableModes": [
                    { "id": "agent", "name": "Agent Mode" },
                    { "id": "plan", "name": "Plan Mode", "description": "Planning only" }
                ],
                "currentModeId": "agent"
            },
            "models": {
                "availableModels": [
                    { "modelId": "claude-4", "name": "Claude 4" }
                ],
                "currentModelId": "claude-4"
            }
        }"#;
        let result: SessionResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.session_id, "sess-abc123");
        let modes = result.modes.unwrap();
        assert_eq!(modes.available_modes.len(), 2);
        assert_eq!(
            modes.available_modes[1].description.as_deref(),
            Some("Planning only")
        );
        let models = result.models.unwrap();
        assert_eq!(models.available_models.len(), 1);
    }

    #[test]
    fn test_permission_roundtrip() {
        let params = RequestPermissionParams {
            session_id: "sess-1".to_string(),
            tool_call: serde_json::json!({"tool": "bash", "command": "ls"}),
            options: vec![
                PermissionOption {
                    option_id: "allow".to_string(),
                    name: "Allow".to_string(),
                    kind: Some("allow".to_string()),
                },
                PermissionOption {
                    option_id: "deny".to_string(),
                    name: "Deny".to_string(),
                    kind: None,
                },
            ],
        };
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("sessionId"));
        assert!(json.contains("optionId"));
        let deserialized: RequestPermissionParams = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.options.len(), 2);
        assert!(deserialized.options[1].kind.is_none());
    }

    #[test]
    fn test_content_block_resource_serialization() {
        let block = ContentBlock::Resource {
            resource: ResourceContent {
                uri: "file:///tmp/test.rs".to_string(),
                text: Some("fn main() {}".to_string()),
                blob: None,
                mime_type: Some("text/x-rust".to_string()),
            },
        };
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"resource"#));
        assert!(json.contains("file:///tmp/test.rs"));
        assert!(!json.contains("blob"));
    }

    #[test]
    fn test_fs_read_params_serialization() {
        let params = FsReadParams {
            session_id: "sess-1".to_string(),
            path: "/tmp/test.txt".to_string(),
            line: Some(10),
            limit: None,
        };
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("sessionId"));
        assert!(json.contains(r#""line":10"#));
        assert!(!json.contains("limit"));
    }

    #[test]
    fn test_fs_write_params_serialization() {
        let params = FsWriteParams {
            session_id: "sess-1".to_string(),
            path: "/tmp/test.txt".to_string(),
            content: "hello world".to_string(),
        };
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("sessionId"));
        assert!(json.contains("hello world"));
    }

    #[test]
    fn test_fs_list_directory_params_serialization() {
        let params = FsListDirectoryParams {
            session_id: "sess-1".to_string(),
            path: "/tmp".to_string(),
            pattern: Some("*.rs".to_string()),
        };
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("sessionId"));
        assert!(json.contains("*.rs"));
    }

    #[test]
    fn test_fs_capabilities_list_directory_default() {
        let json = r#"{"readTextFile": true, "writeTextFile": false}"#;
        let caps: FsCapabilities = serde_json::from_str(json).unwrap();
        assert!(caps.read_text_file);
        assert!(!caps.write_text_file);
        assert!(!caps.list_directory);
    }
}

//! ACP (Agent Communication Protocol) message type definitions.
//!
//! These types model the JSON-RPC parameter and result objects exchanged
//! between the par-term host and an ACP-compatible agent (e.g. Claude Code).
//!
//! The module is organized by domain:
//! - [`initialize`] — Handshake types (`initialize` request/response)
//! - [`session`] — Session lifecycle and update types
//! - [`content`] — Content block types used in prompts/responses
//! - [`permissions`] — Permission request/response types
//! - [`fs_ops`] — File system operation parameter types
//! - [`config_update`] — Config update RPC types

pub mod config_update;
pub mod content;
pub mod fs_ops;
pub mod initialize;
pub mod permissions;
pub mod session;

// Re-export all public types at the protocol module level so callers that
// use `crate::protocol::SomeType` continue to work without changes.
pub use config_update::ConfigUpdateParams;
pub use content::{ContentBlock, ResourceContent};
pub use fs_ops::{FsFindParams, FsListDirectoryParams, FsReadParams, FsWriteParams};
pub use initialize::{
    AgentCapabilities, AuthMethod, ClientCapabilities, ClientInfo, FsCapabilities,
    InitializeParams, InitializeResult, PromptCapabilities,
};
pub use permissions::{
    PermissionOption, PermissionOutcome, RequestPermissionParams, RequestPermissionResponse,
};
pub use session::{
    AgentCommand, ModeEntry, ModelEntry, ModelsInfo, ModesInfo, PlanEntry, PlanInfo,
    SessionLoadParams, SessionNewParams, SessionPromptParams, SessionPromptResult, SessionResult,
    SessionUpdate, SessionUpdateParams, ToolCallInfo, ToolCallUpdateInfo,
};

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
                config: false,
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

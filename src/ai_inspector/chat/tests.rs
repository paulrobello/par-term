//! Tests for the chat state, text utilities, and message types.

use par_term_acp::{SessionUpdate, ToolCallInfo, ToolCallUpdateInfo};

use super::state::ChatState;
use super::text_utils::{
    TextSegment, extract_code_block_commands, extract_inline_config_update, parse_text_segments,
};
use super::types::ChatMessage;

#[test]
fn test_new_chat_state() {
    let state = ChatState::new();
    assert!(state.messages.is_empty());
    assert!(state.input.is_empty());
    assert!(!state.streaming);
}

#[test]
fn test_default_chat_state() {
    let state = ChatState::default();
    assert!(state.messages.is_empty());
    assert!(!state.streaming);
}

#[test]
fn test_handle_agent_message_chunks() {
    let mut state = ChatState::new();
    state.handle_update(SessionUpdate::AgentMessageChunk {
        text: "Hello ".to_string(),
    });
    state.handle_update(SessionUpdate::AgentMessageChunk {
        text: "world".to_string(),
    });
    assert!(state.streaming);
    assert_eq!(state.streaming_text(), "Hello world");

    state.flush_agent_message();
    assert!(!state.streaming);
    assert_eq!(state.messages.len(), 1);
    match &state.messages[0] {
        ChatMessage::Agent(text) => assert_eq!(text, "Hello world"),
        _ => panic!("Expected Agent message"),
    }
}

#[test]
fn test_flush_empty_buffer_no_message() {
    let mut state = ChatState::new();
    state.flush_agent_message();
    assert!(state.messages.is_empty());
    assert!(!state.streaming);
}

#[test]
fn test_flush_trims_trailing_whitespace() {
    let mut state = ChatState::new();
    state.handle_update(SessionUpdate::AgentMessageChunk {
        text: "Hello  \n\n".to_string(),
    });
    state.flush_agent_message();
    match &state.messages[0] {
        ChatMessage::Agent(text) => assert_eq!(text, "Hello"),
        _ => panic!("Expected Agent message"),
    }
}

#[test]
fn test_handle_thinking_chunks() {
    let mut state = ChatState::new();
    state.handle_update(SessionUpdate::AgentThoughtChunk {
        text: "Let me ".to_string(),
    });
    state.handle_update(SessionUpdate::AgentThoughtChunk {
        text: "think...".to_string(),
    });
    assert_eq!(state.messages.len(), 1);
    match &state.messages[0] {
        ChatMessage::Thinking(text) => assert_eq!(text, "Let me think..."),
        _ => panic!("Expected Thinking message"),
    }
}

#[test]
fn test_thinking_not_coalesced_after_other_message() {
    let mut state = ChatState::new();
    state.handle_update(SessionUpdate::AgentThoughtChunk {
        text: "First thought".to_string(),
    });
    state.add_user_message("Interruption".to_string());
    state.handle_update(SessionUpdate::AgentThoughtChunk {
        text: "Second thought".to_string(),
    });
    assert_eq!(state.messages.len(), 3);
    match &state.messages[0] {
        ChatMessage::Thinking(text) => assert_eq!(text, "First thought"),
        _ => panic!("Expected Thinking"),
    }
    match &state.messages[2] {
        ChatMessage::Thinking(text) => assert_eq!(text, "Second thought"),
        _ => panic!("Expected Thinking"),
    }
}

#[test]
fn test_handle_tool_call_and_update() {
    let mut state = ChatState::new();
    state.handle_update(SessionUpdate::ToolCall(ToolCallInfo {
        tool_call_id: "tc-1".to_string(),
        title: "Reading file".to_string(),
        kind: "read".to_string(),
        status: "in_progress".to_string(),
        content: None,
    }));
    state.handle_update(SessionUpdate::ToolCallUpdate(ToolCallUpdateInfo {
        tool_call_id: "tc-1".to_string(),
        status: Some("completed".to_string()),
        title: None,
        content: None,
    }));
    assert_eq!(state.messages.len(), 1);
    match &state.messages[0] {
        ChatMessage::ToolCall { status, title, .. } => {
            assert_eq!(status, "completed");
            assert_eq!(title, "Reading file");
        }
        _ => panic!("Expected ToolCall"),
    }
}

#[test]
fn test_tool_call_update_matches_by_id() {
    let mut state = ChatState::new();
    state.handle_update(SessionUpdate::ToolCall(ToolCallInfo {
        tool_call_id: "tc-1".to_string(),
        title: "Read file A".to_string(),
        kind: "read".to_string(),
        status: "in_progress".to_string(),
        content: None,
    }));
    state.handle_update(SessionUpdate::ToolCall(ToolCallInfo {
        tool_call_id: "tc-2".to_string(),
        title: "Read file B".to_string(),
        kind: "read".to_string(),
        status: "in_progress".to_string(),
        content: None,
    }));

    // Update the first tool call, not the second.
    state.handle_update(SessionUpdate::ToolCallUpdate(ToolCallUpdateInfo {
        tool_call_id: "tc-1".to_string(),
        status: Some("completed".to_string()),
        title: Some("Read file A (done)".to_string()),
        content: None,
    }));

    match &state.messages[0] {
        ChatMessage::ToolCall {
            tool_call_id,
            status,
            title,
            ..
        } => {
            assert_eq!(tool_call_id, "tc-1");
            assert_eq!(status, "completed");
            assert_eq!(title, "Read file A (done)");
        }
        _ => panic!("Expected ToolCall"),
    }
    // Second tool call unchanged.
    match &state.messages[1] {
        ChatMessage::ToolCall {
            tool_call_id,
            status,
            title,
            ..
        } => {
            assert_eq!(tool_call_id, "tc-2");
            assert_eq!(status, "in_progress");
            assert_eq!(title, "Read file B");
        }
        _ => panic!("Expected ToolCall"),
    }
}

#[test]
fn test_tool_call_update_nonexistent_id_is_noop() {
    let mut state = ChatState::new();
    state.handle_update(SessionUpdate::ToolCall(ToolCallInfo {
        tool_call_id: "tc-1".to_string(),
        title: "Read file".to_string(),
        kind: "read".to_string(),
        status: "in_progress".to_string(),
        content: None,
    }));
    // Update for a different id should be a no-op.
    state.handle_update(SessionUpdate::ToolCallUpdate(ToolCallUpdateInfo {
        tool_call_id: "tc-999".to_string(),
        status: Some("completed".to_string()),
        title: None,
        content: None,
    }));
    match &state.messages[0] {
        ChatMessage::ToolCall { status, .. } => assert_eq!(status, "in_progress"),
        _ => panic!("Expected ToolCall"),
    }
}

#[test]
fn test_handle_unknown_update_is_noop() {
    let mut state = ChatState::new();
    state.handle_update(SessionUpdate::Unknown(serde_json::json!({"foo": "bar"})));
    assert!(state.messages.is_empty());
}

#[test]
fn test_add_messages() {
    let mut state = ChatState::new();
    state.add_user_message("test".to_string());
    state.add_system_message("system".to_string());
    state.add_command_suggestion("cargo test".to_string());
    state.add_auto_approved("read file".to_string());
    assert_eq!(state.messages.len(), 4);

    assert!(matches!(&state.messages[0], ChatMessage::User { text, .. } if text == "test"));
    assert!(matches!(&state.messages[1], ChatMessage::System(t) if t == "system"));
    assert!(
        matches!(&state.messages[2], ChatMessage::CommandSuggestion(t) if t == "cargo test")
    );
    assert!(matches!(&state.messages[3], ChatMessage::AutoApproved(t) if t == "read file"));
}

#[test]
fn test_extract_code_block_commands_bash() {
    let text = "Here's a command:\n```bash\ncargo test\ncargo build --release\n```\nDone.";
    let cmds = extract_code_block_commands(text);
    assert_eq!(cmds, vec!["cargo test", "cargo build --release"]);
}

#[test]
fn test_extract_code_block_commands_sh() {
    let text = "Try this:\n```sh\n$ echo hello\n$ ls -la\n```";
    let cmds = extract_code_block_commands(text);
    assert_eq!(cmds, vec!["echo hello", "ls -la"]);
}

#[test]
fn test_extract_code_block_commands_skips_comments_and_empty() {
    let text = "```bash\n# This is a comment\n\necho hello\n```";
    let cmds = extract_code_block_commands(text);
    assert_eq!(cmds, vec!["echo hello"]);
}

#[test]
fn test_extract_code_block_commands_ignores_non_shell() {
    let text = "```python\nprint('hello')\n```\n```bash\necho hi\n```";
    let cmds = extract_code_block_commands(text);
    assert_eq!(cmds, vec!["echo hi"]);
}

#[test]
fn test_extract_code_block_commands_with_metadata_tag() {
    let text = "```bash title=deploy\n./deploy.sh\n```";
    let cmds = extract_code_block_commands(text);
    assert_eq!(cmds, vec!["./deploy.sh"]);
}

#[test]
fn test_extract_code_block_commands_uppercase_lang() {
    let text = "```BASH\necho hi\n```";
    let cmds = extract_code_block_commands(text);
    assert_eq!(cmds, vec!["echo hi"]);
}

#[test]
fn test_extract_code_block_commands_line_continuation() {
    let text =
        "```bash\ncurl -H 'Auth: a' \\\n  --data 'x=1' \\\n  https://example.test\n```";
    let cmds = extract_code_block_commands(text);
    assert_eq!(
        cmds,
        vec!["curl -H 'Auth: a' --data 'x=1' https://example.test"]
    );
}

#[test]
fn test_extract_code_block_commands_no_blocks() {
    let text = "No code blocks here.";
    let cmds = extract_code_block_commands(text);
    assert!(cmds.is_empty());
}

#[test]
fn test_extract_code_block_commands_ignores_bare_blocks() {
    let text =
        "Description:\n```\nThis is just text, not a command.\n```\n```bash\ngit status\n```";
    let cmds = extract_code_block_commands(text);
    assert_eq!(cmds, vec!["git status"]);
}

#[test]
fn test_flush_extracts_command_suggestions() {
    let mut state = ChatState::new();
    state.handle_update(SessionUpdate::AgentMessageChunk {
        text: "Try this:\n```bash\ncargo test\n```".to_string(),
    });
    state.flush_agent_message();
    assert_eq!(state.messages.len(), 2);
    assert!(matches!(&state.messages[0], ChatMessage::Agent(_)));
    assert!(
        matches!(&state.messages[1], ChatMessage::CommandSuggestion(cmd) if cmd == "cargo test")
    );
}

#[test]
fn test_user_message_starts_pending() {
    let mut state = ChatState::new();
    state.add_user_message("hello".to_string());
    assert!(matches!(
        &state.messages[0],
        ChatMessage::User { pending: true, .. }
    ));
}

#[test]
fn test_mark_oldest_pending_sent() {
    let mut state = ChatState::new();
    state.add_user_message("first".to_string());
    state.add_user_message("second".to_string());
    state.mark_oldest_pending_sent();
    assert!(matches!(
        &state.messages[0],
        ChatMessage::User { pending: false, .. }
    ));
    assert!(matches!(
        &state.messages[1],
        ChatMessage::User { pending: true, .. }
    ));
}

#[test]
fn test_cancel_last_pending() {
    let mut state = ChatState::new();
    state.add_user_message("first".to_string());
    state.add_user_message("second".to_string());
    assert!(state.cancel_last_pending());
    assert_eq!(state.messages.len(), 1);
    assert!(matches!(
        &state.messages[0],
        ChatMessage::User { text, .. } if text == "first"
    ));
}

#[test]
fn test_cancel_last_pending_empty() {
    let mut state = ChatState::new();
    assert!(!state.cancel_last_pending());
}

#[test]
fn test_cancel_last_pending_none_pending() {
    let mut state = ChatState::new();
    state.add_user_message("sent".to_string());
    state.mark_oldest_pending_sent();
    assert!(!state.cancel_last_pending());
}

#[test]
fn test_parse_text_segments_plain_only() {
    let segments = parse_text_segments("Hello world\nSecond line");
    assert_eq!(
        segments,
        vec![TextSegment::Plain("Hello world\nSecond line".to_string())]
    );
}

#[test]
fn test_parse_text_segments_code_block() {
    let text = "Before\n```rust\nfn main() {}\n```\nAfter";
    let segments = parse_text_segments(text);
    assert_eq!(
        segments,
        vec![
            TextSegment::Plain("Before".to_string()),
            TextSegment::CodeBlock {
                lang: "rust".to_string(),
                code: "fn main() {}".to_string(),
            },
            TextSegment::Plain("After".to_string()),
        ]
    );
}

#[test]
fn test_parse_text_segments_multiple_blocks() {
    let text = "Text\n```bash\necho hi\n```\nMiddle\n```python\nprint(1)\n```\nEnd";
    let segments = parse_text_segments(text);
    assert_eq!(segments.len(), 5);
    assert!(matches!(&segments[0], TextSegment::Plain(t) if t == "Text"));
    assert!(
        matches!(&segments[1], TextSegment::CodeBlock { lang, code } if lang == "bash" && code == "echo hi")
    );
    assert!(matches!(&segments[2], TextSegment::Plain(t) if t == "Middle"));
    assert!(
        matches!(&segments[3], TextSegment::CodeBlock { lang, code } if lang == "python" && code == "print(1)")
    );
    assert!(matches!(&segments[4], TextSegment::Plain(t) if t == "End"));
}

#[test]
fn test_parse_text_segments_unclosed_block() {
    let text = "Before\n```rust\nfn main() {}";
    let segments = parse_text_segments(text);
    assert_eq!(
        segments,
        vec![
            TextSegment::Plain("Before".to_string()),
            TextSegment::CodeBlock {
                lang: "rust".to_string(),
                code: "fn main() {}".to_string(),
            },
        ]
    );
}

#[test]
fn test_parse_text_segments_bare_block() {
    let text = "Before\n```\nsome text\n```\nAfter";
    let segments = parse_text_segments(text);
    assert_eq!(
        segments,
        vec![
            TextSegment::Plain("Before".to_string()),
            TextSegment::CodeBlock {
                lang: String::new(),
                code: "some text".to_string(),
            },
            TextSegment::Plain("After".to_string()),
        ]
    );
}

#[test]
fn test_extract_inline_config_update_direct_object() {
    let text = r#"
<function=mcp__par-term-config__config_update>
<parameter=updates>
{"custom_shader":"rain.glsl","custom_shader_enabled":true}
</parameter>
</function>
</tool_call>
"#;
    let updates = extract_inline_config_update(text).expect("expected inline update");
    assert_eq!(
        updates.get("custom_shader"),
        Some(&serde_json::Value::String("rain.glsl".to_string()))
    );
    assert_eq!(
        updates.get("custom_shader_enabled"),
        Some(&serde_json::Value::Bool(true))
    );
}

#[test]
fn test_extract_inline_config_update_nested_updates() {
    let text = r#"
<function=mcp__par-term-config__config_update>
<parameter=updates>
{"updates":{"window_opacity":0.9}}
</parameter>
</function>
"#;
    let updates = extract_inline_config_update(text).expect("expected inline update");
    assert_eq!(
        updates.get("window_opacity"),
        Some(&serde_json::Value::from(0.9))
    );
}

#[test]
fn test_extract_inline_config_update_absent() {
    let text = "normal agent response";
    assert!(extract_inline_config_update(text).is_none());
}

#[test]
fn test_build_context_replay_prompt_includes_conversation() {
    let mut state = ChatState::new();
    state.add_user_message("sent".to_string());
    state.mark_oldest_pending_sent();
    state.add_user_message("queued".to_string());
    state
        .messages
        .push(ChatMessage::Agent("answer".to_string()));
    state.add_command_suggestion("echo answer".to_string());

    let prompt = state
        .build_context_replay_prompt()
        .expect("expected replay prompt");

    assert!(prompt.contains("[User]\nsent"));
    assert!(prompt.contains("[Assistant]\nanswer"));
    assert!(!prompt.contains("queued"));
    assert!(!prompt.contains("echo answer"));
}

#[test]
fn test_build_context_replay_prompt_none_when_empty() {
    let state = ChatState::new();
    assert!(state.build_context_replay_prompt().is_none());
}

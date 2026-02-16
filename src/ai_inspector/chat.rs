//! Chat data model for agent conversations in the AI Inspector.
//!
//! Provides [`ChatState`] for tracking the conversation between the user and
//! an ACP agent, including streaming message assembly, tool call tracking,
//! permission requests, and system messages.

use crate::acp::protocol::SessionUpdate;

/// A message in the chat history.
#[derive(Debug, Clone)]
pub enum ChatMessage {
    /// A message sent by the user.
    User(String),
    /// A completed response from the agent.
    Agent(String),
    /// The agent's internal reasoning / chain-of-thought.
    Thinking(String),
    /// A tool call initiated by the agent.
    ToolCall {
        tool_call_id: String,
        title: String,
        kind: String,
        status: String,
    },
    /// A command suggestion from the agent.
    CommandSuggestion(String),
    /// A permission request from the agent awaiting user action.
    Permission {
        request_id: u64,
        description: String,
        options: Vec<(String, String)>, // (option_id, label)
        resolved: bool,
    },
    /// A tool call that was automatically approved.
    AutoApproved(String),
    /// A system-level informational message.
    System(String),
}

/// Chat state for the agent conversation.
pub struct ChatState {
    /// All messages in the conversation history.
    pub messages: Vec<ChatMessage>,
    /// The current text input from the user (not yet sent).
    pub input: String,
    /// Whether the agent is currently streaming a response.
    pub streaming: bool,
    /// Buffer for assembling agent message chunks before flushing.
    agent_text_buffer: String,
}

impl ChatState {
    /// Create a new empty chat state.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            streaming: false,
            agent_text_buffer: String::new(),
        }
    }

    /// Process an incoming [`SessionUpdate`] from the agent, updating chat
    /// state accordingly.
    pub fn handle_update(&mut self, update: SessionUpdate) {
        match update {
            SessionUpdate::AgentMessageChunk { text } => {
                self.agent_text_buffer.push_str(&text);
                self.streaming = true;
            }
            SessionUpdate::AgentThoughtChunk { text } => {
                // Coalesce consecutive thought chunks into a single Thinking message.
                if let Some(ChatMessage::Thinking(existing)) = self.messages.last_mut() {
                    existing.push_str(&text);
                } else {
                    self.messages.push(ChatMessage::Thinking(text));
                }
            }
            SessionUpdate::ToolCall(info) => {
                self.messages.push(ChatMessage::ToolCall {
                    tool_call_id: info.tool_call_id,
                    title: info.title,
                    kind: info.kind,
                    status: info.status,
                });
            }
            SessionUpdate::ToolCallUpdate(info) => {
                // Find the matching tool call by id (searching from most recent).
                for msg in self.messages.iter_mut().rev() {
                    if let ChatMessage::ToolCall {
                        tool_call_id,
                        status,
                        title,
                        ..
                    } = msg
                        && *tool_call_id == info.tool_call_id
                    {
                        if let Some(new_status) = &info.status {
                            *status = new_status.clone();
                        }
                        if let Some(new_title) = &info.title {
                            *title = new_title.clone();
                        }
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    /// Flush the agent text buffer into a completed [`ChatMessage::Agent`]
    /// message and reset streaming state.
    pub fn flush_agent_message(&mut self) {
        if !self.agent_text_buffer.is_empty() {
            let text = std::mem::take(&mut self.agent_text_buffer);
            self.messages
                .push(ChatMessage::Agent(text.trim_end().to_string()));
        }
        self.streaming = false;
    }

    /// Returns the current in-progress streaming text (not yet flushed).
    pub fn streaming_text(&self) -> &str {
        &self.agent_text_buffer
    }

    /// Add a user message to the conversation.
    pub fn add_user_message(&mut self, text: String) {
        self.messages.push(ChatMessage::User(text));
    }

    /// Add a system message to the conversation.
    pub fn add_system_message(&mut self, text: String) {
        self.messages.push(ChatMessage::System(text));
    }

    /// Add a command suggestion to the conversation.
    pub fn add_command_suggestion(&mut self, command: String) {
        self.messages.push(ChatMessage::CommandSuggestion(command));
    }

    /// Add an auto-approved tool call notice to the conversation.
    pub fn add_auto_approved(&mut self, description: String) {
        self.messages.push(ChatMessage::AutoApproved(description));
    }
}

impl Default for ChatState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::protocol::{ToolCallInfo, ToolCallUpdateInfo};

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

        assert!(matches!(&state.messages[0], ChatMessage::User(t) if t == "test"));
        assert!(matches!(&state.messages[1], ChatMessage::System(t) if t == "system"));
        assert!(
            matches!(&state.messages[2], ChatMessage::CommandSuggestion(t) if t == "cargo test")
        );
        assert!(matches!(&state.messages[3], ChatMessage::AutoApproved(t) if t == "read file"));
    }
}

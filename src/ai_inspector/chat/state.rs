//! `ChatState` — manages the conversation history, streaming buffer, and
//! message assembly for the AI Inspector panel.

use par_term_acp::SessionUpdate;

use super::text_utils::{extract_code_block_commands, truncate_replay_text};
use super::types::ChatMessage;

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
    ///
    /// Non-chunk updates automatically flush any accumulated agent text
    /// buffer so that the complete message is recorded before tool calls
    /// or other events.
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
                // Flush any pending agent text before recording a tool call.
                self.flush_agent_message();
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
            _ => {
                // For any other update type, flush pending text.
                self.flush_agent_message();
            }
        }
    }

    /// Flush the agent text buffer into a completed [`ChatMessage::Agent`]
    /// message and reset streaming state.
    ///
    /// Also extracts any fenced bash/sh code blocks and appends them as
    /// [`ChatMessage::CommandSuggestion`] entries so the UI can offer
    /// "Run in terminal" buttons.
    pub fn flush_agent_message(&mut self) {
        if !self.agent_text_buffer.is_empty() {
            let text = std::mem::take(&mut self.agent_text_buffer);
            let trimmed = text.trim_end().to_string();

            // Extract fenced code blocks with bash/sh language tags
            let commands = extract_code_block_commands(&trimmed);

            self.messages.push(ChatMessage::Agent(trimmed));

            for cmd in commands {
                self.messages.push(ChatMessage::CommandSuggestion(cmd));
            }
        }
        self.streaming = false;
    }

    /// Returns the current in-progress streaming text (not yet flushed).
    pub fn streaming_text(&self) -> &str {
        &self.agent_text_buffer
    }

    /// Add a user message to the conversation.
    ///
    /// Flushes any pending agent text first so messages stay interleaved.
    /// The message starts as `pending: true` (queued, not yet sent).
    pub fn add_user_message(&mut self, text: String) {
        self.flush_agent_message();
        self.messages.push(ChatMessage::User {
            text,
            pending: true,
        });
    }

    /// Mark the oldest pending user message as sent (no longer cancellable).
    pub fn mark_oldest_pending_sent(&mut self) {
        for msg in &mut self.messages {
            if let ChatMessage::User { pending, .. } = msg
                && *pending
            {
                *pending = false;
                return;
            }
        }
    }

    /// Cancel and remove the most recent pending user message.
    ///
    /// Returns `true` if a message was removed.
    pub fn cancel_last_pending(&mut self) -> bool {
        for i in (0..self.messages.len()).rev() {
            if let ChatMessage::User { pending: true, .. } = &self.messages[i] {
                self.messages.remove(i);
                return true;
            }
        }
        false
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

    /// Clear all chat messages and reset streaming state.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.agent_text_buffer.clear();
        self.streaming = false;
    }

    /// Build a bounded transcript prompt for restoring local chat context
    /// into a newly connected ACP session.
    ///
    /// This is best-effort: it preserves visible UI conversation context
    /// (user/agent/system/tool summaries), not the agent's internal session
    /// state or permission request identifiers.
    pub fn build_context_replay_prompt(&self) -> Option<String> {
        const MAX_ENTRIES: usize = 24;
        const MAX_TOTAL_CHARS: usize = 16_000;
        const MAX_ENTRY_CHARS: usize = 1_200;

        let mut entries: Vec<String> = Vec::new();

        for msg in &self.messages {
            match msg {
                ChatMessage::User {
                    text,
                    pending: false,
                } => {
                    entries.push(format!(
                        "[User]\n{}",
                        truncate_replay_text(text, MAX_ENTRY_CHARS)
                    ));
                }
                ChatMessage::User { pending: true, .. } => {
                    // Skip queued/unsent prompts: the new session never saw them.
                }
                ChatMessage::Agent(text) => {
                    entries.push(format!(
                        "[Assistant]\n{}",
                        truncate_replay_text(text, MAX_ENTRY_CHARS)
                    ));
                }
                ChatMessage::System(text) => {
                    entries.push(format!(
                        "[System]\n{}",
                        truncate_replay_text(text, MAX_ENTRY_CHARS / 2)
                    ));
                }
                ChatMessage::AutoApproved(desc) => {
                    entries.push(format!(
                        "[Tool Auto-Approved]\n{}",
                        truncate_replay_text(desc, MAX_ENTRY_CHARS / 2)
                    ));
                }
                ChatMessage::ToolCall {
                    title,
                    kind,
                    status,
                    ..
                } => {
                    entries.push(format!(
                        "[Tool Call]\n{} ({kind}) - {status}",
                        truncate_replay_text(title, MAX_ENTRY_CHARS / 2)
                    ));
                }
                ChatMessage::Permission {
                    description,
                    resolved,
                    ..
                } => {
                    let state = if *resolved { "resolved" } else { "unresolved" };
                    entries.push(format!(
                        "[Permission Request - {state}]\n{}",
                        truncate_replay_text(description, MAX_ENTRY_CHARS / 2)
                    ));
                }
                ChatMessage::Thinking(_) | ChatMessage::CommandSuggestion(_) => {
                    // Skip internal reasoning and derived command suggestions to
                    // reduce noise/duplication in the replay transcript.
                }
            }
        }

        if !self.agent_text_buffer.trim().is_empty() {
            entries.push(format!(
                "[Assistant Partial]\n{}",
                truncate_replay_text(&self.agent_text_buffer, MAX_ENTRY_CHARS)
            ));
        }

        if entries.is_empty() {
            return None;
        }

        let mut selected: Vec<String> = Vec::new();
        let mut total_chars = 0usize;
        for entry in entries.iter().rev() {
            let entry_chars = entry.chars().count();
            if !selected.is_empty()
                && (selected.len() >= MAX_ENTRIES || total_chars + entry_chars > MAX_TOTAL_CHARS)
            {
                break;
            }
            total_chars += entry_chars;
            selected.push(entry.clone());
        }
        selected.reverse();

        let mut prompt = String::from(
            "[System: par-term context restore]\n\
The following is a best-effort transcript reconstructed from the local UI chat \
history after reconnecting or switching agent/provider. It preserves visible \
conversation context only (not hidden session state, pending permissions, or \
tool-call IDs). Use it to continue the conversation naturally from the latest \
user request. Do not restate the transcript unless asked.\n\n",
        );

        if selected.len() < entries.len() {
            prompt.push_str("[Older transcript entries omitted for length.]\n\n");
        }

        prompt.push_str(&selected.join("\n\n"));
        Some(prompt)
    }
}

impl Default for ChatState {
    fn default() -> Self {
        Self::new()
    }
}

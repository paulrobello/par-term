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
    /// Previously submitted user prompts, newest first.
    input_history: Vec<String>,
    /// Current position while navigating input history.
    input_history_cursor: Option<usize>,
    /// Draft text to restore when navigating back past the newest history entry.
    input_history_draft: Option<String>,
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
            input_history: Vec::new(),
            input_history_cursor: None,
            input_history_draft: None,
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

    pub fn set_input_history(&mut self, entries: Vec<String>) {
        self.input_history = par_term_config::normalize_assistant_input_history(entries);
        self.reset_input_history_navigation();
    }

    pub fn input_history_entries(&self) -> &[String] {
        &self.input_history
    }

    pub fn record_user_input_history(&mut self, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            self.reset_input_history_navigation();
            return;
        }

        let mut entries = Vec::with_capacity(self.input_history.len() + 1);
        entries.push(trimmed.to_string());
        entries.extend(self.input_history.iter().cloned());
        self.input_history = par_term_config::normalize_assistant_input_history(entries);
        self.reset_input_history_navigation();
    }

    pub fn navigate_input_history_older(&mut self) -> bool {
        if self.input_history.is_empty() {
            return false;
        }

        let next_cursor = match self.input_history_cursor {
            Some(cursor) if cursor + 1 < self.input_history.len() => cursor + 1,
            Some(_) => return false,
            None => {
                self.input_history_draft = Some(self.input.clone());
                0
            }
        };

        self.input_history_cursor = Some(next_cursor);
        self.input = self.input_history[next_cursor].clone();
        true
    }

    pub fn navigate_input_history_newer(&mut self) -> bool {
        let Some(cursor) = self.input_history_cursor else {
            return false;
        };

        if cursor == 0 {
            self.input_history_cursor = None;
            self.input = self.input_history_draft.take().unwrap_or_default();
            return true;
        }

        let next_cursor = cursor - 1;
        self.input_history_cursor = Some(next_cursor);
        self.input = self.input_history[next_cursor].clone();
        true
    }

    pub fn reset_input_history_navigation(&mut self) {
        self.input_history_cursor = None;
        self.input_history_draft = None;
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

#[cfg(test)]
mod tests {
    use super::ChatState;

    #[test]
    fn assistant_input_history_records_trimmed_newest_unique_entries() {
        let mut state = ChatState::new();

        state.record_user_input_history("  newest  ");
        state.record_user_input_history("");
        state.record_user_input_history("older");
        state.record_user_input_history("newest");

        assert_eq!(
            state.input_history_entries(),
            ["newest".to_string(), "older".to_string()]
        );
    }

    #[test]
    fn assistant_input_history_navigate_older_snapshots_draft() {
        let mut state = ChatState::new();
        state.set_input_history(vec!["newest".to_string(), "older".to_string()]);
        state.input = "draft".to_string();

        assert!(state.navigate_input_history_older());
        assert_eq!(state.input, "newest");

        assert!(state.navigate_input_history_older());
        assert_eq!(state.input, "older");

        assert!(!state.navigate_input_history_older());
        assert_eq!(state.input, "older");
    }

    #[test]
    fn assistant_input_history_navigate_newer_restores_saved_draft() {
        let mut state = ChatState::new();
        state.set_input_history(vec!["newest".to_string(), "older".to_string()]);
        state.input = "draft".to_string();
        assert!(state.navigate_input_history_older());
        assert!(state.navigate_input_history_older());

        assert!(state.navigate_input_history_newer());
        assert_eq!(state.input, "newest");

        assert!(state.navigate_input_history_newer());
        assert_eq!(state.input, "draft");

        assert!(!state.navigate_input_history_newer());
        assert_eq!(state.input, "draft");
    }

    #[test]
    fn assistant_input_history_recording_new_prompt_resets_navigation_state() {
        let mut state = ChatState::new();
        state.set_input_history(vec!["newest".to_string(), "older".to_string()]);
        state.input = "draft".to_string();
        assert!(state.navigate_input_history_older());

        state.record_user_input_history("fresh");
        state.input = "current draft".to_string();

        assert!(state.navigate_input_history_older());
        assert_eq!(state.input, "fresh");
        assert!(state.navigate_input_history_newer());
        assert_eq!(state.input, "current draft");
    }
}

//! Chat data model for agent conversations in the AI Inspector.
//!
//! Provides [`ChatState`] for tracking the conversation between the user and
//! an ACP agent, including streaming message assembly, tool call tracking,
//! permission requests, and system messages.

use par_term_acp::SessionUpdate;

/// A message in the chat history.
#[derive(Debug, Clone)]
pub enum ChatMessage {
    /// A message sent by the user. `pending` is true while the prompt is
    /// queued (waiting for the agent lock) and false once sending has begun.
    User { text: String, pending: bool },
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

/// System guidance sent with each user prompt so the agent always wraps shell
/// commands in fenced code blocks (which the UI extracts as runnable
/// `CommandSuggestion` entries).
pub const AGENT_SYSTEM_GUIDANCE: &str = "\
[System instructions: highest priority] You are an AI assistant running via the ACP (Agent Communication \
Protocol) inside par-term, a GPU-accelerated terminal emulator. \
You have filesystem access through ACP: you can read and write files. \
IMPORTANT: Some local tools like Find/Glob may not work in this ACP environment. \
If a file search or directory listing fails, do NOT stop — instead work around it: \
use shell commands (ls, find) wrapped in code blocks to discover files, or ask the \
user for paths. Always continue helping even when a tool call fails. \
When you suggest shell commands, ALWAYS wrap them in a fenced code block with a \
shell language tag (```bash, ```sh, ```zsh, or ```shell). \
The terminal UI will detect these blocks and render them with \"Run\" and \"Paste\" \
buttons so the user can execute them directly. When the user runs a command, \
you will receive a notification with the exit code, and the command output will \
be visible to you through the normal terminal capture channel. \
Do NOT add disclaimers about output not being captured. \
Plain-text command suggestions will NOT be actionable. \
Never use bare ``` blocks for commands — always include the language tag. \
For direct executable requests, complete the full requested workflow before \
declaring success (for example, if asked to create a shader and set it active, \
you must both write the shader file and call `config_update` to activate it). \
Do NOT call Skill, Task, or TodoWrite-style tools unless they are explicitly \
available and working in this ACP host. If they fail or are unavailable, \
continue by keeping a plain-text checklist in your response instead. \
Do NOT switch into plan mode for direct executable requests (for example \
creating files, editing code, or changing par-term settings/shaders). Do not \
call `EnterPlanMode` or `Todo`/`TodoWrite` tools in this host unless they are \
explicitly available and required by the user. \
There is no generic `Skill file-write` helper in this host; to create/edit files, \
use the normal ACP file tools (for example Read/Write/Edit) directly. \
If a `Read` call fails because the path is a directory, do not loop on `Read`; \
use a directory-listing/search tool (if available) or write the known target file \
path directly. \
When using the `Write` tool, use the exact parameter names expected by the host \
(for example `file_path` and `content`, not `filepath`). If a write fails, \
correct the tool parameters and retry the same task instead of switching to an \
unrelated example or different project/file. \
Never emit XML-style tool markup such as <function=...> or <tool_call> tags \
in regular chat responses. \
If asked which model/provider you are, do not assume Anthropic defaults; \
state you are running through an ACP wrapper in par-term and use configured \
backend/model details when available. \
To modify par-term settings (shaders, font_size, window_opacity, etc.), use the \
`config_update` MCP tool (available via par-term-config MCP server). \
Example: call config_update with updates: {\"custom_shader\": \"crt.glsl\", \
\"custom_shader_enabled\": true}. Changes apply immediately — no restart needed. \
For visual/shader debugging, you can request a terminal screenshot using the \
`terminal_screenshot` MCP tool (from the same par-term MCP server). This may \
require user permission and returns an image of the current terminal output. \
IMPORTANT: Do NOT edit ~/.config/par-term/config.yaml directly — always use the \
config_update tool instead. Direct config.yaml edits race with par-term's own \
config saves and will be silently overwritten.\n\n";

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
}

/// A segment of agent message text for rendering.
#[derive(Debug, PartialEq)]
pub enum TextSegment {
    /// Regular (non-code) text.
    Plain(String),
    /// A fenced code block with optional language tag.
    CodeBlock { lang: String, code: String },
}

/// Parse agent message text into alternating plain-text and code-block segments.
///
/// Recognises fenced code blocks delimited by triple backticks, with an
/// optional language tag on the opening fence. Unclosed code blocks are
/// treated as extending to the end of the text.
pub fn parse_text_segments(text: &str) -> Vec<TextSegment> {
    let mut segments = Vec::new();
    let mut plain_lines: Vec<&str> = Vec::new();
    let mut in_block = false;
    let mut block_lang = String::new();
    let mut code_lines: Vec<&str> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if in_block {
                // End of code block
                let code = code_lines.join("\n");
                segments.push(TextSegment::CodeBlock {
                    lang: std::mem::take(&mut block_lang),
                    code,
                });
                code_lines.clear();
                in_block = false;
            } else {
                // Flush accumulated plain text
                if !plain_lines.is_empty() {
                    segments.push(TextSegment::Plain(plain_lines.join("\n")));
                    plain_lines.clear();
                }
                // Start code block — extract language tag
                block_lang = trimmed.trim_start_matches('`').trim().to_string();
                in_block = true;
            }
        } else if in_block {
            code_lines.push(line);
        } else {
            plain_lines.push(line);
        }
    }

    // Flush remaining content
    if in_block {
        let code = code_lines.join("\n");
        segments.push(TextSegment::CodeBlock {
            lang: block_lang,
            code,
        });
    } else if !plain_lines.is_empty() {
        segments.push(TextSegment::Plain(plain_lines.join("\n")));
    }

    segments
}

/// Extract shell commands from fenced code blocks in text.
///
/// Looks for code blocks tagged with `bash`, `sh`, `shell`, or `zsh`.
/// Supports additional metadata after the language tag (for example:
/// ` ```bash title=example`), and combines continuation lines ending with `\`.
/// Lines starting with `#` (comments) or empty lines are skipped.
fn extract_code_block_commands(text: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut in_block = false;
    let mut is_shell_block = false;
    let mut continued: Vec<String> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if in_block {
                // End of block
                if !continued.is_empty() {
                    commands.push(continued.join(" "));
                    continued.clear();
                }
                in_block = false;
                is_shell_block = false;
            } else {
                // Start of block — check language tag
                let lang = trimmed
                    .trim_start_matches('`')
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                is_shell_block = lang == "bash" || lang == "sh" || lang == "shell" || lang == "zsh";
                in_block = true;
            }
            continue;
        }

        if in_block && is_shell_block {
            let cmd = trimmed.strip_prefix("$ ").unwrap_or(trimmed);
            if cmd.is_empty() || cmd.starts_with('#') {
                continue;
            }

            let continued_line = cmd.ends_with('\\');
            let segment = if continued_line {
                cmd.trim_end_matches('\\').trim_end()
            } else {
                cmd
            };

            if !segment.is_empty() {
                continued.push(segment.to_string());
            }

            if !continued_line && !continued.is_empty() {
                commands.push(continued.join(" "));
                continued.clear();
            }
        }
    }

    if !continued.is_empty() {
        commands.push(continued.join(" "));
    }

    commands
}

/// Extract a literal XML-style `config_update` tool call emitted as plain text.
///
/// Some local backends can emit `<function=...>` / `<parameter=...>` blocks
/// instead of a structured ACP tool call. This helper parses that fallback
/// format so the host can still apply the requested config update.
pub fn extract_inline_config_update(
    text: &str,
) -> Option<std::collections::HashMap<String, serde_json::Value>> {
    const FN_TAG: &str = "<function=mcp__par-term-config__config_update>";
    const PARAM_START: &str = "<parameter=updates>";
    const PARAM_END: &str = "</parameter>";

    let fn_idx = text.find(FN_TAG)?;
    let after_fn = &text[fn_idx + FN_TAG.len()..];
    let param_idx = after_fn.find(PARAM_START)?;
    let after_param = &after_fn[param_idx + PARAM_START.len()..];
    let end_idx = after_param.find(PARAM_END)?;
    let json_text = after_param[..end_idx].trim();
    if json_text.is_empty() {
        return None;
    }

    let parsed: serde_json::Value = serde_json::from_str(json_text).ok()?;
    match parsed {
        serde_json::Value::Object(mut map) => {
            if let Some(serde_json::Value::Object(updates)) = map.remove("updates") {
                Some(updates.into_iter().collect())
            } else {
                Some(map.into_iter().collect())
            }
        }
        _ => None,
    }
}

/// Extract the function name from XML-style inline tool markup emitted as
/// plain text, e.g. `<function=Write>` -> `Write`.
pub fn extract_inline_tool_function_name(text: &str) -> Option<String> {
    let start = text.find("<function=")?;
    let after = &text[start + "<function=".len()..];
    let end = after.find('>')?;
    let name = after[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
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
    use par_term_acp::{ToolCallInfo, ToolCallUpdateInfo};

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
        let text = "```bash\ncurl -H 'Auth: a' \\\n  --data 'x=1' \\\n  https://example.test\n```";
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
}

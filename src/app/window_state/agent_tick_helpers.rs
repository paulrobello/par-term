//! Per-tick helper methods for ACP agent message processing.
//!
//! These private methods are called by `process_agent_messages_tick` in
//! `agent_messages.rs` to keep that function manageable.  Each helper
//! encapsulates one logical phase of the per-frame agent processing loop:
//!
//! - `attempt_skill_failure_recovery` — bounded retry when a local-backend
//!   tool call fails or a shader activation is incomplete.
//! - `feed_auto_context` — send the latest command metadata to the agent
//!   when auto-context is enabled and a new command has completed.
//! - `refresh_inspector_snapshot` — rebuild the AI Inspector snapshot when
//!   the panel is open and a refresh has been requested.

use crate::ai_inspector::chat::ChatMessage;
use crate::app::window_state::WindowState;
use crate::app::window_state::agent_message_helpers::redact_auto_context_command;
use par_term_acp::{AgentMessage, AgentStatus, ContentBlock};

const AUTO_CONTEXT_MIN_INTERVAL_MS: u64 = 1200;

impl WindowState {
    /// Bounded recovery: if the prompt failed due to a local backend tool
    /// mismatch (failed Skill/Write or inline tool markup), or if a shader
    /// activation request completed without a config_update call, nudge the
    /// agent to continue the same task with proper ACP tool use.
    ///
    /// Returns true if a recovery prompt was sent.
    pub(crate) fn attempt_skill_failure_recovery(
        &mut self,
        saw_prompt_complete: bool,
        shader_activation_incomplete: bool,
        last_user_text: &Option<String>,
    ) -> bool {
        if !saw_prompt_complete
            || (!self.agent_state.agent_skill_failure_detected && !shader_activation_incomplete)
            || self.agent_state.agent_skill_recovery_attempts >= 3
        {
            return false;
        }

        let Some(agent) = &self.agent_state.agent else {
            return false;
        };

        let had_recoverable_failure = self.agent_state.agent_skill_failure_detected;
        self.agent_state.agent_skill_recovery_attempts = self
            .agent_state
            .agent_skill_recovery_attempts
            .saturating_add(1);
        self.agent_state.agent_skill_failure_detected = false;
        self.overlay_ui.ai_inspector.chat.streaming = true;
        if shader_activation_incomplete && !had_recoverable_failure {
            self.overlay_ui
                .ai_inspector
                .chat
                .add_system_message(format!(
                    "Agent completed a shader task response without activating the shader via \
                     config_update. Auto-retrying (attempt {}/3) to finish the activation step.",
                    self.agent_state.agent_skill_recovery_attempts
                ));
        } else {
            self.overlay_ui.ai_inspector.chat.add_system_message(
                format!(
                    "Recoverable local-backend tool failure detected (failed Skill/Write or \
                     inline tool markup). Auto-retrying (attempt {}/3) with stricter ACP tool guidance.",
                    self.agent_state.agent_skill_recovery_attempts
                ),
            );
        }

        let mut content: Vec<ContentBlock> = vec![ContentBlock::Text {
            text: format!(
                "{}[End system instructions]",
                crate::ai_inspector::chat::AGENT_SYSTEM_GUIDANCE
            ),
        }];

        if self.config.ai_inspector.ai_inspector_agent_terminal_access {
            content.push(ContentBlock::Text {
                text: "[Terminal access enabled]\n\
                    Shell commands you write in fenced code blocks \
                    (```bash, ```sh, ```zsh, or ```shell) are automatically \
                    executed in the user's active terminal — no user click or \
                    approval required. You can run commands autonomously on \
                    behalf of the user. After each command completes, you will \
                    receive an auto-context notification with the exit code, \
                    duration, and working directory. Use this capability to run \
                    commands, observe results, and complete multi-step tasks \
                    without waiting for user input."
                    .to_string(),
            });
        }

        if let Some(user_text) = last_user_text
            && crate::ai_inspector::shader_context::should_inject_shader_context(
                user_text,
                &self.config,
            )
        {
            content.push(ContentBlock::Text {
                text: crate::ai_inspector::shader_context::build_shader_context(&self.config),
            });
        }

        let extra_recovery_strictness = if self.agent_state.agent_skill_recovery_attempts >= 2 {
            " Do not explore unrelated files or dependencies. For shader tasks, go directly \
             to the shader file write and config_update activation steps."
        } else {
            ""
        };
        content.push(ContentBlock::Text {
            text: format!(
                "[Host recovery note]\nContinue the previous user task and stay on the \
                   same domain/problem (do not switch to unrelated examples/files). Do NOT \
                   use `Skill`, `Task`, or `TodoWrite`. Do NOT emit XML-style tool markup \
                   (`<function=...>`). Use normal ACP file/system/MCP tools directly. If \
                   a `Read` fails because the target is a directory, do not retry `Read` on \
                   that directory; use a listing/search tool or write the known target file \
                   path directly. \
                   Complete the full requested workflow before declaring success (for shader \
                   tasks: write the requested shader content, then call config_update to \
                   activate it). \
                   using `Write`, use exact parameters like `file_path` and `content` (not \
                   `filepath`). For par-term settings changes use \
                   `mcp__par-term-config__config_update` / `config_update`. If a tool \
                   fails, correct the call and retry the same task with the available \
                   tools. If you have already created the requested shader file, do not \
                   stop there: call config_update now to activate it before declaring \
                   success. Do not ask the user to restate the request unless you truly \
                   need missing information.{}",
                extra_recovery_strictness
            ),
        });

        let agent = agent.clone();
        let tx = self.agent_state.agent_tx.clone();
        let handle = self.runtime.spawn(async move {
            let agent = agent.lock().await;
            if let Some(ref tx) = tx {
                let _ = tx.send(AgentMessage::PromptStarted);
            }
            let _ = agent.send_prompt(content).await;
            if let Some(tx) = tx {
                let _ = tx.send(AgentMessage::PromptComplete);
            }
        });
        self.agent_state.pending_send_handles.push_back(handle);
        self.focus_state.needs_redraw = true;
        true
    }

    /// Auto-execute new CommandSuggestion messages when terminal access is
    /// enabled, and send the latest command metadata to the agent when
    /// auto-context is enabled and a new command has completed.
    pub(crate) fn feed_auto_context(&mut self, msg_count_before: usize) {
        // Auto-execute new CommandSuggestion messages when terminal access is enabled.
        if self.config.ai_inspector.ai_inspector_agent_terminal_access {
            let new_messages = &self.overlay_ui.ai_inspector.chat.messages[msg_count_before..];
            let commands_to_run: Vec<String> = new_messages
                .iter()
                .filter_map(|msg| {
                    if let ChatMessage::CommandSuggestion(cmd) = msg {
                        Some(format!("{cmd}\n"))
                    } else {
                        None
                    }
                })
                .collect();

            if !commands_to_run.is_empty()
                && let Some(tab) = self.tab_manager.active_tab()
                && let Ok(term) = tab.terminal.try_write()
            {
                for cmd in &commands_to_run {
                    let _ = term.write(cmd.as_bytes());
                }
                crate::debug_info!(
                    "AI_INSPECTOR",
                    "Auto-executed {} command(s) in terminal",
                    commands_to_run.len()
                );
            }
        }

        // Detect new command completions and auto-refresh the snapshot.
        // This is separate from agent auto-context so the panel always shows
        // up-to-date command history regardless of agent connection state.
        if self.overlay_ui.ai_inspector.open
            && let Some(tab) = self.tab_manager.active_tab()
            && let Ok(term) = tab.terminal.try_write()
        {
            let history = term.core_command_history();
            let current_count = history.len();

            if current_count != self.overlay_ui.ai_inspector.last_command_count {
                // Command count changed — refresh the snapshot
                let had_commands = self.overlay_ui.ai_inspector.last_command_count > 0;
                self.overlay_ui.ai_inspector.last_command_count = current_count;
                self.overlay_ui.ai_inspector.needs_refresh = true;

                // Auto-context feeding: send latest command info to agent.
                // Fires when auto-context is enabled OR when terminal drive is
                // active so the agent can see the outcome of commands it ran.
                if had_commands
                    && current_count > 0
                    && (self.config.ai_inspector.ai_inspector_auto_context
                        || self.config.ai_inspector.ai_inspector_agent_terminal_access)
                    && self.overlay_ui.ai_inspector.agent_status == AgentStatus::Connected
                    && let Some((cmd, exit_code, duration_ms)) = history.last()
                {
                    let now = std::time::Instant::now();
                    let throttled =
                        self.agent_state
                            .last_auto_context_sent_at
                            .is_some_and(|last_sent| {
                                now.duration_since(last_sent)
                                    < std::time::Duration::from_millis(AUTO_CONTEXT_MIN_INTERVAL_MS)
                            });

                    if !throttled {
                        let exit_code_str = exit_code
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| "N/A".to_string());
                        let duration = duration_ms.unwrap_or(0);

                        let cwd = term.shell_integration_cwd().unwrap_or_default();
                        let (sanitized_cmd, was_redacted) = redact_auto_context_command(cmd);

                        let context = format!(
                            "[Auto-context event]\nCommand completed:\n$ {}\nExit code: {}\nDuration: {}ms\nCWD: {}\nSensitive arguments redacted: {}",
                            sanitized_cmd, exit_code_str, duration, cwd, was_redacted
                        );

                        if let Some(agent) = &self.agent_state.agent {
                            self.agent_state.last_auto_context_sent_at = Some(now);
                            self.overlay_ui.ai_inspector.chat.add_system_message(if was_redacted {
                                "Auto-context sent command metadata to the agent (sensitive values redacted).".to_string()
                            } else {
                                "Auto-context sent command metadata to the agent.".to_string()
                            });
                            self.focus_state.needs_redraw = true;
                            let agent = agent.clone();
                            let content = vec![ContentBlock::Text { text: context }];
                            self.runtime.spawn(async move {
                                let agent = agent.lock().await;
                                let _ = agent.send_prompt(content).await;
                            });
                        }
                    }
                }
            }
        }
    }

    /// Refresh the AI Inspector snapshot if the panel is open and a refresh
    /// has been requested (e.g., after a new command completed).
    pub(crate) fn refresh_inspector_snapshot(&mut self) {
        if self.overlay_ui.ai_inspector.open
            && self.overlay_ui.ai_inspector.needs_refresh
            && let Some(tab) = self.tab_manager.active_tab()
            && let Ok(term) = tab.terminal.try_write()
        {
            let snapshot = crate::ai_inspector::snapshot::SnapshotData::gather(
                &term,
                &self.overlay_ui.ai_inspector.scope,
                self.config.ai_inspector.ai_inspector_context_max_lines,
            );
            self.overlay_ui.ai_inspector.snapshot = Some(snapshot);
            self.overlay_ui.ai_inspector.needs_refresh = false;
        }
    }
}

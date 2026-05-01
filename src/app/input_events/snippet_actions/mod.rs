//! Snippet and custom action execution helpers for WindowState keybindings.
//!
//! Sub-modules:
//! - `workflow`: Sequence/repeat/condition evaluation engine and glob matching
//! - `shell_command`: ShellCommand action handler (fire-and-forget + capture_output)
//! - `new_tab`: NewTab action handler (tab creation + delayed command write)
//! - `split_pane`: SplitPane action handler (pane splitting + delayed command write)
//! - `key_sequence`: KeySequence action handler and prefix-action char extraction
//! - `insert_text`: InsertText action handler (variable substitution + terminal write)
//! - `tests`: Unit tests

mod insert_text;
mod key_sequence;
mod new_tab;
mod shell_command;
mod split_pane;
#[cfg(test)]
mod tests;
mod workflow;

// Re-export items used by tests and by the prefix-key handler below.
pub(crate) use key_sequence::extract_prefix_action_char;

use crate::app::window_state::WindowState;
use crate::config::snippets::{CustomActionConfig, normalize_action_prefix_char};
use std::sync::Arc;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

const CUSTOM_ACTION_PREFIX_TOAST: &str = "Actions: prefix... (Esc to cancel)";

fn prefix_action_for_char(actions: &[CustomActionConfig], input_char: char) -> Option<String> {
    let normalized_input = normalize_action_prefix_char(input_char);

    actions
        .iter()
        .find(|action| action.normalized_prefix_char() == Some(normalized_input))
        .map(|action| action.id().to_string())
}

impl WindowState {
    fn show_custom_action_prefix_toast(&mut self) {
        self.overlay_state.toast_message = Some(CUSTOM_ACTION_PREFIX_TOAST.to_string());
        self.overlay_state.toast_hide_time = None;
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    fn clear_custom_action_prefix_toast(&mut self) {
        if self.overlay_state.toast_message.as_deref() == Some(CUSTOM_ACTION_PREFIX_TOAST) {
            self.overlay_state.toast_message = None;
            self.overlay_state.toast_hide_time = None;
            self.focus_state.needs_redraw = true;
            self.request_redraw();
        }
    }

    /// Handle the global custom-action prefix key and its single-character follow-up.
    pub(crate) fn handle_custom_action_prefix_key(&mut self, event: &KeyEvent) -> bool {
        if self.custom_action_prefix_state.is_active() {
            if event.state != ElementState::Pressed {
                return true;
            }

            let is_modifier_only = matches!(
                event.logical_key,
                Key::Named(
                    NamedKey::Shift
                        | NamedKey::Control
                        | NamedKey::Alt
                        | NamedKey::Super
                        | NamedKey::Meta
                )
            );
            if is_modifier_only {
                return true;
            }

            self.custom_action_prefix_state.exit();
            self.clear_custom_action_prefix_toast();

            if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
                crate::debug_log!("PREFIX_ACTION", "Esc pressed, prefix mode cancelled");
                return true;
            }

            let Some(input_char) = extract_prefix_action_char(event) else {
                self.show_toast("Actions: unsupported key");
                return true;
            };

            if let Some(action_id) = prefix_action_for_char(&self.config.actions, input_char) {
                if !self.execute_custom_action(&action_id) {
                    self.show_toast("Actions: failed");
                }
                return true;
            }

            self.show_toast(format!("Actions: no binding for {}", input_char));
            return true;
        }

        if event.state != ElementState::Pressed {
            return false;
        }

        let Some(prefix_combo) = self.custom_action_prefix_combo.as_ref() else {
            return false;
        };

        if !self
            .config
            .actions
            .iter()
            .any(|action| action.prefix_char().is_some())
        {
            crate::debug_log!(
                "PREFIX_ACTION",
                "No actions with prefix_char configured, skipping"
            );
            return false;
        }

        let matcher = crate::keybindings::KeybindingMatcher::from_event_with_remapping(
            event,
            &self.input_handler.modifiers,
            &self.config.modifier_remapping,
        );

        if matcher.matches_with_physical_preference(prefix_combo, self.config.use_physical_keys) {
            crate::debug_info!(
                "PREFIX_ACTION",
                "Prefix combo matched, entering prefix mode"
            );
            self.custom_action_prefix_state.enter();
            self.show_custom_action_prefix_toast();
            return true;
        }

        false
    }

    /// Execute a snippet by ID.
    ///
    /// Returns true if the snippet was found and executed, false otherwise.
    pub(crate) fn execute_snippet(&mut self, snippet_id: &str) -> bool {
        // Find the snippet by ID
        let snippet = match self.config.snippets.iter().find(|s| s.id == snippet_id) {
            Some(s) => s,
            None => {
                log::warn!("Snippet not found: {}", snippet_id);
                return false;
            }
        };

        // Check if snippet is enabled
        if !snippet.enabled {
            log::debug!("Snippet '{}' is disabled", snippet.title);
            return false;
        }

        // Substitute variables in the snippet content, including session variables
        let substituted_content = {
            let session_vars = self.badge_state.variables.read();
            let result = crate::snippets::VariableSubstitutor::new().substitute_with_session(
                &snippet.content,
                &snippet.variables,
                Some(&session_vars),
            );
            drop(session_vars); // Explicitly drop before using self again
            match result {
                Ok(content) => content,
                Err(e) => {
                    log::error!(
                        "Failed to substitute variables in snippet '{}': {}",
                        snippet.title,
                        e
                    );
                    self.show_toast(format!("Snippet Error: {}", e));
                    return false;
                }
            }
        };

        // Write to the active terminal
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            // try_lock: intentional -- execute_snippet called from keybinding handler in
            // sync event loop. On miss: the snippet is not sent to the terminal this
            // invocation. The user can trigger the keybinding again.
            if let Ok(terminal) = tab.terminal.try_write() {
                // Append newline if auto_execute is enabled
                let content_to_write = if snippet.auto_execute {
                    format!("{}\n", substituted_content)
                } else {
                    substituted_content.clone()
                };

                if let Err(e) = terminal.write(content_to_write.as_bytes()) {
                    log::error!("Failed to write snippet to terminal: {}", e);
                    return false;
                }

                log::info!(
                    "Executed snippet '{}' (auto_execute={})",
                    snippet.title,
                    snippet.auto_execute
                );
                return true;
            } else {
                log::error!("Failed to lock terminal for snippet execution");
                return false;
            }
        }

        false
    }

    /// Execute a custom action by ID.
    ///
    /// Returns true if the action was found and executed, false otherwise.
    pub(crate) fn execute_custom_action(&mut self, action_id: &str) -> bool {
        // Find and clone the action up front to release the immutable borrow on
        // `self.config.actions` before calling `&mut self` methods on the sub-modules.
        let action = match self.config.actions.iter().find(|a| a.id() == action_id) {
            Some(a) => a.clone(),
            None => {
                log::warn!("Custom action not found: {}", action_id);
                return false;
            }
        };

        match action {
            CustomActionConfig::ShellCommand {
                command,
                args,
                notify_on_success,
                timeout_secs,
                title,
                capture_output,
                ..
            } => self.execute_shell_command_action(
                command,
                args,
                notify_on_success,
                timeout_secs,
                title,
                capture_output,
            ),
            CustomActionConfig::NewTab { command, title, .. } => {
                self.execute_new_tab_action(command, title)
            }
            CustomActionConfig::InsertText {
                text, variables, ..
            } => self.execute_insert_text_action(text, variables),
            CustomActionConfig::SplitPane {
                direction,
                command,
                command_is_direct,
                focus_new_pane,
                delay_ms,
                split_percent,
                title,
                ..
            } => self.execute_split_pane_action(
                direction,
                command,
                command_is_direct,
                focus_new_pane,
                delay_ms,
                split_percent,
                title,
            ),
            CustomActionConfig::KeySequence { keys, title, .. } => {
                self.execute_key_sequence_action(keys, title)
            }
            CustomActionConfig::Sequence { steps, .. } => {
                let ctx = Arc::clone(&self.last_workflow_context);
                self.execute_sequence_sync(steps, &ctx);
                true
            }
            CustomActionConfig::Condition {
                check,
                on_true_id,
                on_false_id,
                ..
            } => {
                let on_true = on_true_id.as_deref().map(|s| s.to_string());
                let on_false = on_false_id.as_deref().map(|s| s.to_string());
                self.execute_condition_standalone(&check, on_true.as_deref(), on_false.as_deref());
                true
            }
            CustomActionConfig::Repeat {
                action_id,
                count,
                delay_ms,
                stop_on_success,
                stop_on_failure,
                ..
            } => {
                let ctx = Arc::clone(&self.last_workflow_context);
                self.execute_repeat(
                    &action_id,
                    count,
                    delay_ms,
                    stop_on_success,
                    stop_on_failure,
                    ctx,
                );
                true
            }
        }
    }
}

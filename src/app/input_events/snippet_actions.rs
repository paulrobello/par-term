//! Snippet and custom action execution helpers for WindowState keybindings.

use crate::app::window_state::WindowState;
use crate::config::snippets::{CustomActionConfig, normalize_action_prefix_char};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};

const CUSTOM_ACTION_PREFIX_TOAST: &str = "Actions: prefix... (Esc to cancel)";
const NEW_TAB_COMMAND_DELAY_MS: u64 = 200;

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
            return false;
        }

        let matcher = crate::keybindings::KeybindingMatcher::from_event_with_remapping(
            event,
            &self.input_handler.modifiers,
            &self.config.modifier_remapping,
        );

        if matcher.matches_with_physical_preference(prefix_combo, self.config.use_physical_keys) {
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
            // try_lock: intentional — execute_snippet called from keybinding handler in
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
        // Find the action by ID
        let action = match self.config.actions.iter().find(|a| a.id() == action_id) {
            Some(a) => a,
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
                ..
            } => {
                // Clone values for the spawned thread
                let command = command.clone();
                let args = args.clone();
                let notify_on_success = *notify_on_success;
                let timeout_secs = *timeout_secs;
                let title = title.clone();

                log::info!(
                    "Executing shell command '{}' (timeout={}s): {} {}",
                    title,
                    timeout_secs,
                    command,
                    args.join(" ")
                );

                // DEFERRED: Command allowlist for restricted deployments.
                //
                // In enterprise/kiosk environments it may be desirable to restrict
                // which executables can be invoked through custom shell-command
                // actions, in addition to the per-action configuration already
                // enforced by the keybinding system. A suggested approach:
                //
                //   1. Add an `allowed_commands: Option<Vec<String>>` field to
                //      `Config` (absent means "allow all").
                //   2. Before spawning, resolve `command` to its canonical path
                //      with `std::fs::canonicalize` and check it against the list.
                //   3. Return `false` (with a toast notification) if the canonical
                //      path is not on the allowlist.
                //
                // Intentionally deferred: requires a policy decision about how the
                // allowlist is managed (config file, environment variable, MDM
                // profile, etc.) that is out of scope for the current sprint.
                // Track as a GitHub issue with the "enterprise" and "security" labels
                // before implementing. Relates to SEC-002 (bypassable command denylist)
                // and ARC-011 (optional feature flags for enterprise deployment).

                // Spawn a background thread to avoid blocking the main event loop
                std::thread::spawn(move || {
                    let timeout = std::time::Duration::from_secs(timeout_secs);
                    let start = std::time::Instant::now();

                    // Use spawn to run the command and wait with timeout
                    let child_result = std::process::Command::new(&command).args(&args).spawn();

                    match child_result {
                        Ok(mut child) => {
                            // Poll for completion with timeout
                            loop {
                                match child.try_wait() {
                                    Ok(Some(status)) => {
                                        let elapsed = start.elapsed();
                                        if status.success() {
                                            log::info!(
                                                "Shell command '{}' completed successfully in {:.2}s",
                                                title,
                                                elapsed.as_secs_f64()
                                            );
                                            if notify_on_success {
                                                log::info!(
                                                    "Command '{}' output available (check terminal or logs)",
                                                    title
                                                );
                                            }
                                        } else {
                                            log::error!(
                                                "Shell command '{}' failed with status: {} after {:.2}s",
                                                title,
                                                status,
                                                elapsed.as_secs_f64()
                                            );
                                        }
                                        break;
                                    }
                                    Ok(None) => {
                                        // Still running, check timeout
                                        if start.elapsed() > timeout {
                                            log::error!(
                                                "Shell command '{}' timed out after {}s, terminating",
                                                title,
                                                timeout_secs
                                            );
                                            let _ = child.kill();
                                            let _ = child.wait();
                                            break;
                                        }
                                        // Small sleep to avoid busy-waiting
                                        std::thread::sleep(std::time::Duration::from_millis(50));
                                    }
                                    Err(e) => {
                                        log::error!(
                                            "Shell command '{}' error checking status: {}",
                                            title,
                                            e
                                        );
                                        break;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to spawn shell command '{}': {}", title, e);
                        }
                    }
                });

                // Return immediately - command is running in background
                true
            }
            CustomActionConfig::NewTab { command, title, .. } => {
                let command = command.clone();
                let title = title.clone();
                let tab_count_before = self.tab_manager.tab_count();
                self.new_tab();

                let opened_new_tab = self.tab_manager.tab_count() > tab_count_before;
                if !opened_new_tab {
                    log::warn!("NewTab action '{}' did not open a tab", title);
                    return false;
                }

                if let Some(command) = command.filter(|cmd| !cmd.trim().is_empty())
                    && let Some(tab) = self.tab_manager.active_tab()
                {
                    let text_with_nl = format!("{}\n", command);
                    let terminal = std::sync::Arc::clone(&tab.terminal);
                    let title = title.clone();

                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(
                            NEW_TAB_COMMAND_DELAY_MS,
                        ));

                        // try_write: background thread; on contention skip the write.
                        // Shell may not be ready yet — user can re-run the action.
                        if let Ok(term) = terminal.try_write()
                            && let Err(e) = term.write(text_with_nl.as_bytes())
                        {
                            log::error!("NewTab action '{}' write failed: {}", title, e);
                        }
                    });
                }

                true
            }
            CustomActionConfig::InsertText {
                text, variables, ..
            } => {
                // Substitute variables
                let substituted_text =
                    match crate::snippets::VariableSubstitutor::new().substitute(text, variables) {
                        Ok(content) => content,
                        Err(e) => {
                            log::error!("Failed to substitute variables in action: {}", e);
                            self.show_toast(format!("Action Error: {}", e));
                            return false;
                        }
                    };

                // Write to the active terminal
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    // try_lock: intentional — execute_custom_action runs from keybinding
                    // handler in sync event loop. On miss: the action text is not written.
                    // Logged as an error so the user is aware; they can retry the keybinding.
                    if let Ok(terminal) = tab.terminal.try_write() {
                        if let Err(e) = terminal.write(substituted_text.as_bytes()) {
                            log::error!("Failed to write action text to terminal: {}", e);
                            return false;
                        }

                        log::info!("Executed insert text action");
                        return true;
                    } else {
                        log::error!("Failed to lock terminal for action execution");
                        return false;
                    }
                }

                false
            }
            CustomActionConfig::SplitPane {
                direction,
                command,
                command_is_direct,
                focus_new_pane,
                delay_ms,
                split_percent,
                title,
                ..
            } => {
                use crate::config::snippets::ActionSplitDirection;

                let pane_direction = match direction {
                    ActionSplitDirection::Horizontal => crate::pane::SplitDirection::Horizontal,
                    ActionSplitDirection::Vertical => crate::pane::SplitDirection::Vertical,
                };
                let focus = *focus_new_pane;
                let is_direct = *command_is_direct;
                let command = command.clone();
                let delay = *delay_ms;
                let percent = *split_percent;
                let title = title.clone();

                crate::debug_info!(
                    "TAB_ACTION",
                    "SplitPane action '{}' direction={:?} focus_new={} direct={}",
                    title,
                    pane_direction,
                    focus,
                    is_direct
                );

                // For direct commands, parse argv and pass as the pane's initial process.
                let initial_command = if is_direct {
                    command.as_deref().map(|cmd_str| {
                        let mut parts = cmd_str.split_whitespace();
                        let cmd = parts.next().unwrap_or("").to_string();
                        let args: Vec<String> = parts.map(|s| s.to_string()).collect();
                        (cmd, args)
                    })
                } else {
                    None
                };

                let new_pane_id =
                    self.split_pane_direction(pane_direction, focus, initial_command, percent);

                // For shell-mode commands, send text to the new pane after a delay.
                if !is_direct && let (Some(pane_id), Some(text)) = (new_pane_id, command) {
                    let text_with_nl = format!("{}\n", text);
                    if let Some(tab) = self.tab_manager.active_tab()
                        && let Some(pm) = tab.pane_manager()
                        && let Some(pane) = pm.get_pane(pane_id)
                    {
                        let terminal = std::sync::Arc::clone(&pane.terminal);
                        std::thread::spawn(move || {
                            if delay > 0 {
                                std::thread::sleep(std::time::Duration::from_millis(delay));
                            }
                            // try_write: background thread; on contention skip the write.
                            // Shell may not be ready — user can retry the keybinding.
                            if let Ok(term) = terminal.try_write()
                                && let Err(e) = term.write(text_with_nl.as_bytes())
                            {
                                log::error!(
                                    "SplitPane action '{}' write failed for pane {}: {}",
                                    title,
                                    pane_id,
                                    e
                                );
                            }
                        });
                    }
                }

                new_pane_id.is_some()
            }
            CustomActionConfig::KeySequence { keys, title, .. } => {
                use crate::keybindings::parse_key_sequence;

                let byte_sequences = match parse_key_sequence(keys) {
                    Ok(seqs) => seqs,
                    Err(e) => {
                        log::error!("Invalid key sequence '{}': {}", keys, e);
                        self.show_toast(format!("Invalid key sequence: {}", e));
                        return false;
                    }
                };

                // Write all key sequences to the terminal
                let write_error = if let Some(tab) = self.tab_manager.active_tab_mut() {
                    // try_lock: intentional — send_keys action in sync event loop.
                    // On miss: the key sequences are not written. User can retry.
                    if let Ok(terminal) = tab.terminal.try_write() {
                        let mut err: Option<String> = None;
                        for bytes in &byte_sequences {
                            if let Err(e) = terminal.write(bytes) {
                                err = Some(format!("{}", e));
                                break;
                            }
                        }
                        err
                    } else {
                        log::error!("Failed to lock terminal for key sequence execution");
                        return false;
                    }
                } else {
                    return false;
                };

                if let Some(e) = write_error {
                    log::error!("Failed to write key sequence: {}", e);
                    self.show_toast(format!("Key sequence error: {}", e));
                    return false;
                }

                log::info!(
                    "Executed key sequence action '{}' ({} keys)",
                    title,
                    byte_sequences.len()
                );
                true
            }
        }
    }
}

fn extract_prefix_action_char(event: &KeyEvent) -> Option<char> {
    event
        .text
        .as_ref()
        .and_then(|text| text.chars().next())
        .filter(|ch| !ch.is_whitespace())
        .or_else(|| match &event.logical_key {
            Key::Character(text) => text.chars().next().filter(|ch| !ch.is_whitespace()),
            _ => None,
        })
        .or(match event.physical_key {
            PhysicalKey::Code(code) => match code {
                KeyCode::KeyA => Some('a'),
                KeyCode::KeyB => Some('b'),
                KeyCode::KeyC => Some('c'),
                KeyCode::KeyD => Some('d'),
                KeyCode::KeyE => Some('e'),
                KeyCode::KeyF => Some('f'),
                KeyCode::KeyG => Some('g'),
                KeyCode::KeyH => Some('h'),
                KeyCode::KeyI => Some('i'),
                KeyCode::KeyJ => Some('j'),
                KeyCode::KeyK => Some('k'),
                KeyCode::KeyL => Some('l'),
                KeyCode::KeyM => Some('m'),
                KeyCode::KeyN => Some('n'),
                KeyCode::KeyO => Some('o'),
                KeyCode::KeyP => Some('p'),
                KeyCode::KeyQ => Some('q'),
                KeyCode::KeyR => Some('r'),
                KeyCode::KeyS => Some('s'),
                KeyCode::KeyT => Some('t'),
                KeyCode::KeyU => Some('u'),
                KeyCode::KeyV => Some('v'),
                KeyCode::KeyW => Some('w'),
                KeyCode::KeyX => Some('x'),
                KeyCode::KeyY => Some('y'),
                KeyCode::KeyZ => Some('z'),
                KeyCode::Digit0 => Some('0'),
                KeyCode::Digit1 => Some('1'),
                KeyCode::Digit2 => Some('2'),
                KeyCode::Digit3 => Some('3'),
                KeyCode::Digit4 => Some('4'),
                KeyCode::Digit5 => Some('5'),
                KeyCode::Digit6 => Some('6'),
                KeyCode::Digit7 => Some('7'),
                KeyCode::Digit8 => Some('8'),
                KeyCode::Digit9 => Some('9'),
                KeyCode::Minus => Some('-'),
                KeyCode::Equal => Some('='),
                KeyCode::BracketLeft => Some('['),
                KeyCode::BracketRight => Some(']'),
                KeyCode::Backslash => Some('\\'),
                KeyCode::Semicolon => Some(';'),
                KeyCode::Quote => Some('\''),
                KeyCode::Backquote => Some('`'),
                KeyCode::Comma => Some(','),
                KeyCode::Period => Some('.'),
                KeyCode::Slash => Some('/'),
                KeyCode::Space => Some(' '),
                _ => None,
            },
            _ => None,
        })
        .filter(|ch| !ch.is_whitespace())
}

#[cfg(test)]
mod tests {
    use super::{extract_prefix_action_char, prefix_action_for_char};
    use crate::config::snippets::CustomActionConfig;
    use std::collections::HashMap;
    use winit::event::{ElementState, KeyEvent};
    use winit::keyboard::{Key, KeyCode, KeyLocation, PhysicalKey};

    fn make_key_event(logical_key: Key, physical_key: PhysicalKey, text: Option<&str>) -> KeyEvent {
        unsafe {
            let mut event: KeyEvent = std::mem::zeroed();
            std::ptr::write(&mut event.physical_key, physical_key);
            std::ptr::write(&mut event.logical_key, logical_key);
            std::ptr::write(&mut event.text, text.map(Into::into));
            std::ptr::write(&mut event.location, KeyLocation::Standard);
            std::ptr::write(&mut event.state, ElementState::Pressed);
            std::ptr::write(&mut event.repeat, false);
            event
        }
    }

    #[test]
    fn prefix_action_matching_is_case_insensitive_for_letters() {
        let actions = vec![CustomActionConfig::InsertText {
            id: "git-status".to_string(),
            title: "Git Status".to_string(),
            text: "git status".to_string(),
            variables: HashMap::new(),
            keybinding: None,
            prefix_char: Some('G'),
            keybinding_enabled: true,
            description: None,
        }];

        assert_eq!(
            prefix_action_for_char(&actions, 'g'),
            Some("git-status".to_string())
        );
        assert_eq!(
            prefix_action_for_char(&actions, 'G'),
            Some("git-status".to_string())
        );
    }

    #[test]
    fn prefix_action_matching_keeps_symbol_bindings_exact() {
        let actions = vec![CustomActionConfig::KeySequence {
            id: "split".to_string(),
            title: "Split".to_string(),
            keys: "Ctrl+C".to_string(),
            keybinding: None,
            prefix_char: Some('%'),
            keybinding_enabled: true,
            description: None,
        }];

        assert_eq!(
            prefix_action_for_char(&actions, '%'),
            Some("split".to_string())
        );
        assert_eq!(prefix_action_for_char(&actions, '5'), None);
    }

    #[test]
    fn extract_prefix_action_char_prefers_event_text() {
        let event = make_key_event(
            Key::Named(winit::keyboard::NamedKey::Enter),
            PhysicalKey::Code(KeyCode::KeyR),
            Some("r"),
        );

        assert_eq!(extract_prefix_action_char(&event), Some('r'));
    }

    #[test]
    fn extract_prefix_action_char_falls_back_to_physical_key() {
        let event = make_key_event(
            Key::Named(winit::keyboard::NamedKey::Enter),
            PhysicalKey::Code(KeyCode::KeyR),
            None,
        );

        assert_eq!(extract_prefix_action_char(&event), Some('r'));
    }
}

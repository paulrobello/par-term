//! Snippet and custom action execution helpers for WindowState keybindings.

use crate::app::window_state::WindowState;

impl WindowState {
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
            if let Ok(terminal) = tab.terminal.try_lock() {
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
        use crate::config::snippets::CustomActionConfig;

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
                ..
            } => {
                log::info!("Executing shell command: {} {}", command, args.join(" "));

                // Execute the shell command
                let result = std::process::Command::new(command).args(args).output();

                match result {
                    Ok(output) => {
                        if output.status.success() {
                            if *notify_on_success {
                                let message =
                                    String::from_utf8_lossy(&output.stdout).trim().to_string();
                                self.show_toast(format!("Command completed: {}", message));
                            }
                            log::info!("Shell command completed successfully");
                            true
                        } else {
                            let error_msg =
                                String::from_utf8_lossy(&output.stderr).trim().to_string();
                            log::error!("Shell command failed: {}", error_msg);
                            self.show_toast(format!("Command failed: {}", error_msg));
                            false
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to execute shell command: {}", e);
                        self.show_toast(format!("Execution Error: {}", e));
                        false
                    }
                }
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
                    if let Ok(terminal) = tab.terminal.try_lock() {
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
                    if let Ok(terminal) = tab.terminal.try_lock() {
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

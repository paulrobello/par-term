//! Snippet and custom action execution helpers for WindowState keybindings.
//!
//! TODO(QA-004): This file is 1254 lines (limit: 800). When it exceeds the limit
//! in a future refactor, extract the per-action-type handlers from
//! `execute_custom_action` into separate modules:
//!
//!   shell_command.rs  — ShellCommand arm (fire-and-forget + capture_output paths)
//!   new_tab.rs        — NewTab arm (tab creation + delayed command write)
//!   split_pane.rs     — SplitPane arm (pane splitting + delayed command write)
//!   key_sequence.rs   — KeySequence arm (parsing + terminal write)
//!
//! Each handler would be a method on WindowState that takes the relevant fields
//! from CustomActionConfig and returns bool. Track in issue QA-004.

use crate::app::window_state::WindowState;
use crate::app::window_state::WorkflowContext;
use crate::config::snippets::{
    ConditionCheck, CustomActionConfig, SequenceStepBehavior, normalize_action_prefix_char,
};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};

const CUSTOM_ACTION_PREFIX_TOAST: &str = "Actions: prefix... (Esc to cancel)";
const NEW_TAB_COMMAND_DELAY_MS: u64 = 200;

/// QA-002: Maximum total delay (ms) allowed for a single Repeat or Sequence before
/// rejecting the action. This caps the event-loop freeze caused by `thread::sleep`
/// in the sync event loop. The proper fix is dispatching to a background Tokio task,
/// but that requires extracting all `&mut self` mutations into a command queue.
/// See AUDIT.md QA-002 for the full plan.
const MAX_TOTAL_DELAY_MS: u64 = 5_000;

fn prefix_action_for_char(actions: &[CustomActionConfig], input_char: char) -> Option<String> {
    let normalized_input = normalize_action_prefix_char(input_char);

    actions
        .iter()
        .find(|action| action.normalized_prefix_char() == Some(normalized_input))
        .map(|action| action.id().to_string())
}

/// Result of executing a single workflow step.
enum StepOutcome {
    /// Step completed successfully.
    Success,
    /// Step "failed" (ShellCommand non-zero exit, or Condition false).
    Failure,
    /// Unrecoverable error (action not found, circular reference); always halts.
    Abort,
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

    /// Execute an action as a workflow step and return a typed outcome.
    ///
    /// Returns:
    /// - `StepOutcome::Success` for most action types (they don't "fail")
    /// - `StepOutcome::Failure` if a `ShellCommand` with `capture_output` exits non-zero,
    ///   or if a `Condition` check evaluates to false
    /// - `StepOutcome::Abort` if the action is not found or a circular reference is detected
    fn execute_action_as_step(
        &mut self,
        action_id: &str,
        ctx: &Arc<Mutex<Option<WorkflowContext>>>,
        visited: &mut HashSet<String>,
    ) -> StepOutcome {
        if visited.contains(action_id) {
            self.show_toast(format!(
                "Workflow: circular reference detected ({})",
                action_id
            ));
            return StepOutcome::Abort;
        }

        let action = match self.config.actions.iter().find(|a| a.id() == action_id) {
            Some(a) => a.clone(),
            None => {
                self.show_toast(format!("Workflow: action '{}' not found", action_id));
                return StepOutcome::Abort;
            }
        };

        visited.insert(action_id.to_string());

        let outcome = match &action {
            CustomActionConfig::ShellCommand {
                capture_output,
                command,
                args,
                notify_on_success,
                timeout_secs: _,
                title,
                ..
            } => {
                if *capture_output {
                    // Run synchronously to get exit code for step outcome
                    let output_result = std::process::Command::new(command).args(args).output();
                    match output_result {
                        Ok(output) => {
                            let exit_code = output.status.code().unwrap_or(-1);
                            let mut combined = String::new();
                            combined.push_str(&String::from_utf8_lossy(&output.stdout));
                            combined.push_str(&String::from_utf8_lossy(&output.stderr));
                            if combined.len() > 65536 {
                                combined.truncate(65536);
                            }
                            let wf_ctx = WorkflowContext {
                                last_exit_code: Some(exit_code),
                                last_output: if combined.is_empty() {
                                    None
                                } else {
                                    Some(combined)
                                },
                            };
                            if let Ok(mut guard) = ctx.lock() {
                                *guard = Some(wf_ctx);
                            }
                            if output.status.success() {
                                log::info!("Step ShellCommand '{}' succeeded", title);
                                if *notify_on_success {
                                    log::info!("Command '{}' output captured", title);
                                }
                                StepOutcome::Success
                            } else {
                                log::error!(
                                    "Step ShellCommand '{}' failed with exit code {}",
                                    title,
                                    exit_code
                                );
                                StepOutcome::Failure
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to spawn step command '{}': {}", title, e);
                            StepOutcome::Abort
                        }
                    }
                } else {
                    // Fire-and-forget for non-capturing shell commands (always succeed from step perspective)
                    let id = action.id().to_string();
                    self.execute_custom_action(&id);
                    StepOutcome::Success
                }
            }
            CustomActionConfig::Condition { check, .. } => {
                if self.evaluate_condition_check(check, ctx) {
                    StepOutcome::Success
                } else {
                    StepOutcome::Failure
                }
            }
            CustomActionConfig::Sequence { steps, .. } => {
                // Use the same visited set so that cross-sequence cycles are detected.
                // action_id is already in visited (inserted above); any nested action that
                // directly or indirectly references it will be caught when execute_action_as_step
                // checks visited at entry.
                let steps = steps.clone();
                self.execute_sequence_steps(&steps, ctx, visited)
            }
            CustomActionConfig::Repeat {
                action_id: rep_id,
                count,
                delay_ms: rep_delay,
                stop_on_success,
                stop_on_failure,
                ..
            } => {
                // Use the same visited set so that cross-repeat cycles are detected.
                let rep_id = rep_id.clone();
                // QA-004: Clamp repeat count to prevent config-based DoS.
                // QA-002: Also cap total delay to MAX_TOTAL_DELAY_MS so the event loop
                // is never frozen for more than a few seconds. The count cap alone
                // doesn't bound delay time (100 iterations * 1s = 100s freeze).
                const MAX_SAFE_REPEAT_COUNT: u32 = 100;
                let count = (*count).min(MAX_SAFE_REPEAT_COUNT);
                let rep_delay = *rep_delay;
                let max_iterations_for_delay = if rep_delay > 0 {
                    (MAX_TOTAL_DELAY_MS / rep_delay).max(1) as u32
                } else {
                    count
                };
                let count = count.min(max_iterations_for_delay);
                let stop_on_success = *stop_on_success;
                let stop_on_failure = *stop_on_failure;
                let mut final_outcome = StepOutcome::Success;
                for i in 0..count {
                    let outcome = self.execute_action_as_step(&rep_id, ctx, visited);
                    // Reset visited between repetitions: re-entering the same action
                    // in the next iteration is not a cycle.
                    visited.clear();
                    match outcome {
                        StepOutcome::Abort => {
                            final_outcome = StepOutcome::Abort;
                            break;
                        }
                        StepOutcome::Success if stop_on_success => break,
                        StepOutcome::Failure if stop_on_failure => {
                            final_outcome = StepOutcome::Failure;
                            break;
                        }
                        _ => {}
                    }
                    if rep_delay > 0 && i < count - 1 {
                        std::thread::sleep(std::time::Duration::from_millis(rep_delay));
                    }
                }
                final_outcome
            }
            _ => {
                // InsertText, KeySequence, NewTab, SplitPane always succeed
                let id = action.id().to_string();
                self.execute_custom_action(&id);
                StepOutcome::Success
            }
        };

        visited.remove(action_id);
        outcome
    }

    /// Evaluate a `ConditionCheck` and return true if the check passes.
    fn evaluate_condition_check(
        &self,
        check: &ConditionCheck,
        ctx: &Arc<Mutex<Option<WorkflowContext>>>,
    ) -> bool {
        match check {
            ConditionCheck::ExitCode { value } => {
                if let Ok(guard) = ctx.lock()
                    && let Some(wf_ctx) = guard.as_ref()
                {
                    return wf_ctx.last_exit_code == Some(*value);
                }
                false
            }
            ConditionCheck::OutputContains {
                pattern,
                case_sensitive,
            } => {
                if let Ok(guard) = ctx.lock()
                    && let Some(wf_ctx) = guard.as_ref()
                    && let Some(output) = &wf_ctx.last_output
                {
                    return if *case_sensitive {
                        output.contains(pattern.as_str())
                    } else {
                        output.to_lowercase().contains(&pattern.to_lowercase())
                    };
                }
                false
            }
            ConditionCheck::EnvVar { name, value } => match std::env::var(name) {
                Ok(env_val) => {
                    if let Some(expected) = value {
                        &env_val == expected
                    } else {
                        true // existence check
                    }
                }
                Err(_) => false,
            },
            ConditionCheck::DirMatches { pattern } => {
                // Use the terminal's reported CWD (from shell integration / OSC 7) stored
                // in session variables, rather than par-term's own process CWD.
                let cwd = {
                    let vars = self.badge_state.variables.read();
                    vars.path.clone()
                };
                let cwd = if cwd.is_empty() {
                    // Fallback to process CWD if shell has not yet reported its path.
                    std::env::current_dir()
                        .ok()
                        .and_then(|p| p.to_str().map(|s| s.to_string()))
                        .unwrap_or_default()
                } else {
                    cwd
                };
                glob_match(pattern, &cwd)
            }
            ConditionCheck::GitBranch { pattern } => {
                // Run git in the terminal's CWD so the branch reflects the active shell's
                // repository, not par-term's own process directory.
                let cwd = {
                    let vars = self.badge_state.variables.read();
                    vars.path.clone()
                };
                let mut cmd = std::process::Command::new("git");
                cmd.args(["rev-parse", "--abbrev-ref", "HEAD"]);
                if !cwd.is_empty() {
                    cmd.current_dir(&cwd);
                }
                let branch = cmd
                    .output()
                    .ok()
                    .and_then(|o| String::from_utf8(o.stdout).ok())
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default();
                glob_match(pattern, &branch)
            }
        }
    }

    /// Execute sequence steps synchronously (called from event loop thread or background thread).
    ///
    /// # Blocking note (QA-002)
    ///
    /// This method is called from the event loop thread. Steps with `delay_ms > 0` will
    /// block the event loop for the delay duration. Total delay is capped at
    /// `MAX_TOTAL_DELAY_MS` to prevent extended UI freezes. The proper fix is to
    /// dispatch to a background Tokio task communicating via mpsc, but that requires
    /// extracting all `&mut self` mutations into a command queue (deferred).
    fn execute_sequence_sync(
        &mut self,
        steps: Vec<par_term_config::snippets::SequenceStep>,
        ctx: &Arc<Mutex<Option<WorkflowContext>>>,
    ) {
        let mut visited: HashSet<String> = HashSet::new();
        self.execute_sequence_steps(&steps, ctx, &mut visited);
    }

    /// Core sequence execution loop. Accepts an external `visited` set so that cycle
    /// detection is shared across nested Sequence and Repeat actions within a single
    /// workflow execution. The `visited` set grows as actions are entered and shrinks
    /// as they return, allowing the same action to appear in separate (non-nested) steps.
    ///
    /// Returns:
    /// - `StepOutcome::Abort` — a step aborted (missing action, circular ref), or a step
    ///   failed with `on_failure = Abort` (toast already shown)
    /// - `StepOutcome::Failure` — a step failed with `on_failure = Stop` (silent early exit)
    /// - `StepOutcome::Success` — all steps completed (including any `Continue`-on-failure steps)
    fn execute_sequence_steps(
        &mut self,
        steps: &[par_term_config::snippets::SequenceStep],
        ctx: &Arc<Mutex<Option<WorkflowContext>>>,
        visited: &mut HashSet<String>,
    ) -> StepOutcome {
        // QA-002: Track cumulative delay to cap total event-loop freeze time.
        let mut total_delay_ms: u64 = 0;
        for step in steps {
            if step.delay_ms > 0 {
                // QA-002: Cap per-step delay so total never exceeds MAX_TOTAL_DELAY_MS.
                let remaining = MAX_TOTAL_DELAY_MS.saturating_sub(total_delay_ms);
                if remaining == 0 {
                    log::warn!(
                        "Sequence: total delay cap ({MAX_TOTAL_DELAY_MS}ms) reached, \
                         skipping remaining steps"
                    );
                    break;
                }
                let actual_delay = step.delay_ms.min(remaining);
                total_delay_ms += actual_delay;
                std::thread::sleep(std::time::Duration::from_millis(actual_delay));
            }

            let outcome = self.execute_action_as_step(&step.action_id, ctx, visited);

            match outcome {
                StepOutcome::Abort => {
                    // Already showed toast in execute_action_as_step
                    return StepOutcome::Abort;
                }
                StepOutcome::Success => {
                    // Continue to next step
                }
                StepOutcome::Failure => {
                    match step.on_failure {
                        SequenceStepBehavior::Abort => {
                            self.show_toast(format!(
                                "Workflow: step '{}' failed, aborting sequence",
                                step.action_id
                            ));
                            return StepOutcome::Abort;
                        }
                        SequenceStepBehavior::Stop => {
                            return StepOutcome::Failure; // silent stop, propagate as failure
                        }
                        SequenceStepBehavior::Continue => {
                            // continue to next step
                        }
                    }
                }
            }
        }
        StepOutcome::Success
    }

    /// Execute a Condition action when triggered directly (not inside a Sequence).
    fn execute_condition_standalone(
        &mut self,
        check: &ConditionCheck,
        on_true_id: Option<&str>,
        on_false_id: Option<&str>,
    ) {
        let ctx = Arc::clone(&self.last_workflow_context);
        let result = self.evaluate_condition_check(check, &ctx);
        let target_id = if result { on_true_id } else { on_false_id };
        if let Some(id) = target_id {
            let id = id.to_string();
            self.execute_custom_action(&id);
        }
    }

    /// Execute a Repeat action: run action_id up to count times with optional delay.
    ///
    /// # Blocking note (QA-002)
    ///
    /// This method is called from the event loop thread. Iterations with `delay_ms > 0` will
    /// block the event loop for the delay duration. Total delay is capped at
    /// `MAX_TOTAL_DELAY_MS` to prevent extended UI freezes. The proper fix is to dispatch
    /// to a background Tokio task communicating via mpsc, but that requires extracting all
    /// `&mut self` mutations into a command queue (deferred).
    fn execute_repeat(
        &mut self,
        action_id: &str,
        count: u32,
        delay_ms: u64,
        stop_on_success: bool,
        stop_on_failure: bool,
        ctx: Arc<Mutex<Option<WorkflowContext>>>,
    ) {
        // QA-002: Cap total delay to MAX_TOTAL_DELAY_MS.
        let count = if delay_ms > 0 {
            count.min((MAX_TOTAL_DELAY_MS / delay_ms).max(1) as u32)
        } else {
            count
        };

        let mut visited: HashSet<String> = HashSet::new();

        for i in 0..count {
            let outcome = self.execute_action_as_step(action_id, &ctx, &mut visited);
            visited.clear(); // Reset visited between repetitions to allow re-entry

            match outcome {
                StepOutcome::Abort => break,
                StepOutcome::Success => {
                    if stop_on_success {
                        break;
                    }
                }
                StepOutcome::Failure => {
                    if stop_on_failure {
                        break;
                    }
                }
            }

            // Sleep between iterations (not after the last one)
            if delay_ms > 0 && i < count - 1 {
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            }
        }
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
                capture_output,
                ..
            } => {
                // Clone values for the spawned thread
                let command = command.clone();
                let args = args.clone();
                let notify_on_success = *notify_on_success;
                let timeout_secs = *timeout_secs;
                let title = title.clone();
                let capture_output = *capture_output;
                let ctx_arc = Arc::clone(&self.last_workflow_context);

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

                    if capture_output {
                        // Collect stdout+stderr, cap at 64KB
                        let output_result =
                            std::process::Command::new(&command).args(&args).output();

                        match output_result {
                            Ok(output) => {
                                let exit_code = output.status.code().unwrap_or(-1);
                                let mut combined = String::new();
                                combined.push_str(&String::from_utf8_lossy(&output.stdout));
                                combined.push_str(&String::from_utf8_lossy(&output.stderr));
                                if combined.len() > 65536 {
                                    combined.truncate(65536);
                                }
                                let ctx = WorkflowContext {
                                    last_exit_code: Some(exit_code),
                                    last_output: if combined.is_empty() {
                                        None
                                    } else {
                                        Some(combined)
                                    },
                                };
                                if let Ok(mut guard) = ctx_arc.lock() {
                                    *guard = Some(ctx);
                                }
                                if output.status.success() {
                                    log::info!("Shell command '{}' completed successfully", title);
                                    if notify_on_success {
                                        log::info!("Command '{}' output available", title);
                                    }
                                } else {
                                    log::error!(
                                        "Shell command '{}' failed with exit code {}",
                                        title,
                                        exit_code
                                    );
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to spawn shell command '{}': {}", title, e);
                            }
                        }
                    } else {
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
                                            std::thread::sleep(std::time::Duration::from_millis(
                                                50,
                                            ));
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
            CustomActionConfig::Sequence { steps, .. } => {
                let steps = steps.clone();
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
                let check = check.clone();
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
                let action_id = action_id.clone();
                let count = *count;
                let delay_ms = *delay_ms;
                let stop_on_success = *stop_on_success;
                let stop_on_failure = *stop_on_failure;
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

/// Simple glob pattern matching (supports `*` as wildcard, no `?` or `[` brackets).
fn glob_match(pattern: &str, text: &str) -> bool {
    // Fast path: no wildcard
    if !pattern.contains('*') {
        return pattern == text;
    }
    // Split pattern on '*' and check all parts are present in order
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut remaining = text;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            // First part must match the start
            if !remaining.starts_with(part) {
                return false;
            }
            remaining = &remaining[part.len()..];
        } else if i == parts.len() - 1 {
            // Last part must match the end
            return remaining.ends_with(part);
        } else {
            // Middle parts must appear somewhere
            if let Some(pos) = remaining.find(part) {
                remaining = &remaining[pos + part.len()..];
            } else {
                return false;
            }
        }
    }
    true
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
    use super::{extract_prefix_action_char, glob_match, prefix_action_for_char};
    use crate::config::snippets::CustomActionConfig;
    use std::collections::HashMap;
    use winit::event::{ElementState, KeyEvent};
    use winit::keyboard::{Key, KeyCode, KeyLocation, PhysicalKey};

    fn make_key_event(logical_key: Key, physical_key: PhysicalKey, text: Option<&str>) -> KeyEvent {
        // SEC-006: Construct a KeyEvent for tests by setting each field individually.
        //
        // `KeyEvent` in winit 0.30.x does not expose a public constructor. We use
        // `std::mem::MaybeUninit` as a safe intermediate: the backing store is
        // allocated but not zeroed, and each field is written via `std::ptr::write`
        // before the value is read.
        //
        // SAFETY: `KeyEvent` comprises primitive types (bool), enums with a zero
        // discriminant (ElementState), Option<SmolStr>, Key, PhysicalKey, and
        // KeyLocation — none of which are invalid when their bytes are set via
        // `std::ptr::write`. All six public fields are written before the value
        // is assumed-initialized via `assume_init()`. No raw pointers, NonNull,
        // or NonZero* fields exist in KeyEvent.
        //
        // If winit adds a public constructor in a future release, prefer that
        // over this workaround. See also winit 0.30.13 `KeyEvent` definition.
        unsafe {
            let mut event: std::mem::MaybeUninit<KeyEvent> = std::mem::MaybeUninit::uninit();
            let ptr = event.as_mut_ptr();
            std::ptr::write(std::ptr::addr_of_mut!((*ptr).physical_key), physical_key);
            std::ptr::write(std::ptr::addr_of_mut!((*ptr).logical_key), logical_key);
            std::ptr::write(std::ptr::addr_of_mut!((*ptr).text), text.map(Into::into));
            std::ptr::write(std::ptr::addr_of_mut!((*ptr).location), KeyLocation::Standard);
            std::ptr::write(std::ptr::addr_of_mut!((*ptr).state), ElementState::Pressed);
            std::ptr::write(std::ptr::addr_of_mut!((*ptr).repeat), false);
            event.assume_init()
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

    #[test]
    fn test_glob_match_exact() {
        assert!(glob_match("main", "main"));
        assert!(!glob_match("main", "master"));
    }

    #[test]
    fn test_glob_match_wildcard() {
        assert!(glob_match("feat/*", "feat/login"));
        assert!(glob_match("*", "anything"));
        assert!(!glob_match("feat/*", "fix/bug"));
    }
}

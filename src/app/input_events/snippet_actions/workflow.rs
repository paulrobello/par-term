//! Workflow engine: sequence execution, repeat, condition evaluation, and glob matching.
//!
//! These methods are all on `WindowState` and are used by the main action dispatch
//! in `mod.rs` and by each other for recursive workflow execution.

use crate::app::window_state::WindowState;
use crate::app::window_state::WorkflowContext;
use crate::config::snippets::{ConditionCheck, CustomActionConfig, SequenceStepBehavior};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// Result of executing a single workflow step.
pub(crate) enum StepOutcome {
    /// Step completed successfully.
    Success,
    /// Step "failed" (ShellCommand non-zero exit, or Condition false).
    Failure,
    /// Unrecoverable error (action not found, circular reference); always halts.
    Abort,
}

/// QA-002: Maximum total delay (ms) allowed for a single Repeat or Sequence before
/// rejecting the action. This caps the event-loop freeze caused by `thread::sleep`
/// in the sync event loop. The proper fix is dispatching to a background Tokio task,
/// but that requires extracting all `&mut self` mutations into a command queue.
/// See AUDIT.md QA-002 for the full plan.
pub(crate) const MAX_TOTAL_DELAY_MS: u64 = 5_000;

impl WindowState {
    /// Execute an action as a workflow step and return a typed outcome.
    ///
    /// Returns:
    /// - `StepOutcome::Success` for most action types (they don't "fail")
    /// - `StepOutcome::Failure` if a `ShellCommand` with `capture_output` exits non-zero,
    ///   or if a `Condition` check evaluates to false
    /// - `StepOutcome::Abort` if the action is not found or a circular reference is detected
    pub(crate) fn execute_action_as_step(
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
    pub(crate) fn evaluate_condition_check(
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
    pub(crate) fn execute_sequence_sync(
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
    /// - `StepOutcome::Abort` -- a step aborted (missing action, circular ref), or a step
    ///   failed with `on_failure = Abort` (toast already shown)
    /// - `StepOutcome::Failure` -- a step failed with `on_failure = Stop` (silent early exit)
    /// - `StepOutcome::Success` -- all steps completed (including any `Continue`-on-failure steps)
    pub(crate) fn execute_sequence_steps(
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
    pub(crate) fn execute_condition_standalone(
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
    pub(crate) fn execute_repeat(
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
}

/// Simple glob pattern matching (supports `*` as wildcard, no `?` or `[` brackets).
pub(crate) fn glob_match(pattern: &str, text: &str) -> bool {
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

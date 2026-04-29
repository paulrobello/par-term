//! Trigger action dispatch for WindowState.
//!
//! This module handles polling trigger action results from the core library
//! and executing frontend-handled actions: RunCommand, PlaySound, and SendText.
//!
//! ## Sub-modules
//!
//! - `mark_line` — MarkLine result deduplication and application
//! - `sound` — Audio playback for PlaySound trigger actions
//!
//! ## Security
//!
//! Triggers fire on terminal output pattern matches, which means an attacker
//! controlling terminal output (e.g., `cat malicious_file`) could trigger
//! arbitrary command execution. To mitigate this:
//!
//! 1. **`prompt_before_run` flag** (default: `true`): When set, dangerous
//!    actions (`RunCommand`, `SendText`) are queued in `TriggerState::pending_trigger_actions`
//!    and presented to the user via a confirmation dialog before execution. Users must
//!    explicitly approve (once or always) each action. Setting `prompt_before_run: false`
//!    bypasses the dialog and executes immediately.
//!
//! 2. **Command denylist**: Even when `prompt_before_run` is `false`,
//!    `RunCommand` actions are checked against a denylist of dangerous
//!    patterns (rm -rf, curl|bash, eval, etc.).
//!
//! 3. **Rate limiting**: Dangerous actions from output triggers are rate-limited
//!    to prevent malicious output flooding from rapid-fire execution.
//!
//! 4. **Process management**: RunCommand spawns are tracked and limited to prevent
//!    resource exhaustion. Output is redirected to null to prevent terminal corruption.

mod mark_line;
mod sound;

use std::collections::HashMap;
use std::process::Stdio;
use std::time::Instant;

use par_term_config::check_command_denylist;

/// Maximum number of concurrent trigger-spawned processes allowed.
/// This prevents resource exhaustion from rapid-fire triggers.
const MAX_TRIGGER_PROCESSES: usize = 10;

/// Maximum age (in seconds) for tracked processes before cleanup.
/// Processes older than this are assumed to have completed.
const PROCESS_CLEANUP_AGE_SECS: u64 = 300; // 5 minutes

use par_term_emu_core_rust::terminal::ActionResult;

use crate::config::automation::TriggerActionConfig;

use super::window_state::WindowState;

/// (grid_row, label, color) tuple for a pending MarkLine action.
/// Shared between `mod.rs` (where it's constructed) and `mark_line.rs` (where it's consumed).
type MarkLineEntry = (usize, Option<String>, Option<(u8, u8, u8)>);

/// Expand a leading `~/` to the user's home directory.
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest).to_string_lossy().to_string();
    }
    path.to_string()
}

/// Shared context for dispatching a single trigger action.
///
/// Groups the per-frame data that every action handler needs so we can
/// pass it as a single argument instead of threading four separate
/// `HashMap` references through every helper.
struct DispatchContext<'a> {
    trigger_prompt_before_run: &'a HashMap<u64, bool>,
    approved_this_frame: &'a std::collections::HashSet<u64>,
    trigger_names: &'a HashMap<u64, String>,
    trigger_split_percent: &'a HashMap<u64, u8>,
}

impl WindowState {
    /// Check for trigger action results and dispatch them.
    ///
    /// Called each frame after check_bell(). Polls the core library for
    /// ActionResult events and executes the appropriate frontend action.
    ///
    /// Security restrictions are enforced for dangerous actions:
    /// - `prompt_before_run` flag queues RunCommand/SendText for dialog confirmation
    /// - Command denylist blocks obviously dangerous RunCommand patterns
    /// - Rate limiting prevents rapid-fire dangerous action execution
    pub(crate) fn check_trigger_actions(&mut self) {
        let tab = if let Some(t) = self.tab_manager.active_tab() {
            t
        } else {
            return;
        };

        // Poll action results and custom session variables from core terminal.
        // Also grab the current scrollback_len so our absolute line calculations
        // are consistent with the row values the trigger system produced.
        // try_lock: intentional — trigger polling in about_to_wait (sync event loop).
        // On miss: triggers are not processed this frame; they will be on the next poll.
        let (mut action_results, current_scrollback_len, custom_vars) =
            if let Ok(term) = tab.terminal.try_write() {
                let ar = term.poll_action_results();
                let sl = term.scrollback_len();
                let cv = term.custom_session_variables();
                (ar, sl, cv)
            } else {
                return;
            };

        // Sync custom session variables from core (set by SetVariable triggers)
        // to the frontend badge state. Values are trimmed because the core
        // captures the full terminal row which may include trailing padding.
        if !custom_vars.is_empty() {
            let mut changed = false;
            let mut vars = self.badge_state.variables_mut();
            for (name, value) in &custom_vars {
                let trimmed = value.trim();
                if vars.custom.get(name).map(|v| v.as_str()) != Some(trimmed) {
                    log::debug!(
                        "Trigger SetVariable synced to badge: {}='{}'",
                        name,
                        trimmed
                    );
                    vars.custom.insert(name.clone(), trimmed.to_string());
                    changed = true;
                }
            }
            drop(vars);
            if changed {
                self.badge_state.mark_dirty();
            }
        }

        // Drain dialog-approved actions from previous frame (dialog ran last frame).
        // Pre-populate approved_this_frame with their IDs so they bypass the prompt check.
        let mut approved_this_frame: std::collections::HashSet<u64> =
            std::collections::HashSet::new();
        if !self.trigger_state.approved_pending_actions.is_empty() {
            let mut pre_approved: Vec<ActionResult> = self
                .trigger_state
                .approved_pending_actions
                .drain(..)
                .collect();
            for action in &pre_approved {
                let tid = match action {
                    ActionResult::RunCommand { trigger_id, .. }
                    | ActionResult::SendText { trigger_id, .. }
                    | ActionResult::SplitPane { trigger_id, .. } => Some(*trigger_id),
                    _ => None,
                };
                if let Some(id) = tid {
                    approved_this_frame.insert(id);
                }
            }
            // Prepend pre-approved to action_results so they execute this frame
            pre_approved.extend(action_results);
            action_results = pre_approved;
        }

        if action_results.is_empty() {
            return;
        }

        // Snapshot trigger_prompt_before_run flags so we can check them without
        // re-borrowing the tab inside the action loop.
        let trigger_prompt_before_run: std::collections::HashMap<u64, bool> =
            tab.scripting.trigger_prompt_before_run.clone();

        // Snapshot trigger names for use in dialog descriptions.
        // Uses the TerminalManager::trigger_names() helper which internally locks
        // the core terminal synchronously (safe: called from the sync event loop).
        let trigger_names: std::collections::HashMap<u64, String> =
            if let Ok(term) = tab.terminal.try_read() {
                term.trigger_names()
            } else {
                std::collections::HashMap::new()
            };

        // Build trigger_split_percent map: trigger_id → split_percent from config.
        // Correlates the trigger name (from core) with the TriggerActionConfig.
        let trigger_split_percent: std::collections::HashMap<u64, u8> = trigger_names
            .iter()
            .filter_map(|(&id, name)| {
                self.config
                    .triggers
                    .iter()
                    .find(|t| &t.name == name)
                    .and_then(|t| {
                        t.actions.iter().find_map(|a| {
                            if let TriggerActionConfig::SplitPane { split_percent, .. } = a {
                                Some((id, *split_percent))
                            } else {
                                None
                            }
                        })
                    })
            })
            .collect();

        // Collect MarkLine events for batch deduplication (processed after the loop).
        // Between frames, the core may fire the same trigger multiple times for the
        // same physical line (once per PTY read). Each scan records a different grid
        // row because scrollback grows between scans, but we only get the scrollback_len
        // at poll time. Batch dedup clusters these into one mark per physical line.
        let mut pending_marks: HashMap<u64, Vec<MarkLineEntry>> = HashMap::new();

        let ctx = DispatchContext {
            trigger_prompt_before_run: &trigger_prompt_before_run,
            approved_this_frame: &approved_this_frame,
            trigger_names: &trigger_names,
            trigger_split_percent: &trigger_split_percent,
        };

        for action in action_results {
            self.dispatch_trigger_action(action, &ctx, &mut pending_marks);
        }

        // Periodically clean up stale rate limiter entries (every ~60 seconds of entries)
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.scripting.trigger_rate_limiter.cleanup(60);
        }

        // Process collected MarkLine events with deduplication.
        if !pending_marks.is_empty() {
            self.apply_mark_line_results(pending_marks, current_scrollback_len);
        }
    }

    /// Dispatch a single trigger action result to the appropriate handler.
    fn dispatch_trigger_action(
        &mut self,
        action: ActionResult,
        ctx: &DispatchContext<'_>,
        pending_marks: &mut HashMap<u64, Vec<MarkLineEntry>>,
    ) {
        match action {
            ActionResult::RunCommand {
                trigger_id,
                command,
                args,
            } => {
                let command = expand_tilde(&command);
                let args: Vec<String> = args.iter().map(|a| expand_tilde(a)).collect();
                self.handle_run_command_action(trigger_id, command, args, ctx);
            }
            ActionResult::PlaySound {
                trigger_id,
                sound_id,
                volume,
            } => {
                let sound_id = expand_tilde(&sound_id);
                log::info!(
                    "Trigger {} firing PlaySound: '{}' at volume {}",
                    trigger_id,
                    sound_id,
                    volume
                );
                if sound_id == "bell" || sound_id.is_empty() {
                    if let Some(tab) = self.tab_manager.active_tab()
                        && let Some(ref audio_bell) = tab.active_bell().audio
                    {
                        audio_bell.play(volume);
                    }
                } else {
                    Self::play_sound_file(&sound_id, volume);
                }
            }
            ActionResult::SendText {
                trigger_id,
                text,
                delay_ms,
            } => {
                self.handle_send_text_action(trigger_id, text, delay_ms, ctx);
            }
            ActionResult::Notify {
                trigger_id,
                title,
                message,
            } => {
                log::info!(
                    "Trigger {} firing Notify: '{}' - '{}'",
                    trigger_id,
                    title,
                    message
                );
                // Trigger notifications always deliver (bypass focus suppression)
                // since the user explicitly configured them
                self.deliver_notification_force(&title, &message);
            }
            ActionResult::SplitPane {
                trigger_id,
                direction,
                command,
                focus_new_pane,
                target,
                source_pane_id,
            } => {
                self.handle_split_pane_action(
                    trigger_id,
                    direction,
                    command,
                    focus_new_pane,
                    target,
                    source_pane_id,
                    ctx,
                );
            }
            ActionResult::MarkLine {
                trigger_id,
                row,
                label,
                color,
            } => {
                self.handle_mark_line_action(trigger_id, row, label, color, pending_marks);
            }
        }
    }

    /// Handle a RunCommand trigger action, including security checks and process spawn.
    fn handle_run_command_action(
        &mut self,
        trigger_id: u64,
        command: String,
        args: Vec<String>,
        ctx: &DispatchContext<'_>,
    ) {
        let prompt = ctx
            .trigger_prompt_before_run
            .get(&trigger_id)
            .copied()
            .unwrap_or(true);

        // SEC-001: If prompt_before_run is false but i_accept_the_risk is not set,
        // block execution and log an audit-level warning.
        if !prompt && !ctx.approved_this_frame.contains(&trigger_id) {
            let trigger_name = ctx
                .trigger_names
                .get(&trigger_id)
                .cloned()
                .unwrap_or_else(|| format!("trigger #{}", trigger_id));
            if self
                .config
                .unaccepted_risk_trigger_names
                .contains(&trigger_name)
            {
                log::warn!(
                    "Trigger '{}' (id={}) RunCommand BLOCKED: \
                     `prompt_before_run: false` requires `i_accept_the_risk: true` \
                     to execute dangerous actions without confirmation. \
                     Add `i_accept_the_risk: true` to this trigger or set \
                     `prompt_before_run: true`.",
                    trigger_name,
                    trigger_id,
                );
                crate::debug_error!(
                    "TRIGGER",
                    "AUDIT RunCommand BLOCKED trigger_id={} trigger_name={} \
                     reason=missing_i_accept_the_risk",
                    trigger_id,
                    trigger_name,
                );
                return;
            }
            // SEC-001: Log audit warning for every prompt_before_run:false execution.
            log::warn!(
                "SECURITY: Trigger '{}' (id={}) executing RunCommand without \
                 confirmation (prompt_before_run: false). \
                 command='{}' args={:?}",
                trigger_name,
                trigger_id,
                command,
                args,
            );
            crate::debug_info!(
                "TRIGGER",
                "AUDIT RunCommand no-prompt trigger_id={} trigger_name={} \
                 command={} args={:?}",
                trigger_id,
                trigger_name,
                command,
                args,
            );
        }

        if prompt
            && !self
                .trigger_state
                .always_allow_trigger_ids
                .contains(&trigger_id)
            && !ctx.approved_this_frame.contains(&trigger_id)
        {
            let trigger_name = ctx
                .trigger_names
                .get(&trigger_id)
                .cloned()
                .unwrap_or_else(|| format!("trigger #{}", trigger_id));
            let description = format!("Run command: {} {}", command, args.join(" "))
                .trim()
                .to_string();
            self.trigger_state.pending_trigger_actions.push(
                crate::app::window_state::PendingTriggerAction {
                    trigger_id,
                    trigger_name,
                    action: ActionResult::RunCommand {
                        trigger_id,
                        command,
                        args,
                    },
                    description,
                },
            );
            return;
        }

        // Security check: command denylist (always applied, even for approved actions)
        if let Some(denied_pattern) = check_command_denylist(&command, &args) {
            log::error!(
                "Trigger {} RunCommand DENIED: '{}' matches denylist pattern '{}'",
                trigger_id,
                command,
                denied_pattern,
            );
            return;
        }

        // Security check: rate limiting (skip for dialog-approved actions this frame)
        if !ctx.approved_this_frame.contains(&trigger_id)
            && let Some(tab) = self.tab_manager.active_tab_mut()
            && !tab
                .scripting
                .trigger_rate_limiter
                .check_and_update(trigger_id)
        {
            log::warn!(
                "Trigger {} RunCommand RATE-LIMITED: '{}' (too frequent)",
                trigger_id,
                command,
            );
            return;
        }

        log::info!(
            "Trigger {} firing RunCommand: {} {:?}",
            trigger_id,
            command,
            args
        );

        // Clean up old process entries (assume completed after timeout)
        let now = Instant::now();
        self.trigger_state
            .trigger_spawned_processes
            .retain(|_pid, spawn_time| {
                now.duration_since(*spawn_time).as_secs() < PROCESS_CLEANUP_AGE_SECS
            });

        // Check process limit to prevent resource exhaustion
        if self.trigger_state.trigger_spawned_processes.len() >= MAX_TRIGGER_PROCESSES {
            log::warn!(
                "Trigger {} RunCommand DENIED: max concurrent processes ({}) reached",
                trigger_id,
                MAX_TRIGGER_PROCESSES
            );
            return;
        }

        // Spawn process with stdout/stderr redirected to null to prevent
        // terminal corruption from inherited file descriptors
        match std::process::Command::new(&command)
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                let pid = child.id();
                log::debug!("RunCommand spawned successfully (PID={})", pid);
                // Security audit trail: record every successful trigger-spawned
                // process at INFO level so it appears in the debug log even
                // without DEBUG_LEVEL set.  This allows post-incident review of
                // which commands were executed via triggers.
                crate::debug_info!(
                    "TRIGGER",
                    "AUDIT RunCommand trigger_id={} pid={} command={} args={:?}",
                    trigger_id,
                    pid,
                    command,
                    args
                );
                // Track the spawned process for resource management
                self.trigger_state
                    .trigger_spawned_processes
                    .insert(pid, Instant::now());
            }
            Err(e) => {
                log::error!("RunCommand failed to spawn '{}': {}", command, e);
                crate::debug_error!(
                    "TRIGGER",
                    "AUDIT RunCommand FAILED trigger_id={} command={} error={}",
                    trigger_id,
                    command,
                    e
                );
            }
        }
    }

    /// Handle a SendText trigger action, including security checks and optional delay.
    fn handle_send_text_action(
        &mut self,
        trigger_id: u64,
        text: String,
        delay_ms: u64,
        ctx: &DispatchContext<'_>,
    ) {
        // Security check: prompt_before_run — if the trigger requires confirmation
        // and has not been pre-approved this session (always_allow) or this frame,
        // enqueue the action for dialog presentation and skip direct execution.
        let prompt = ctx
            .trigger_prompt_before_run
            .get(&trigger_id)
            .copied()
            .unwrap_or(true);
        if prompt
            && !self
                .trigger_state
                .always_allow_trigger_ids
                .contains(&trigger_id)
            && !ctx.approved_this_frame.contains(&trigger_id)
        {
            let trigger_name = ctx
                .trigger_names
                .get(&trigger_id)
                .cloned()
                .unwrap_or_else(|| format!("trigger #{}", trigger_id));
            let description = format!("Send text: '{}'", text);
            self.trigger_state.pending_trigger_actions.push(
                crate::app::window_state::PendingTriggerAction {
                    trigger_id,
                    trigger_name,
                    action: ActionResult::SendText {
                        trigger_id,
                        text,
                        delay_ms,
                    },
                    description,
                },
            );
            return;
        }

        // Security check: rate limiting (skip for dialog-approved actions this frame)
        if !ctx.approved_this_frame.contains(&trigger_id)
            && let Some(tab) = self.tab_manager.active_tab_mut()
            && !tab
                .scripting
                .trigger_rate_limiter
                .check_and_update(trigger_id)
        {
            log::warn!(
                "Trigger {} SendText RATE-LIMITED: '{}' (too frequent)",
                trigger_id,
                text,
            );
            return;
        }

        log::info!(
            "Trigger {} firing SendText: '{}' (delay={}ms)",
            trigger_id,
            text,
            delay_ms
        );
        // Security audit trail: record every SendText execution so
        // post-incident analysis can reconstruct what text was injected
        // into the terminal via trigger automation.
        crate::debug_info!(
            "TRIGGER",
            "AUDIT SendText trigger_id={} delay_ms={} text={:?}",
            trigger_id,
            delay_ms,
            text
        );
        if let Some(tab) = self.tab_manager.active_tab() {
            if delay_ms == 0 {
                // try_lock: intentional — trigger SendText in sync event loop.
                // On miss: the triggered text is not sent this frame. Low risk:
                // triggers fire on repeated output patterns and will retry.
                if let Ok(term) = tab.terminal.try_write()
                    && let Err(e) = term.write(text.as_bytes())
                {
                    log::error!("SendText write failed: {}", e);
                }
            } else {
                let terminal = std::sync::Arc::clone(&tab.terminal);
                let text_owned = text;
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    // try_lock: intentional — delayed SendText from spawned thread.
                    // On miss: the delayed text is not sent. Acceptable; trigger
                    // automation has inherent timing flexibility.
                    if let Ok(term) = terminal.try_write()
                        && let Err(e) = term.write(text_owned.as_bytes())
                    {
                        log::error!("Delayed SendText write failed: {}", e);
                    }
                });
            }
        }
    }

    /// Handle a SplitPane trigger action, including security checks and pane creation.
    #[allow(clippy::too_many_arguments)]
    fn handle_split_pane_action(
        &mut self,
        trigger_id: u64,
        direction: par_term_emu_core_rust::terminal::TriggerSplitDirection,
        command: Option<par_term_emu_core_rust::terminal::TriggerSplitCommand>,
        focus_new_pane: bool,
        target: par_term_emu_core_rust::terminal::TriggerSplitTarget,
        source_pane_id: Option<u64>,
        ctx: &DispatchContext<'_>,
    ) {
        // Security check: prompt_before_run — queue action for dialog if not pre-approved
        let prompt = ctx
            .trigger_prompt_before_run
            .get(&trigger_id)
            .copied()
            .unwrap_or(true);
        if prompt
            && !self
                .trigger_state
                .always_allow_trigger_ids
                .contains(&trigger_id)
            && !ctx.approved_this_frame.contains(&trigger_id)
        {
            let trigger_name = ctx
                .trigger_names
                .get(&trigger_id)
                .cloned()
                .unwrap_or_else(|| format!("trigger #{}", trigger_id));
            let dir_str = match direction {
                par_term_emu_core_rust::terminal::TriggerSplitDirection::Horizontal => "horizontal",
                par_term_emu_core_rust::terminal::TriggerSplitDirection::Vertical => "vertical",
            };
            let description = format!("Split pane ({}) and run command", dir_str);
            self.trigger_state.pending_trigger_actions.push(
                crate::app::window_state::PendingTriggerAction {
                    trigger_id,
                    trigger_name,
                    action: ActionResult::SplitPane {
                        trigger_id,
                        direction,
                        command,
                        focus_new_pane,
                        target,
                        source_pane_id,
                    },
                    description,
                },
            );
            return;
        }

        // Security check: rate limiting
        if let Some(tab) = self.tab_manager.active_tab_mut()
            && !tab
                .scripting
                .trigger_rate_limiter
                .check_and_update(trigger_id)
        {
            log::warn!(
                "Trigger {} SplitPane RATE-LIMITED (too frequent)",
                trigger_id,
            );
            return;
        }

        let pane_direction = match direction {
            par_term_emu_core_rust::terminal::TriggerSplitDirection::Horizontal => {
                crate::pane::SplitDirection::Horizontal
            }
            par_term_emu_core_rust::terminal::TriggerSplitDirection::Vertical => {
                crate::pane::SplitDirection::Vertical
            }
        };

        crate::debug_info!(
            "TRIGGER",
            "AUDIT SplitPane trigger_id={} direction={:?} focus_new={}",
            trigger_id,
            pane_direction,
            focus_new_pane
        );

        let pct = ctx
            .trigger_split_percent
            .get(&trigger_id)
            .copied()
            .unwrap_or(66);
        let new_pane_id = self.split_pane_direction(pane_direction, focus_new_pane, None, pct);

        // After split, optionally send a command to the new pane.
        if let (Some(pane_id), Some(cmd)) = (new_pane_id, command) {
            let (text, delay_ms) = match cmd {
                par_term_emu_core_rust::terminal::TriggerSplitCommand::SendText {
                    text,
                    delay_ms,
                } => (format!("{}\n", text), delay_ms),
                par_term_emu_core_rust::terminal::TriggerSplitCommand::InitialCommand {
                    command: cmd_name,
                    args,
                } => {
                    // InitialCommand is not yet supported for trigger-created panes.
                    // Fall back to sending command as text to the new shell.
                    log::warn!(
                        "Trigger {} SplitPane InitialCommand not fully supported; \
                         sending as text",
                        trigger_id
                    );
                    let full = if args.is_empty() {
                        format!("{}\n", cmd_name)
                    } else {
                        format!("{} {}\n", cmd_name, args.join(" "))
                    };
                    (full, 200)
                }
            };

            // Send text to the new pane's terminal with optional delay.
            if let Some(tab) = self.tab_manager.active_tab()
                && let Some(pm) = tab.pane_manager()
                && let Some(pane) = pm.get_pane(pane_id)
            {
                let terminal = std::sync::Arc::clone(&pane.terminal);
                std::thread::spawn(move || {
                    if delay_ms > 0 {
                        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    }
                    if let Ok(term) = terminal.try_write()
                        && let Err(e) = term.write(text.as_bytes())
                    {
                        log::error!(
                            "SplitPane SendText write failed for pane {}: {}",
                            pane_id,
                            e
                        );
                    }
                });
            }
        }
    }

    /// Handle a MarkLine trigger action.
    ///
    /// MarkLine events are batched in `pending_marks` for deduplication.
    fn handle_mark_line_action(
        &mut self,
        trigger_id: u64,
        row: usize,
        label: Option<String>,
        color: Option<(u8, u8, u8)>,
        pending_marks: &mut HashMap<u64, Vec<MarkLineEntry>>,
    ) {
        pending_marks
            .entry(trigger_id)
            .or_default()
            .push((row, label, color));
    }
}

//! Trigger action dispatch for WindowState.
//!
//! This module handles polling trigger action results from the core library
//! and executing frontend-handled actions: RunCommand, PlaySound, SendText,
//! and Prettify (relayed through Notify with magic prefix).
//!
//! ## Sub-modules
//!
//! - `mark_line` — MarkLine result deduplication and application
//! - `prettify` — Prettify relay event processing with scope computation
//! - `sound` — Audio playback for PlaySound trigger actions
//!
//! ## Security
//!
//! Triggers fire on terminal output pattern matches, which means an attacker
//! controlling terminal output (e.g., `cat malicious_file`) could trigger
//! arbitrary command execution. To mitigate this:
//!
//! 1. **`require_user_action` flag** (default: `true`): When set, dangerous
//!    actions (`RunCommand`, `SendText`) are suppressed since all trigger
//!    matches come from passive terminal output. Users must opt-in to
//!    output-triggered dangerous actions by setting this to `false`.
//!
//! 2. **Command denylist**: Even when `require_user_action` is `false`,
//!    `RunCommand` actions are checked against a denylist of dangerous
//!    patterns (rm -rf, curl|bash, eval, etc.).
//!
//! 3. **Rate limiting**: Dangerous actions from output triggers are rate-limited
//!    to prevent malicious output flooding from rapid-fire execution.
//!
//! 4. **Process management**: RunCommand spawns are tracked and limited to prevent
//!    resource exhaustion. Output is redirected to null to prevent terminal corruption.

mod mark_line;
mod prettify;
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

use crate::config::automation::{PRETTIFY_RELAY_PREFIX, PrettifyRelayPayload};

use super::window_state::WindowState;

/// (grid_row, label, color) tuple for a pending MarkLine action.
/// Shared between `mod.rs` (where it's constructed) and `mark_line.rs` (where it's consumed).
pub(self) type MarkLineEntry = (usize, Option<String>, Option<(u8, u8, u8)>);

/// Expand a leading `~/` to the user's home directory.
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest).to_string_lossy().to_string();
    }
    path.to_string()
}

/// Check if a dangerous action from a trigger should be suppressed.
///
/// Returns `true` if the action should be blocked, `false` if it should proceed.
/// This checks the `require_user_action` flag for the trigger. Since all trigger
/// matches come from passive terminal output, `require_user_action: true` means
/// the action is always suppressed.
fn should_suppress_dangerous_action(
    trigger_id: u64,
    action_name: &str,
    trigger_security: &HashMap<u64, bool>,
) -> bool {
    // Look up the require_user_action flag for this trigger.
    // Default to true (suppress) for unknown trigger IDs (safe default).
    let require_user_action = trigger_security.get(&trigger_id).copied().unwrap_or(true);

    if require_user_action {
        log::warn!(
            "Trigger {} {} BLOCKED: require_user_action=true (output-triggered dangerous actions \
             are suppressed by default; set require_user_action: false in trigger config to allow)",
            trigger_id,
            action_name,
        );
        return true;
    }

    false
}

impl WindowState {
    /// Check for trigger action results and dispatch them.
    ///
    /// Called each frame after check_bell(). Polls the core library for
    /// ActionResult events and executes the appropriate frontend action.
    ///
    /// Security restrictions are enforced for dangerous actions:
    /// - `require_user_action` flag blocks RunCommand/SendText from output triggers
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
        let (action_results, current_scrollback_len, custom_vars) =
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

        if action_results.is_empty() {
            return;
        }

        // Snapshot the trigger security map from the active tab for checking
        // require_user_action. We clone the reference to avoid borrow issues.
        let trigger_security = if let Some(t) = self.tab_manager.active_tab() {
            t.trigger_security.clone()
        } else {
            return;
        };

        // Collect MarkLine events for batch deduplication (processed after the loop).
        // Between frames, the core may fire the same trigger multiple times for the
        // same physical line (once per PTY read). Each scan records a different grid
        // row because scrollback grows between scans, but we only get the scrollback_len
        // at poll time. Batch dedup clusters these into one mark per physical line.
        let mut pending_marks: HashMap<u64, Vec<MarkLineEntry>> = HashMap::new();

        // Collect prettify relay events (MarkLine with __prettify__ label prefix).
        // Tuple: (trigger_id, matched_grid_row, payload).
        let mut pending_prettify: Vec<(u64, usize, PrettifyRelayPayload)> = Vec::new();

        for action in action_results {
            match action {
                ActionResult::RunCommand {
                    trigger_id,
                    command,
                    args,
                } => {
                    let command = expand_tilde(&command);
                    let args: Vec<String> = args.iter().map(|a| expand_tilde(a)).collect();

                    // Security check 1: require_user_action flag
                    if should_suppress_dangerous_action(trigger_id, "RunCommand", &trigger_security)
                    {
                        continue;
                    }

                    // Security check 2: rate limiting
                    if let Some(tab) = self.tab_manager.active_tab_mut()
                        && !tab.trigger_rate_limiter.check_and_update(trigger_id)
                    {
                        log::warn!(
                            "Trigger {} RunCommand RATE-LIMITED: '{}' (too frequent)",
                            trigger_id,
                            command,
                        );
                        continue;
                    }

                    // Security check 3: command denylist
                    if let Some(denied_pattern) = check_command_denylist(&command, &args) {
                        log::error!(
                            "Trigger {} RunCommand DENIED: '{}' matches denylist pattern '{}'",
                            trigger_id,
                            command,
                            denied_pattern,
                        );
                        continue;
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
                        continue;
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
                            && let Some(ref audio_bell) = tab.bell.audio
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
                    // Security check 1: require_user_action flag
                    if should_suppress_dangerous_action(trigger_id, "SendText", &trigger_security) {
                        continue;
                    }

                    // Security check 2: rate limiting
                    if let Some(tab) = self.tab_manager.active_tab_mut()
                        && !tab.trigger_rate_limiter.check_and_update(trigger_id)
                    {
                        log::warn!(
                            "Trigger {} SendText RATE-LIMITED: '{}' (too frequent)",
                            trigger_id,
                            text,
                        );
                        continue;
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
                ActionResult::MarkLine {
                    trigger_id,
                    row,
                    label,
                    color,
                } => {
                    // Check if this is a prettify relay (packed into MarkLine by to_core_action).
                    if let Some(ref lbl) = label
                        && let Some(json) = lbl.strip_prefix(PRETTIFY_RELAY_PREFIX)
                    {
                        if let Ok(payload) = serde_json::from_str::<PrettifyRelayPayload>(json) {
                            pending_prettify.push((trigger_id, row, payload));
                        } else {
                            log::error!(
                                "Trigger {} prettify relay: invalid payload: {}",
                                trigger_id,
                                json
                            );
                        }
                        continue;
                    }
                    pending_marks
                        .entry(trigger_id)
                        .or_default()
                        .push((row, label, color));
                }
            }
        }

        // Periodically clean up stale rate limiter entries (every ~60 seconds of entries)
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.trigger_rate_limiter.cleanup(60);
        }

        // Process collected MarkLine events with deduplication.
        if !pending_marks.is_empty() {
            self.apply_mark_line_results(pending_marks, current_scrollback_len);
        }

        // Process collected prettify relay events.
        if !pending_prettify.is_empty() {
            self.apply_prettify_triggers(pending_prettify, current_scrollback_len);
        }
    }

}

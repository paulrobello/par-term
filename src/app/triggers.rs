//! Trigger action dispatch and sound playback.
//!
//! This module handles polling trigger action results from the core library
//! and executing frontend-handled actions: RunCommand, PlaySound, SendText,
//! and Prettify (relayed through Notify with magic prefix).
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

use std::collections::HashMap;
use std::io::BufReader;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Instant, SystemTime};

use par_term_config::check_command_denylist;

/// Maximum number of concurrent trigger-spawned processes allowed.
/// This prevents resource exhaustion from rapid-fire triggers.
const MAX_TRIGGER_PROCESSES: usize = 10;

/// Maximum age (in seconds) for tracked processes before cleanup.
/// Processes older than this are assumed to have completed.
const PROCESS_CLEANUP_AGE_SECS: u64 = 300; // 5 minutes
use par_term_emu_core_rust::terminal::ActionResult;

use crate::config::automation::{PRETTIFY_RELAY_PREFIX, PrettifyRelayPayload, PrettifyScope};
use crate::prettifier::types::ContentBlock;
use crate::tab::Tab;

use super::window_state::WindowState;

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

/// (grid_row, label, color) tuple for a pending MarkLine action.
type MarkLineEntry = (usize, Option<String>, Option<(u8, u8, u8)>);

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

    /// Apply MarkLine trigger results using a rebuild strategy.
    ///
    /// Between frames, the core fires trigger scans on every PTY read. Each
    /// scan records the match at a different grid row (because scrollback grows
    /// between reads), and the batch may contain rows like [10, 8, 6, 4] for a
    /// single physical line. Trying to cluster these is fragile.
    ///
    /// Instead, we use a rebuild approach:
    /// 1. Keep historical marks that have scrolled into scrollback (they won't
    ///    be re-scanned, so we must preserve them).
    /// 2. Discard stale marks in the visible grid for each trigger_id present
    ///    in the current batch (these will be rebuilt from fresh results).
    /// 3. Add new marks using only the smallest row per trigger_id (most
    ///    current, consistent with `current_scrollback_len`).
    fn apply_mark_line_results(
        &mut self,
        pending_marks: HashMap<u64, Vec<MarkLineEntry>>,
        current_scrollback_len: usize,
    ) {
        let tab = if let Some(t) = self.tab_manager.active_tab_mut() {
            t
        } else {
            return;
        };

        // Remove stale visible-grid marks for trigger_ids that have fresh results.
        // Marks in scrollback (line < current_scrollback_len) are historical and
        // must be preserved since the trigger scanner only scans the visible grid.
        let trigger_ids_in_batch: Vec<u64> = pending_marks.keys().copied().collect();
        tab.trigger_marks.retain(|m| {
            if let Some(tid) = m.trigger_id
                && trigger_ids_in_batch.contains(&tid)
            {
                // Keep only if in scrollback (historical)
                return m.line < current_scrollback_len;
            }
            true // Keep marks from other triggers or shell integration
        });

        // For each trigger, deduplicate rows from the batch. The last scan
        // (producing the smallest rows) has row values consistent with
        // current_scrollback_len. We use a HashSet of rows to eliminate exact
        // duplicates, then add marks for each unique row.
        for (trigger_id, entries) in pending_marks {
            // Deduplicate: keep only unique rows, preferring the entry with
            // the smallest row (from the most recent scan).
            let mut seen_rows = std::collections::HashSet::new();
            let mut unique: Vec<MarkLineEntry> = Vec::new();
            // Process in reverse so the last (smallest-row) entry for each
            // physical line wins.
            for (row, label, color) in entries.into_iter().rev() {
                if seen_rows.insert(row) {
                    unique.push((row, label, color));
                }
            }

            for (row, label, color) in unique {
                let absolute_line = current_scrollback_len + row;
                log::info!(
                    "Trigger {} MarkLine: row={} abs={} label={:?}",
                    trigger_id,
                    row,
                    absolute_line,
                    label
                );
                tab.trigger_marks
                    .push(crate::scrollback_metadata::ScrollbackMark {
                        line: absolute_line,
                        exit_code: None,
                        start_time: None,
                        duration_ms: None,
                        command: label,
                        color,
                        trigger_id: Some(trigger_id),
                    });
            }
        }
    }

    /// Process collected prettify relay events.
    ///
    /// Each event is a `(trigger_id, matched_grid_row, payload)` tuple relayed
    /// through the core MarkLine system. This method:
    /// 1. Checks the master prettifier toggle
    /// 2. Handles `command_filter` scoping
    /// 3. Routes `format: "none"` to suppression
    /// 4. Builds `ContentBlock`s based on scope and dispatches to the pipeline
    fn apply_prettify_triggers(
        &mut self,
        pending: Vec<(u64, usize, PrettifyRelayPayload)>,
        current_scrollback_len: usize,
    ) {
        // Pre-populate the regex cache with all patterns from pending events.
        // This is done before borrowing `tab` to avoid borrow-checker conflicts.
        // Patterns that are already cached are skipped; invalid patterns are logged once.
        for (trigger_id, _row, payload) in &pending {
            if let Some(ref filter) = payload.command_filter
                && !self
                    .trigger_state
                    .trigger_regex_cache
                    .contains_key(filter.as_str())
            {
                match regex::Regex::new(filter) {
                    Ok(re) => {
                        self.trigger_state
                            .trigger_regex_cache
                            .insert(filter.clone(), re);
                    }
                    Err(e) => {
                        log::error!(
                            "Trigger {} prettify: invalid command_filter regex '{}': {}",
                            trigger_id,
                            filter,
                            e
                        );
                    }
                }
            }
            if let Some(ref block_end) = payload.block_end
                && !self
                    .trigger_state
                    .trigger_regex_cache
                    .contains_key(block_end.as_str())
            {
                match regex::Regex::new(block_end) {
                    Ok(re) => {
                        self.trigger_state
                            .trigger_regex_cache
                            .insert(block_end.clone(), re);
                    }
                    Err(e) => {
                        log::error!(
                            "Trigger {} prettify: invalid block_end regex '{}': {}",
                            trigger_id,
                            block_end,
                            e
                        );
                    }
                }
            }
        }

        let tab = if let Some(t) = self.tab_manager.active_tab_mut() {
            t
        } else {
            return;
        };

        // Check master toggle — if prettifier is disabled, skip all.
        if let Some(ref pipeline) = tab.prettifier {
            if !pipeline.is_enabled() {
                return;
            }
        } else {
            // No pipeline configured — nothing to do.
            return;
        }

        // Read terminal content and metadata we need for scope handling.
        // We lock the terminal once and extract everything we need.
        let (lines_by_abs, preceding_command, scope_ranges) =
            Self::read_terminal_context(tab, current_scrollback_len, &pending);

        for (idx, (trigger_id, _grid_row, payload)) in pending.into_iter().enumerate() {
            // Check command_filter: if set, only fire when the preceding command matches.
            if let Some(ref filter) = payload.command_filter {
                // Use the pre-compiled regex from the cache (populated before this loop).
                // If the pattern was invalid it was logged above and not inserted — skip.
                if let Some(re) = self.trigger_state.trigger_regex_cache.get(filter.as_str()) {
                    match preceding_command.as_deref() {
                        Some(cmd) if re.is_match(cmd) => {}
                        _ => {
                            log::debug!(
                                "Trigger {} prettify: command_filter '{}' did not match, skipping",
                                trigger_id,
                                filter
                            );
                            continue;
                        }
                    }
                } else {
                    // Pattern was invalid (logged during pre-compilation above).
                    continue;
                }
            }

            // Get the pre-computed scope range; narrow for Block scope if block_end is set.
            let (start_abs, end_abs) = if let Some(&range) = scope_ranges.get(idx) {
                if payload.scope == PrettifyScope::Block {
                    // Look up the pre-compiled block_end regex from the cache.
                    let block_end_re = payload
                        .block_end
                        .as_deref()
                        .and_then(|pat| self.trigger_state.trigger_regex_cache.get(pat));
                    Self::narrow_block_scope(range.0, range.1, block_end_re, &lines_by_abs)
                } else {
                    range
                }
            } else {
                continue;
            };

            log::info!(
                "Trigger {} prettify: format='{}' scope={:?} rows={}..{}",
                trigger_id,
                payload.format,
                payload.scope,
                start_abs,
                end_abs,
            );

            // Handle "none" format — suppress auto-detection for this range.
            if payload.format == "none" {
                if let Some(ref mut pipeline) = tab.prettifier {
                    pipeline.suppress_detection(start_abs..end_abs);
                    log::debug!(
                        "Trigger {} prettify: suppressed auto-detection for rows {}..{}",
                        trigger_id,
                        start_abs,
                        end_abs,
                    );
                }
                continue;
            }

            // Build the ContentBlock from the determined range.
            let content_lines: Vec<String> = (start_abs..end_abs)
                .filter_map(|abs| lines_by_abs.get(&abs).cloned())
                .collect();

            if content_lines.is_empty() {
                log::debug!(
                    "Trigger {} prettify: no content in range {}..{}, skipping",
                    trigger_id,
                    start_abs,
                    end_abs,
                );
                continue;
            }

            let content = ContentBlock {
                lines: content_lines,
                preceding_command: preceding_command.clone(),
                start_row: start_abs,
                end_row: end_abs,
                timestamp: SystemTime::now(),
            };

            // Dispatch to the pipeline, bypassing confidence scoring.
            if let Some(ref mut pipeline) = tab.prettifier {
                pipeline.trigger_prettify(&payload.format, content);
            }
        }
    }

    /// Read terminal content and metadata needed for prettify scope handling.
    ///
    /// Returns `(lines_by_abs_line, preceding_command, scope_ranges)`.
    /// `scope_ranges` maps each pending index to its `(start_abs, end_abs)` range.
    /// We read all needed lines in one lock acquisition to avoid contention.
    fn read_terminal_context(
        tab: &Tab,
        current_scrollback_len: usize,
        pending: &[(u64, usize, PrettifyRelayPayload)],
    ) -> (HashMap<usize, String>, Option<String>, Vec<(usize, usize)>) {
        #![allow(clippy::type_complexity)]
        let mut lines_by_abs: HashMap<usize, String> = HashMap::new();
        let mut scope_ranges: Vec<(usize, usize)> = Vec::with_capacity(pending.len());

        let preceding_command;

        // try_lock: intentional — prettify trigger processing in about_to_wait (sync loop).
        // On miss: prettify is skipped this frame; the pending events are reprocessed next poll.
        if let Ok(term) = tab.terminal.try_write() {
            // Compute scope ranges for each pending event using scrollback metadata.
            let max_readable = current_scrollback_len + 200; // generous upper bound for visible grid
            for (_trigger_id, grid_row, payload) in pending {
                let matched_abs = current_scrollback_len + grid_row;
                let range = match payload.scope {
                    PrettifyScope::Line => (matched_abs, matched_abs + 1),
                    PrettifyScope::CommandOutput => {
                        // Use previous_mark/next_mark to find command output boundaries.
                        let output_start = term
                            .scrollback_previous_mark(matched_abs)
                            .map(|p| p + 1) // output starts after the prompt line
                            .unwrap_or(0);
                        let output_end = term
                            .scrollback_next_mark(matched_abs + 1)
                            .unwrap_or(max_readable);
                        (output_start, output_end)
                    }
                    PrettifyScope::Block => {
                        // For block scope, read from match to a reasonable limit.
                        // Actual block_end matching is done after reading.
                        (matched_abs, matched_abs + 500)
                    }
                };
                scope_ranges.push(range);
            }

            // Find the widest range we need to read.
            let min_abs = scope_ranges.iter().map(|(s, _)| *s).min().unwrap_or(0);
            let max_abs = scope_ranges.iter().map(|(_, e)| *e).max().unwrap_or(0);

            // Read the lines in the determined range.
            for abs_line in min_abs..max_abs {
                if let Some(text) = term.line_text_at_absolute(abs_line) {
                    lines_by_abs.insert(abs_line, text);
                }
            }

            // Get preceding command from the most recent mark before the first match.
            let first_matched_abs = pending
                .iter()
                .map(|(_, row, _)| current_scrollback_len + row)
                .min()
                .unwrap_or(0);

            preceding_command = term
                .scrollback_previous_mark(first_matched_abs)
                .and_then(|mark_line| term.scrollback_metadata_for_line(mark_line))
                .and_then(|m| m.command);
        } else {
            return (lines_by_abs, None, Vec::new());
        }

        (lines_by_abs, preceding_command, scope_ranges)
    }

    /// Narrow a block scope range by scanning for a block_end regex match.
    ///
    /// If `block_end_re` is set and matches a line in `lines_by_abs`, the range
    /// is narrowed to `start..match+1`. Otherwise returns the original range.
    ///
    /// Accepts a pre-compiled `Regex` reference to avoid hot-path recompilation.
    fn narrow_block_scope(
        start_abs: usize,
        end_abs: usize,
        block_end_re: Option<&regex::Regex>,
        lines_by_abs: &HashMap<usize, String>,
    ) -> (usize, usize) {
        if let Some(re) = block_end_re {
            for abs in (start_abs + 1)..end_abs {
                if let Some(text) = lines_by_abs.get(&abs)
                    && re.is_match(text)
                {
                    return (start_abs, abs + 1); // Include the end line.
                }
            }
        }

        // No block_end found or no pattern — use original range capped to available content.
        let max_available = lines_by_abs.keys().max().copied().unwrap_or(start_abs) + 1;
        (start_abs, end_abs.min(max_available))
    }

    /// Play a sound file. Absolute paths are used directly; relative names
    /// are resolved against the par-term sounds directory.
    fn play_sound_file(sound_id: &str, volume: u8) {
        let candidate = std::path::Path::new(sound_id);
        let path = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            Self::sounds_dir().join(sound_id)
        };

        if !path.exists() {
            log::warn!("Sound file not found: {}", path.display());
            return;
        }

        let volume_f32 = (volume as f32 / 100.0).clamp(0.0, 1.0);

        std::thread::spawn(move || {
            let file = match std::fs::File::open(&path) {
                Ok(f) => f,
                Err(e) => {
                    log::error!("Failed to open sound file '{}': {}", path.display(), e);
                    return;
                }
            };
            let stream = match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to open audio output: {}", e);
                    return;
                }
            };
            let sink = rodio::Player::connect_new(stream.mixer());
            let source = match rodio::Decoder::new(BufReader::new(file)) {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to decode sound file '{}': {}", path.display(), e);
                    return;
                }
            };
            sink.set_volume(volume_f32);
            sink.append(source);
            sink.sleep_until_end();
        });
    }

    /// Get the sounds directory path.
    fn sounds_dir() -> PathBuf {
        if let Some(config_dir) = dirs::config_dir() {
            config_dir.join("par-term").join("sounds")
        } else {
            PathBuf::from("sounds")
        }
    }
}

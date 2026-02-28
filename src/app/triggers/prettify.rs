//! Prettify relay trigger processing for WindowState.
//!
//! Handles `Prettify` triggers that are relayed through the core MarkLine
//! system. Responsible for:
//! - Regex cache pre-population
//! - Command filter matching
//! - Scope range computation (Line, CommandOutput, Block)
//! - ContentBlock construction and pipeline dispatch

use std::collections::HashMap;
use std::time::SystemTime;

use crate::app::window_state::WindowState;
use crate::config::automation::{PrettifyRelayPayload, PrettifyScope};
use crate::prettifier::types::ContentBlock;
use crate::tab::Tab;

impl WindowState {
    /// Process collected prettify relay events.
    ///
    /// Each event is a `(trigger_id, matched_grid_row, payload)` tuple relayed
    /// through the core MarkLine system. This method:
    /// 1. Checks the master prettifier toggle
    /// 2. Handles `command_filter` scoping
    /// 3. Routes `format: "none"` to suppression
    /// 4. Builds `ContentBlock`s based on scope and dispatches to the pipeline
    pub(super) fn apply_prettify_triggers(
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
    pub(super) fn read_terminal_context(
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
    pub(super) fn narrow_block_scope(
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
}

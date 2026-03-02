//! MarkLine trigger result application for WindowState.
//!
//! Implements the rebuild strategy for deduplicating MarkLine events
//! that fire multiple times per frame due to repeated PTY reads.

use std::collections::HashMap;

use super::MarkLineEntry;
use crate::app::window_state::WindowState;

impl WindowState {
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
    pub(super) fn apply_mark_line_results(
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
        tab.scripting.trigger_marks.retain(|m| {
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
                tab.scripting
                    .trigger_marks
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
}

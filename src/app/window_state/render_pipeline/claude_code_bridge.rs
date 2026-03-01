//! Claude Code prettifier bridge — per-frame viewport → pipeline interaction.
//!
//! Extracted from `gather_data.rs` as proposed in the R-05 doc comment.
//!
//! `ClaudeCodePrettifierBridge` encapsulates the heuristic Claude Code session
//! detection, viewport hashing, action-bullet segmentation, and segment
//! preprocessing that previously lived inline in `gather_render_data`.
//!
//! The struct takes shared/mutable references at construction time, reducing
//! the borrow-checker surface in the caller.

use crate::cell_renderer::Cell;
use crate::pane::PaneManager;
use crate::prettifier::pipeline::PrettifierPipeline;

/// Encapsulates per-frame Claude Code viewport → prettifier pipeline interaction.
pub(super) struct ClaudeCodePrettifierBridge<'a> {
    pub(super) pipeline: &'a mut PrettifierPipeline,
    pub(super) pane_manager: &'a mut Option<PaneManager>,
    pub(super) cells: &'a [Cell],
    pub(super) visible_lines: usize,
    pub(super) grid_cols: usize,
    pub(super) scrollback_len: usize,
    pub(super) scroll_offset: usize,
}

impl<'a> ClaudeCodePrettifierBridge<'a> {
    /// Scan visible output for Claude Code signature patterns.
    ///
    /// Returns `true` if this frame triggered detection for the first time.
    /// No-op (returns `false`) if already active.
    pub(super) fn detect_session(&mut self) -> bool {
        if self.pipeline.claude_code().is_active() {
            return false;
        }
        for row_idx in 0..self.visible_lines {
            let start = row_idx * self.grid_cols;
            let end = (start + self.grid_cols).min(self.cells.len());
            if start >= self.cells.len() {
                break;
            }
            let row_text: String = self.cells[start..end]
                .iter()
                .map(|c| {
                    let g = c.grapheme.as_str();
                    if g.is_empty() || g == "\0" { " " } else { g }
                })
                .collect();
            if row_text.contains("Claude Code")
                || row_text.contains("claude.ai/code")
                || row_text.contains("Tips for getting the best")
                || (row_text.contains("Model:")
                    && (row_text.contains("Opus")
                        || row_text.contains("Sonnet")
                        || row_text.contains("Haiku")))
            {
                crate::debug_info!(
                    "PRETTIFIER",
                    "Claude Code session detected from output heuristic"
                );
                self.pipeline.mark_claude_code_active();
                return true;
            }
        }
        false
    }

    /// Hash a sample of visible rows to detect viewport-level changes.
    pub(super) fn compute_viewport_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        // Sample every 4th row for speed; enough to catch redraws.
        for row_idx in (0..self.visible_lines).step_by(4) {
            let start = row_idx * self.grid_cols;
            let end = (start + self.grid_cols).min(self.cells.len());
            if start >= self.cells.len() {
                break;
            }
            for c in &self.cells[start..end] {
                c.grapheme.as_str().hash(&mut hasher);
            }
        }
        self.scrollback_len.hash(&mut hasher);
        self.scroll_offset.hash(&mut hasher);
        hasher.finish()
    }

    /// Retrieve the last cached viewport hash from the focused pane.
    pub(super) fn cached_viewport_hash(&self) -> u64 {
        self.pane_manager
            .as_ref()
            .and_then(|pm| pm.focused_pane())
            .map(|p| p.cache.prettifier_feed_last_hash)
            .unwrap_or(0)
    }

    /// Store the viewport hash in the focused pane's cache.
    pub(super) fn store_viewport_hash(&mut self, hash: u64) {
        if let Some(pm) = self.pane_manager.as_mut()
            && let Some(pane) = pm.focused_pane_mut()
        {
            pane.cache.prettifier_feed_last_hash = hash;
        }
    }

    /// Segment the visible viewport at action bullets and submit each segment.
    ///
    /// Clears existing blocks when the viewport has changed, then submits
    /// segments with enough content for meaningful detection.
    pub(super) fn segment_and_submit(&mut self, viewport_changed: bool) {
        use crate::app::window_state::{
            preprocess_claude_code_segment, reconstruct_markdown_from_cells,
        };

        // Clear blocks when visible content changes.
        if viewport_changed {
            if !self.pipeline.active_blocks().is_empty() {
                self.pipeline.clear_blocks();
                crate::debug_log!("PRETTIFIER", "CC viewport changed, cleared all blocks");
            }
        }

        self.pipeline.reset_boundary();

        crate::debug_log!(
            "PRETTIFIER",
            "per-frame feed (CC): scanning {} visible lines, viewport_changed={}, scrollback={}, scroll_offset={}",
            self.visible_lines,
            viewport_changed,
            self.scrollback_len,
            self.scroll_offset
        );

        // Collect all rows with raw + reconstructed text.
        let mut rows: Vec<(String, String, usize)> = Vec::new(); // (raw, recon, abs_row)

        for row_idx in 0..self.visible_lines {
            let absolute_row = self.scrollback_len.saturating_sub(self.scroll_offset) + row_idx;
            let start = row_idx * self.grid_cols;
            let end = (start + self.grid_cols).min(self.cells.len());
            if start >= self.cells.len() {
                break;
            }

            let row_text: String = self.cells[start..end]
                .iter()
                .map(|c| {
                    let g = c.grapheme.as_str();
                    if g.is_empty() || g == "\0" { " " } else { g }
                })
                .collect();

            let line = reconstruct_markdown_from_cells(&self.cells[start..end]);
            rows.push((row_text, line, absolute_row));
        }

        // Split into segments at action bullets (⏺) and collapse markers.
        let mut segments: Vec<Vec<(String, usize)>> = Vec::new();
        let mut current: Vec<(String, usize)> = Vec::new();

        for (raw, recon, abs_row) in &rows {
            let trimmed = raw.trim();
            // Collapse markers — boundary, include the line in the preceding
            // segment so row alignment is preserved.
            if raw.contains("(ctrl+o to expand)") {
                current.push((recon.clone(), *abs_row));
                segments.push(std::mem::take(&mut current));
                continue;
            }
            // Action bullets (⏺) start a new segment
            if trimmed.starts_with('⏺') || trimmed.starts_with("● ") {
                if !current.is_empty() {
                    segments.push(std::mem::take(&mut current));
                }
                current.push((recon.clone(), *abs_row));
                continue;
            }
            // Horizontal rules (─────) are boundaries
            if trimmed.len() > 10 && trimmed.chars().all(|c| c == '─' || c == '━') {
                if !current.is_empty() {
                    segments.push(std::mem::take(&mut current));
                }
                continue;
            }
            current.push((recon.clone(), *abs_row));
        }
        if !current.is_empty() {
            segments.push(current);
        }

        crate::debug_log!(
            "PRETTIFIER",
            "CC segmentation: {} total rows -> {} segments",
            rows.len(),
            segments.len()
        );

        let min_segment_lines = 5;
        let mut submitted = 0usize;
        let mut skipped_short = 0usize;
        let mut skipped_empty = 0usize;
        for mut segment in segments {
            let non_empty = segment.iter().filter(|(l, _)| !l.trim().is_empty()).count();
            if non_empty < min_segment_lines {
                skipped_short += 1;
                continue;
            }

            let pre_len = segment.len();
            preprocess_claude_code_segment(&mut segment);
            if segment.is_empty() {
                skipped_empty += 1;
                continue;
            }

            crate::debug_log!(
                "PRETTIFIER",
                "CC segment: {} lines (was {} before preprocess), rows={}..{}, first={:?}",
                segment.len(),
                pre_len,
                segment.first().map(|(_, r)| *r).unwrap_or(0),
                segment.last().map(|(_, r)| *r + 1).unwrap_or(0),
                segment
                    .first()
                    .map(|(l, _)| &l[..l.floor_char_boundary(60)])
            );

            submitted += 1;
            self.pipeline
                .submit_command_output(std::mem::take(&mut segment), Some("claude".to_string()));
        }

        crate::debug_log!(
            "PRETTIFIER",
            "CC segmentation complete: submitted={}, skipped_short={}, skipped_empty={}",
            submitted,
            skipped_short,
            skipped_empty
        );
    }
}

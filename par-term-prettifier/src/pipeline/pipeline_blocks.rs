//! Block lifecycle management for [`super::pipeline_impl::PrettifierPipeline`].
//!
//! Contains the private helpers that manage the `active_blocks` deque:
//! - [`PrettifierPipeline::handle_block`] — detect, render, deduplicate, and push a new block.
//! - [`PrettifierPipeline::evict_excess_blocks`] — cap the deque and clean up stale metadata.
//! - Public query accessors: [`active_blocks`] and [`block_at_row`].
//!
//! These methods are `impl` blocks on `PrettifierPipeline` split into a separate file
//! to keep `pipeline_impl.rs` under 500 lines.

use std::collections::VecDeque;

use super::super::buffer::DualViewBuffer;
use super::super::types::ContentBlock;
use super::block::PrettifiedBlock;
use super::pipeline_impl::{MAX_ACTIVE_BLOCKS, PrettifierPipeline};

impl PrettifierPipeline {
    /// Detect format and render a content block, storing it as a `PrettifiedBlock`.
    ///
    /// # Sub-phases
    ///
    /// 1. **Suppression check** — bail early if this row range is suppressed.
    /// 2. **Stale-block eviction** — remove overlapping blocks whose content changed.
    /// 3. **Detection + render + store** — run format detection, render the block,
    ///    deduplicate by content hash, and push to `active_blocks`.
    pub(super) fn handle_block(&mut self, content: ContentBlock) {
        // Phase 1: Suppression check
        let row_range = content.start_row..content.end_row;
        if self.is_suppressed(&row_range) {
            crate::debug_log!(
                "PRETTIFIER",
                "pipeline::handle_block: SUPPRESSED rows={}..{}, skipping",
                row_range.start,
                row_range.end
            );
            return;
        }

        crate::debug_info!(
            "PRETTIFIER",
            "pipeline::handle_block: processing {} lines, rows={}..{}, active_blocks={}",
            content.lines.len(),
            content.start_row,
            content.end_row,
            self.active_blocks.len()
        );

        // Log first few lines of content for debugging
        for (i, line) in content.lines.iter().take(5).enumerate() {
            crate::debug_log!(
                "PRETTIFIER",
                "pipeline::handle_block: content[{}]={:?}",
                i,
                &line[..line.floor_char_boundary(100)]
            );
        }
        if content.lines.len() > 5 {
            crate::debug_log!(
                "PRETTIFIER",
                "pipeline::handle_block: ... ({} more lines)",
                content.lines.len() - 5
            );
        }

        let detection_result = self.registry.detect(&content);
        if detection_result.is_none() {
            // Remove stale blocks: if an existing block overlaps this range
            // but the content has changed (e.g., approval prompt replaced markdown),
            // the old block must be removed so it doesn't cover the new content.
            let content_hash = {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                content.lines.hash(&mut hasher);
                hasher.finish()
            };
            let stale_idx = self.active_blocks.iter().position(|b| {
                let c = b.content();
                c.start_row < row_range.end
                    && c.end_row > row_range.start
                    && b.buffer.content_hash() != content_hash
            });
            if let Some(idx) = stale_idx {
                crate::debug_log!(
                    "PRETTIFIER",
                    "removing stale block rows={}..{} (content changed)",
                    self.active_blocks[idx].content().start_row,
                    self.active_blocks[idx].content().end_row
                );
                self.active_blocks.remove(idx);
            }
        }
        if let Some(detection) = detection_result {
            let format_id = detection.format_id.clone();
            let mut buffer = DualViewBuffer::new(content);
            let content_hash = buffer.content_hash();
            let terminal_width = self.renderer_config.terminal_width;

            // Deduplicate: if an existing block covers overlapping rows,
            // skip if content is identical, or replace if content changed.
            // This prevents the per-frame viewport feed from creating
            // thousands of duplicate blocks for the same visible content.
            let overlapping_idx = self.active_blocks.iter().position(|b| {
                let c = b.content();
                c.start_row < row_range.end && c.end_row > row_range.start
            });
            if let Some(idx) = overlapping_idx {
                if self.active_blocks[idx].buffer.content_hash() == content_hash {
                    // Same content, same rows — skip duplicate.
                    return;
                }

                // Content changed — remove the old block so we can replace it.
                // The render_pipeline's content-hash dedup + throttle prevents
                // per-frame churn, so we can always allow replacement here.
                self.active_blocks.remove(idx);
            }

            crate::debug_info!(
                "PRETTIFIER",
                "block detected: format={}, confidence={:.2}, rows={}..{}, lines={}",
                detection.format_id,
                detection.confidence,
                row_range.start,
                row_range.end,
                buffer.source().lines.len()
            );

            self.render_into_buffer(&mut buffer, &format_id, terminal_width);

            let block_id = self.next_block_id;
            self.next_block_id += 1;

            let has_rendered = buffer.rendered().is_some();
            crate::debug_info!(
                "PRETTIFIER",
                "block stored: id={}, rendered={}",
                block_id,
                has_rendered
            );

            self.active_blocks.push_back(PrettifiedBlock {
                buffer,
                detection,
                block_id,
            });

            // Evict oldest blocks if we exceed the cap.
            self.evict_excess_blocks();
        }
    }

    /// Evict oldest active blocks when the count exceeds [`MAX_ACTIVE_BLOCKS`].
    ///
    /// Also cleans up suppressed ranges and Claude Code integration entries
    /// that reference rows below the oldest remaining block.
    pub(super) fn evict_excess_blocks(&mut self) {
        while self.active_blocks.len() > MAX_ACTIVE_BLOCKS {
            self.active_blocks.pop_front();
        }

        // Clean up suppressed_ranges below the oldest remaining block.
        if let Some(oldest) = self.active_blocks.front() {
            let min_row = oldest.content().start_row;
            self.suppressed_ranges.retain(|r| r.end > min_row);
            self.claude_code.cleanup_stale_entries(min_row);
        }
    }

    /// Get the list of active prettified blocks.
    pub fn active_blocks(&self) -> &VecDeque<PrettifiedBlock> {
        &self.active_blocks
    }

    /// Find the prettified block that covers the given row.
    ///
    /// A block covers row `r` if `start_row <= r < end_row`.
    pub fn block_at_row(&self, row: usize) -> Option<&PrettifiedBlock> {
        // Blocks are naturally sorted by start_row. Use binary search to find
        // the last block whose start_row <= row, then check if row < end_row.
        let idx = self
            .active_blocks
            .partition_point(|b| b.content().start_row <= row);
        if idx == 0 {
            return None;
        }
        let block = &self.active_blocks[idx - 1];
        if row < block.content().end_row {
            Some(block)
        } else {
            None
        }
    }
}

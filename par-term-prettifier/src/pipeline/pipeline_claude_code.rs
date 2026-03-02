//! Claude Code integration methods for [`super::pipeline_impl::PrettifierPipeline`].
//!
//! Provides session detection, content expand handling, and line-level event processing
//! for Claude Code sessions. These methods delegate to the [`ClaudeCodeIntegration`] field
//! on the pipeline.

use std::ops::Range;

use super::super::buffer::DualViewBuffer;
use super::super::claude_code::{ClaudeCodeEvent, ClaudeCodeIntegration};
use super::super::types::ContentBlock;
use super::block::PrettifiedBlock;
use super::pipeline_impl::PrettifierPipeline;

impl PrettifierPipeline {
    /// Access the Claude Code integration state.
    pub fn claude_code(&self) -> &ClaudeCodeIntegration {
        &self.claude_code
    }

    /// Attempt to detect a Claude Code session from environment and process name.
    pub fn detect_claude_code_session(
        &mut self,
        env_vars: &std::collections::HashMap<String, String>,
        process_name: &str,
    ) -> bool {
        self.claude_code.detect_session(env_vars, process_name)
    }

    /// Manually mark this as a Claude Code session (from output pattern heuristics).
    pub fn mark_claude_code_active(&mut self) {
        self.claude_code.mark_active();
    }

    /// Feed a line through the Claude Code integration.
    ///
    /// Returns an event if the line is a Claude Code control pattern.
    pub fn process_claude_code_line(&mut self, line: &str, row: usize) -> Option<ClaudeCodeEvent> {
        self.claude_code.process_line(line, row)
    }

    /// Called when Claude Code integration detects a content expansion (Ctrl+O).
    ///
    /// If `auto_render_on_expand` is enabled, re-processes the expanded content
    /// through the detection pipeline.
    pub fn on_claude_code_expand(&mut self, row_range: Range<usize>) {
        if !self.claude_code.config().auto_render_on_expand {
            return;
        }

        let block = self.extract_content_block(row_range);
        if let Some(detection) = self.registry.detect(&block) {
            let format_id = detection.format_id.clone();
            let mut buffer = DualViewBuffer::new(block);
            let terminal_width = self.renderer_config.terminal_width;

            self.render_into_buffer(&mut buffer, &format_id, terminal_width);

            let block_id = self.next_block_id;
            self.next_block_id += 1;

            self.active_blocks.push_back(PrettifiedBlock {
                buffer,
                detection,
                block_id,
            });
        }
    }

    /// Build a `ContentBlock` from a row range (used for expand events).
    fn extract_content_block(&self, row_range: Range<usize>) -> ContentBlock {
        // Gather lines from active blocks that overlap this range, or create
        // a placeholder block for the row range.
        let mut lines = Vec::new();
        for block in &self.active_blocks {
            let c = block.content();
            if c.start_row < row_range.end && c.end_row > row_range.start {
                // Overlapping block — extract the relevant lines.
                let start = row_range.start.saturating_sub(c.start_row);
                let end = (row_range.end - c.start_row).min(c.lines.len());
                for line in &c.lines[start..end] {
                    lines.push(line.clone());
                }
            }
        }

        ContentBlock {
            lines,
            preceding_command: None,
            start_row: row_range.start,
            end_row: row_range.end,
            timestamp: std::time::SystemTime::now(),
        }
    }
}

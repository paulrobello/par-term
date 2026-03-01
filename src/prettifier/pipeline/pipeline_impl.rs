//! `PrettifierPipeline` — boundary detection → format detection → rendering.

use std::collections::{HashMap, VecDeque};
use std::ops::Range;

use super::super::boundary::{BoundaryConfig, BoundaryDetector, DetectionScope};
use super::super::buffer::DualViewBuffer;
use super::super::cache::RenderCache;
use super::super::claude_code::{ClaudeCodeEvent, ClaudeCodeIntegration};
use super::super::registry::RendererRegistry;
use super::super::traits::RendererConfig;
use super::super::types::{ContentBlock, DetectionResult, DetectionSource};
use super::block::PrettifiedBlock;
use super::config::PrettifierConfig;
use crate::config::prettifier::ClaudeCodeConfig;

/// Default maximum number of entries in the render cache.
const DEFAULT_CACHE_SIZE: usize = 64;

/// Maximum active blocks before oldest-first eviction.
const MAX_ACTIVE_BLOCKS: usize = 128;

/// Orchestrates boundary detection, format detection, and rendering.
pub struct PrettifierPipeline {
    /// Detects content block boundaries in the terminal output stream.
    boundary_detector: BoundaryDetector,
    /// Registry of detectors and renderers.
    registry: RendererRegistry,
    /// Blocks that have been detected and (optionally) rendered.
    active_blocks: VecDeque<PrettifiedBlock>,
    /// Base enabled state from config.
    enabled: bool,
    /// Per-session override for enabled state (from toggle).
    pub(super) session_override: Option<bool>,
    /// Monotonically increasing block ID counter.
    next_block_id: u64,
    /// Terminal environment for renderers.
    renderer_config: RendererConfig,
    /// Cache of rendered content to avoid re-rendering unchanged blocks.
    render_cache: RenderCache,
    /// Row ranges where auto-detection is suppressed (via `prettify_format: "none"` triggers).
    pub(super) suppressed_ranges: Vec<Range<usize>>,
    /// Claude Code integration for session detection and expand/collapse tracking.
    claude_code: ClaudeCodeIntegration,
}

impl PrettifierPipeline {
    /// Create a new pipeline from config, registry, and renderer config.
    pub fn new(
        config: PrettifierConfig,
        registry: RendererRegistry,
        renderer_config: RendererConfig,
    ) -> Self {
        Self::with_claude_code(
            config,
            registry,
            renderer_config,
            ClaudeCodeConfig::default(),
        )
    }

    /// Create a new pipeline with explicit Claude Code configuration.
    pub fn with_claude_code(
        config: PrettifierConfig,
        mut registry: RendererRegistry,
        renderer_config: RendererConfig,
        claude_config: ClaudeCodeConfig,
    ) -> Self {
        registry.set_confidence_threshold(config.confidence_threshold);

        let boundary_config = BoundaryConfig {
            scope: config.detection_scope,
            max_scan_lines: config.max_scan_lines,
            debounce_ms: config.debounce_ms,
            blank_line_threshold: 2,
        };

        Self {
            boundary_detector: BoundaryDetector::new(boundary_config),
            registry,
            active_blocks: VecDeque::new(),
            enabled: config.enabled,
            session_override: None,
            next_block_id: 0,
            renderer_config,
            render_cache: RenderCache::new(DEFAULT_CACHE_SIZE),
            suppressed_ranges: Vec::new(),
            claude_code: ClaudeCodeIntegration::new(claude_config),
        }
    }

    /// Feed a line of terminal output. May trigger block emission, detection,
    /// and rendering.
    pub fn process_output(&mut self, line: &str, row: usize) {
        if !self.is_enabled() {
            crate::debug_trace!(
                "PRETTIFIER",
                "pipeline::process_output SKIPPED (disabled) row={}: {:?}",
                row,
                &line[..line.floor_char_boundary(60)]
            );
            return;
        }
        if let Some(block) = self.boundary_detector.push_line(line, row) {
            crate::debug_info!(
                "PRETTIFIER",
                "pipeline::process_output: boundary emitted block, {} lines, rows={}..{}",
                block.lines.len(),
                block.start_row,
                block.end_row
            );
            self.handle_block(block);
        }
    }

    /// Signal that a command is starting (OSC 133 C marker).
    pub fn on_command_start(&mut self, command: &str) {
        self.boundary_detector.on_command_start(command);
    }

    /// Signal that a command has ended (OSC 133 D marker).
    pub fn on_command_end(&mut self) {
        if let Some(block) = self.boundary_detector.on_command_end() {
            self.handle_block(block);
        }
    }

    /// Submit pre-built command output lines (read from scrollback) for detection and rendering.
    ///
    /// This bypasses the boundary detector's line-by-line accumulation and directly
    /// creates a `ContentBlock` from the provided lines. Used when `CommandFinished`
    /// fires and we can read the complete output from terminal scrollback.
    pub fn submit_command_output(&mut self, lines: Vec<(String, usize)>, command: Option<String>) {
        self.boundary_detector.reset();
        if lines.is_empty() {
            crate::debug_log!(
                "PRETTIFIER",
                "pipeline::submit_command_output: empty lines, skipping"
            );
            return;
        }

        let start_row = lines.first().expect("lines is non-empty, checked above").1;
        let end_row = lines.last().expect("lines is non-empty, checked above").1 + 1;

        crate::debug_info!(
            "PRETTIFIER",
            "pipeline::submit_command_output: {} lines, rows={}..{}, cmd={:?}, first={:?}, last={:?}",
            lines.len(),
            start_row,
            end_row,
            command.as_deref().map(|c| &c[..c.floor_char_boundary(40)]),
            lines.first().map(|(l, _)| &l[..l.floor_char_boundary(60)]),
            lines.last().map(|(l, _)| &l[..l.floor_char_boundary(60)])
        );

        let text_lines: Vec<String> = lines.into_iter().map(|(text, _)| text).collect();

        let block = ContentBlock {
            lines: text_lines,
            preceding_command: command,
            start_row,
            end_row,
            timestamp: std::time::SystemTime::now(),
        };

        self.handle_block(block);
    }

    /// Get the configured detection scope.
    pub fn detection_scope(&self) -> DetectionScope {
        self.boundary_detector.scope()
    }

    /// Reset the boundary detector, discarding accumulated lines.
    ///
    /// Used by the per-frame feed to provide a fresh content snapshot
    /// on each generation change, preventing duplicate accumulation.
    pub fn reset_boundary(&mut self) {
        self.boundary_detector.reset();
    }

    /// Clear all active prettified blocks.
    ///
    /// Used when transitioning from verbose→compact mode in Claude Code sessions
    /// so that cell substitution doesn't overwrite Claude Code's own rendering.
    pub fn clear_blocks(&mut self) {
        self.active_blocks.clear();
    }

    /// Signal that the terminal entered or exited the alternate screen.
    pub fn on_alt_screen_change(&mut self, entering: bool) {
        if let Some(block) = self.boundary_detector.on_alt_screen_change(entering) {
            self.handle_block(block);
        }
    }

    /// Check whether the debounce timeout has elapsed.
    pub fn check_debounce(&mut self) {
        if let Some(block) = self.boundary_detector.check_debounce() {
            self.handle_block(block);
        }
    }

    /// Bypass detection and force-render content as a specific format.
    ///
    /// Creates a `PrettifiedBlock` with confidence 1.0 and `TriggerInvoked` source.
    pub fn trigger_prettify(&mut self, format_id: &str, content: ContentBlock) {
        crate::debug_info!(
            "PRETTIFIER",
            "pipeline::trigger_prettify: format={}, {} lines, rows={}..{}",
            format_id,
            content.lines.len(),
            content.start_row,
            content.end_row
        );

        let detection = DetectionResult {
            format_id: format_id.to_string(),
            confidence: 1.0,
            matched_rules: vec![],
            source: DetectionSource::TriggerInvoked,
        };

        let mut buffer = DualViewBuffer::new(content);
        let terminal_width = self.renderer_config.terminal_width;

        self.render_into_buffer(&mut buffer, format_id, terminal_width);

        let block_id = self.next_block_id;
        self.next_block_id += 1;

        crate::debug_info!(
            "PRETTIFIER",
            "pipeline::trigger_prettify: stored block_id={}, has_rendered={}",
            block_id,
            buffer.rendered().is_some()
        );

        self.active_blocks.push_back(PrettifiedBlock {
            buffer,
            detection,
            block_id,
        });
    }

    /// Toggle the global enabled state for this session.
    pub fn toggle_global(&mut self) {
        self.session_override = Some(!self.is_enabled());
    }

    /// Toggle the view mode for a specific block.
    pub fn toggle_block(&mut self, block_id: u64) {
        if let Some(block) = self
            .active_blocks
            .iter_mut()
            .find(|b| b.block_id == block_id)
        {
            block.buffer.toggle_view();
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

    /// Whether the pipeline is effectively enabled.
    pub fn is_enabled(&self) -> bool {
        self.session_override.unwrap_or(self.enabled)
    }

    /// Mark a row range as suppressed (no auto-detection).
    ///
    /// Used by `prettify_format: "none"` triggers to prevent the auto-detection
    /// pipeline from running on specific content.
    pub fn suppress_detection(&mut self, row_range: Range<usize>) {
        // Avoid duplicates.
        if !self.suppressed_ranges.iter().any(|r| r == &row_range) {
            self.suppressed_ranges.push(row_range);
        }
    }

    /// Check if auto-detection is suppressed for a row range.
    pub fn is_suppressed(&self, row_range: &Range<usize>) -> bool {
        self.suppressed_ranges.iter().any(|suppressed| {
            // Suppressed if any suppressed range fully contains the query range.
            suppressed.start <= row_range.start && suppressed.end >= row_range.end
        })
    }

    // -- Claude Code integration ------------------------------------------------

    /// Access the Claude Code integration state.
    pub fn claude_code(&self) -> &ClaudeCodeIntegration {
        &self.claude_code
    }

    /// Attempt to detect a Claude Code session from environment and process name.
    pub fn detect_claude_code_session(
        &mut self,
        env_vars: &HashMap<String, String>,
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

    // -- Renderer config -------------------------------------------------------

    /// Update the renderer config (e.g., on terminal resize or theme change).
    ///
    /// When the terminal width changes, marks all blocks as needing re-render.
    pub fn update_renderer_config(&mut self, config: RendererConfig) {
        self.renderer_config = config;
    }

    /// Update cell dimensions from the GPU renderer.
    ///
    /// Called after the renderer is initialized (or on font change) so that
    /// inline graphics (e.g., Mermaid diagrams) are sized with the actual
    /// cell metrics instead of the fallback estimate.
    pub fn update_cell_dims(&mut self, width: f32, height: f32) {
        self.renderer_config.cell_width_px = Some(width);
        self.renderer_config.cell_height_px = Some(height);
    }

    /// Re-render all blocks that need it (e.g., after a terminal width change).
    pub fn re_render_if_needed(&mut self) {
        let terminal_width = self.renderer_config.terminal_width;

        for block in &mut self.active_blocks {
            if block.buffer.needs_render(terminal_width) {
                let format_id = block.detection.format_id.clone();
                let content_hash = block.buffer.content_hash();

                // Check cache first.
                if let Some(cached) = self.render_cache.get(content_hash, terminal_width) {
                    block.buffer.set_rendered(cached.clone(), terminal_width);
                } else if let Some(renderer) = self.registry.get_renderer(&format_id)
                    && let Ok(rendered) =
                        renderer.render(block.buffer.source(), &self.renderer_config)
                {
                    self.render_cache.put(
                        content_hash,
                        terminal_width,
                        &format_id,
                        rendered.clone(),
                    );
                    block.buffer.set_rendered(rendered, terminal_width);
                }
            }
        }
    }

    /// Get a reference to the render cache (for diagnostics).
    pub fn render_cache(&self) -> &RenderCache {
        &self.render_cache
    }

    /// Detect format and render a content block, storing it as a `PrettifiedBlock`.
    ///
    /// # Sub-phases (R-44 extraction candidates)
    ///
    /// This method performs three conceptually distinct phases that could be
    /// extracted into private helpers to improve readability (L-effort because
    /// of the tight local-variable dependencies below):
    ///
    /// 1. **Suppression check** — bail early if this row range is suppressed.
    /// 2. **Stale-block eviction** — remove overlapping blocks whose content changed.
    /// 3. **Detection + render + store** — run format detection, render the block,
    ///    deduplicate by content hash, and push to `active_blocks`.
    ///
    /// Extraction blockers:
    /// - `row_range`, `detection_result`, `content_hash`, and `format_id` are
    ///   computed at different points and shared across all three phases.
    /// - `content` is consumed (moved) into `DualViewBuffer::new`, so ownership
    ///   must transfer at the right point and cannot be borrowed across phases
    ///   without restructuring the types.
    ///
    /// Proposed helpers (future refactoring):
    /// ```ignore
    /// fn is_block_stale_or_duplicate(&self, row_range: &Range<usize>, hash: u64) -> BlockStatus;
    /// fn detect_and_render(&mut self, content: ContentBlock) -> Option<PrettifiedBlock>;
    /// ```
    fn handle_block(&mut self, content: ContentBlock) {
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
    fn evict_excess_blocks(&mut self) {
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

    /// Render content into a `DualViewBuffer`, using the cache when possible.
    fn render_into_buffer(
        &mut self,
        buffer: &mut DualViewBuffer,
        format_id: &str,
        terminal_width: usize,
    ) {
        let content_hash = buffer.content_hash();

        crate::debug_log!(
            "PRETTIFIER",
            "pipeline::render_into_buffer: format={}, hash={:#x}, width={}, source_lines={}",
            format_id,
            content_hash,
            terminal_width,
            buffer.source().lines.len()
        );

        // Check cache first.
        if let Some(cached) = self.render_cache.get(content_hash, terminal_width) {
            crate::debug_info!(
                "PRETTIFIER",
                "pipeline::render_into_buffer: CACHE HIT, {} rendered lines",
                cached.lines.len()
            );
            buffer.set_rendered(cached.clone(), terminal_width);
            return;
        }

        // Render and cache.
        if let Some(renderer) = self.registry.get_renderer(format_id) {
            match renderer.render(buffer.source(), &self.renderer_config) {
                Ok(rendered) => {
                    crate::debug_info!(
                        "PRETTIFIER",
                        "pipeline::render_into_buffer: RENDERED {} lines -> {} styled lines, badge={:?}",
                        buffer.source().lines.len(),
                        rendered.lines.len(),
                        rendered.format_badge
                    );
                    // Log first few rendered lines
                    for (i, line) in rendered.lines.iter().take(3).enumerate() {
                        let text: String = line.segments.iter().map(|s| s.text.as_str()).collect();
                        crate::debug_log!(
                            "PRETTIFIER",
                            "pipeline::render_into_buffer: output[{}]={:?} (segs={})",
                            i,
                            &text[..text.floor_char_boundary(100)],
                            line.segments.len()
                        );
                    }
                    if rendered.lines.len() > 3 {
                        crate::debug_log!(
                            "PRETTIFIER",
                            "pipeline::render_into_buffer: ... ({} more rendered lines)",
                            rendered.lines.len() - 3
                        );
                    }
                    self.render_cache.put(
                        content_hash,
                        terminal_width,
                        format_id,
                        rendered.clone(),
                    );
                    buffer.set_rendered(rendered, terminal_width);
                }
                Err(e) => {
                    crate::debug_error!(
                        "PRETTIFIER",
                        "pipeline::render_into_buffer: RENDER FAILED format={}: {:?}",
                        format_id,
                        e
                    );
                }
            }
        } else {
            crate::debug_error!(
                "PRETTIFIER",
                "pipeline::render_into_buffer: NO RENDERER found for format={}",
                format_id
            );
        }
    }
}

//! Prettifier pipeline: boundary detection → format detection → rendering.
//!
//! `PrettifierPipeline` wires together a `BoundaryDetector`, a `RendererRegistry`,
//! a `RenderCache`, and block tracking into a single flow. Terminal output lines are
//! fed in, content blocks are emitted at boundaries, detected, rendered, and stored
//! for display. Each `PrettifiedBlock` wraps a `DualViewBuffer` for efficient
//! source/rendered toggling and copy operations.

use std::collections::{HashMap, VecDeque};
use std::ops::Range;

use super::boundary::{BoundaryConfig, BoundaryDetector, DetectionScope};
use super::buffer::DualViewBuffer;
use super::cache::RenderCache;
use super::claude_code::{ClaudeCodeEvent, ClaudeCodeIntegration};
use super::registry::RendererRegistry;
use super::traits::RendererConfig;
use super::types::{ContentBlock, DetectionResult, DetectionSource, ViewMode};
use crate::config::prettifier::ClaudeCodeConfig;

/// Default maximum number of entries in the render cache.
const DEFAULT_CACHE_SIZE: usize = 64;

/// Maximum active blocks before oldest-first eviction.
const MAX_ACTIVE_BLOCKS: usize = 128;

/// Configuration for the `PrettifierPipeline`.
#[derive(Debug, Clone)]
pub struct PrettifierConfig {
    /// Whether the prettifier is enabled.
    pub enabled: bool,
    /// Whether to respect alternate-screen transitions as boundaries.
    pub respect_alternate_screen: bool,
    /// Minimum confidence for a detection to be accepted.
    pub confidence_threshold: f32,
    /// Maximum lines to accumulate before forcing emission.
    pub max_scan_lines: usize,
    /// Milliseconds of inactivity before emitting a block.
    pub debounce_ms: u64,
    /// When to detect boundaries.
    pub detection_scope: DetectionScope,
}

impl Default for PrettifierConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            respect_alternate_screen: true,
            confidence_threshold: 0.6,
            max_scan_lines: 500,
            debounce_ms: 100,
            detection_scope: DetectionScope::All,
        }
    }
}

/// A content block that has been through the detection and rendering pipeline.
///
/// Wraps a `DualViewBuffer` for source/rendered dual-view management.
#[derive(Debug)]
pub struct PrettifiedBlock {
    /// Dual-view buffer managing source + rendered content.
    pub buffer: DualViewBuffer,
    /// The detection result that matched this block.
    pub detection: DetectionResult,
    /// Unique identifier for this block within the session.
    pub block_id: u64,
}

impl PrettifiedBlock {
    /// Get the original content block.
    pub fn content(&self) -> &ContentBlock {
        self.buffer.source()
    }

    /// Get the current view mode.
    pub fn view_mode(&self) -> ViewMode {
        *self.buffer.view_mode()
    }

    /// Whether rendered content is available.
    pub fn has_rendered(&self) -> bool {
        self.buffer.rendered().is_some()
    }
}

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
    session_override: Option<bool>,
    /// Monotonically increasing block ID counter.
    next_block_id: u64,
    /// Terminal environment for renderers.
    renderer_config: RendererConfig,
    /// Cache of rendered content to avoid re-rendering unchanged blocks.
    render_cache: RenderCache,
    /// Row ranges where auto-detection is suppressed (via `prettify_format: "none"` triggers).
    suppressed_ranges: Vec<Range<usize>>,
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
                &line[..line.len().min(60)]
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
            command.as_deref().map(|c| &c[..c.len().min(40)]),
            lines.first().map(|(l, _)| &l[..l.len().min(60)]),
            lines.last().map(|(l, _)| &l[..l.len().min(60)])
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
    fn handle_block(&mut self, content: ContentBlock) {
        // Skip auto-detection if this block's row range is suppressed.
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
                &line[..line.len().min(100)]
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

                // Don't replace a larger block with a much smaller overlapping one.
                // This prevents viewport-sized per-frame feeds (~24 lines) from
                // evicting full command-output blocks (~200+ lines).
                let existing = self.active_blocks[idx].content();
                let existing_span = existing.end_row - existing.start_row;
                let new_span = row_range.end - row_range.start;
                if new_span * 2 < existing_span {
                    crate::debug_log!(
                        "PRETTIFIER",
                        "pipeline::handle_block: keeping larger block ({}..{}, {} lines) over smaller ({}..{}, {} lines)",
                        existing.start_row,
                        existing.end_row,
                        existing_span,
                        row_range.start,
                        row_range.end,
                        new_span
                    );
                    return;
                }

                // Content changed — remove the old block so we can replace it.
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
                            &text[..text.len().min(100)],
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
                    self.render_cache
                        .put(content_hash, terminal_width, format_id, rendered.clone());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::traits::*;
    use crate::prettifier::types::*;
    use std::time::SystemTime;

    // -----------------------------------------------------------------------
    // Test helpers — mock detector and renderer
    // -----------------------------------------------------------------------

    struct AlwaysDetector {
        id: &'static str,
        confidence: f32,
    }

    impl ContentDetector for AlwaysDetector {
        fn format_id(&self) -> &str {
            self.id
        }
        fn display_name(&self) -> &str {
            self.id
        }
        fn detect(&self, _content: &ContentBlock) -> Option<DetectionResult> {
            Some(DetectionResult {
                format_id: self.id.to_string(),
                confidence: self.confidence,
                matched_rules: vec!["always".to_string()],
                source: DetectionSource::AutoDetected,
            })
        }
        fn quick_match(&self, _first_lines: &[&str]) -> bool {
            true
        }
        fn detection_rules(&self) -> &[DetectionRule] {
            &[]
        }
    }

    struct OkRenderer {
        id: &'static str,
    }

    impl ContentRenderer for OkRenderer {
        fn format_id(&self) -> &str {
            self.id
        }
        fn display_name(&self) -> &str {
            self.id
        }
        fn capabilities(&self) -> Vec<RendererCapability> {
            vec![RendererCapability::TextStyling]
        }
        fn render(
            &self,
            _content: &ContentBlock,
            _config: &RendererConfig,
        ) -> Result<RenderedContent, RenderError> {
            Ok(RenderedContent {
                lines: vec![StyledLine::plain("rendered")],
                line_mapping: vec![],
                graphics: vec![],
                format_badge: "OK".to_string(),
            })
        }
        fn format_badge(&self) -> &str {
            "OK"
        }
    }

    struct FailRenderer;

    impl ContentRenderer for FailRenderer {
        fn format_id(&self) -> &str {
            "fail"
        }
        fn display_name(&self) -> &str {
            "Fail"
        }
        fn capabilities(&self) -> Vec<RendererCapability> {
            vec![]
        }
        fn render(
            &self,
            _content: &ContentBlock,
            _config: &RendererConfig,
        ) -> Result<RenderedContent, RenderError> {
            Err(RenderError::RenderFailed("boom".to_string()))
        }
        fn format_badge(&self) -> &str {
            "FAIL"
        }
    }

    fn test_registry(confidence: f32) -> RendererRegistry {
        let mut reg = RendererRegistry::new(confidence);
        reg.register_detector(
            10,
            Box::new(AlwaysDetector {
                id: "test",
                confidence: 0.8,
            }),
        );
        reg.register_renderer("test", Box::new(OkRenderer { id: "test" }));
        reg
    }

    fn test_pipeline() -> PrettifierPipeline {
        PrettifierPipeline::new(
            PrettifierConfig {
                detection_scope: DetectionScope::All,
                ..PrettifierConfig::default()
            },
            test_registry(0.5),
            RendererConfig::default(),
        )
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_process_output_flow() {
        let mut pipeline = test_pipeline();

        // Feed lines that trigger blank-line boundary (threshold=2).
        pipeline.process_output("# Hello", 0);
        pipeline.process_output("world", 1);
        pipeline.process_output("", 2);
        pipeline.process_output("", 3);

        // Should have detected and rendered one block.
        assert_eq!(pipeline.active_blocks().len(), 1);
        let block = &pipeline.active_blocks()[0];
        assert_eq!(block.detection.format_id, "test");
        assert!(block.has_rendered());
        assert_eq!(block.view_mode(), ViewMode::Rendered);
        assert_eq!(block.block_id, 0);
    }

    #[test]
    fn test_trigger_prettify_bypasses_detection() {
        let mut pipeline = test_pipeline();

        let content = ContentBlock {
            lines: vec!["raw content".to_string()],
            preceding_command: None,
            start_row: 0,
            end_row: 1,
            timestamp: SystemTime::now(),
        };

        pipeline.trigger_prettify("test", content);

        assert_eq!(pipeline.active_blocks().len(), 1);
        let block = &pipeline.active_blocks()[0];
        assert_eq!(block.detection.source, DetectionSource::TriggerInvoked);
        assert!((block.detection.confidence - 1.0).abs() < f32::EPSILON);
        assert!(block.has_rendered());
    }

    #[test]
    fn test_toggle_global() {
        let mut pipeline = test_pipeline();
        assert!(pipeline.is_enabled());

        pipeline.toggle_global();
        assert!(!pipeline.is_enabled());

        pipeline.toggle_global();
        assert!(pipeline.is_enabled());
    }

    #[test]
    fn test_toggle_block() {
        let mut pipeline = test_pipeline();

        let content = ContentBlock {
            lines: vec!["test".to_string()],
            preceding_command: None,
            start_row: 0,
            end_row: 1,
            timestamp: SystemTime::now(),
        };
        pipeline.trigger_prettify("test", content);

        assert_eq!(pipeline.active_blocks()[0].view_mode(), ViewMode::Rendered);
        pipeline.toggle_block(0);
        assert_eq!(pipeline.active_blocks()[0].view_mode(), ViewMode::Source);
        pipeline.toggle_block(0);
        assert_eq!(pipeline.active_blocks()[0].view_mode(), ViewMode::Rendered);

        // Toggling a non-existent block is a no-op.
        pipeline.toggle_block(999);
    }

    #[test]
    fn test_is_enabled_with_session_override() {
        let config = PrettifierConfig {
            enabled: false,
            ..PrettifierConfig::default()
        };
        let mut pipeline =
            PrettifierPipeline::new(config, test_registry(0.5), RendererConfig::default());

        assert!(!pipeline.is_enabled());

        // Override to enabled.
        pipeline.session_override = Some(true);
        assert!(pipeline.is_enabled());

        // Override to disabled.
        pipeline.session_override = Some(false);
        assert!(!pipeline.is_enabled());

        // Clear override — falls back to config.
        pipeline.session_override = None;
        assert!(!pipeline.is_enabled());
    }

    #[test]
    fn test_disabled_pipeline_discards() {
        let config = PrettifierConfig {
            enabled: false,
            detection_scope: DetectionScope::All,
            ..PrettifierConfig::default()
        };
        let mut pipeline =
            PrettifierPipeline::new(config, test_registry(0.5), RendererConfig::default());

        pipeline.process_output("# Hello", 0);
        pipeline.process_output("world", 1);
        pipeline.process_output("", 2);
        pipeline.process_output("", 3);

        // Disabled — no blocks should be produced.
        assert!(pipeline.active_blocks().is_empty());
    }

    #[test]
    fn test_block_at_row() {
        let mut pipeline = test_pipeline();

        let content = ContentBlock {
            lines: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            preceding_command: None,
            start_row: 10,
            end_row: 13,
            timestamp: SystemTime::now(),
        };
        pipeline.trigger_prettify("test", content);

        assert!(pipeline.block_at_row(9).is_none());
        assert!(pipeline.block_at_row(10).is_some());
        assert!(pipeline.block_at_row(12).is_some());
        assert!(pipeline.block_at_row(13).is_none());
    }

    #[test]
    fn test_on_command_end_triggers_detection() {
        let config = PrettifierConfig {
            detection_scope: DetectionScope::CommandOutput,
            ..PrettifierConfig::default()
        };
        let mut pipeline =
            PrettifierPipeline::new(config, test_registry(0.5), RendererConfig::default());

        pipeline.on_command_start("echo hello");
        pipeline.process_output("hello", 0);
        pipeline.on_command_end();

        assert_eq!(pipeline.active_blocks().len(), 1);
        assert_eq!(
            pipeline.active_blocks()[0]
                .content()
                .preceding_command
                .as_deref(),
            Some("echo hello")
        );
    }

    #[test]
    fn test_render_failure_stores_none() {
        let mut reg = RendererRegistry::new(0.5);
        reg.register_detector(
            10,
            Box::new(AlwaysDetector {
                id: "fail",
                confidence: 0.8,
            }),
        );
        reg.register_renderer("fail", Box::new(FailRenderer));

        let mut pipeline = PrettifierPipeline::new(
            PrettifierConfig {
                detection_scope: DetectionScope::All,
                ..PrettifierConfig::default()
            },
            reg,
            RendererConfig::default(),
        );

        let content = ContentBlock {
            lines: vec!["test".to_string()],
            preceding_command: None,
            start_row: 0,
            end_row: 1,
            timestamp: SystemTime::now(),
        };
        pipeline.trigger_prettify("fail", content);

        assert_eq!(pipeline.active_blocks().len(), 1);
        assert!(!pipeline.active_blocks()[0].has_rendered());
    }

    #[test]
    fn test_block_ids_increment() {
        let mut pipeline = test_pipeline();

        for i in 0..3 {
            let content = ContentBlock {
                lines: vec![format!("block {i}")],
                preceding_command: None,
                start_row: i * 10,
                end_row: i * 10 + 1,
                timestamp: SystemTime::now(),
            };
            pipeline.trigger_prettify("test", content);
        }

        let ids: Vec<u64> = pipeline
            .active_blocks()
            .iter()
            .map(|b| b.block_id)
            .collect();
        assert_eq!(ids, vec![0, 1, 2]);
    }

    #[test]
    fn test_config_defaults() {
        let config = PrettifierConfig::default();
        assert!(config.enabled);
        assert!(config.respect_alternate_screen);
        assert!((config.confidence_threshold - 0.6).abs() < f32::EPSILON);
        assert_eq!(config.max_scan_lines, 500);
        assert_eq!(config.debounce_ms, 100);
        assert_eq!(config.detection_scope, DetectionScope::All);
    }

    #[test]
    fn test_render_cache_hit() {
        let mut pipeline = test_pipeline();

        // Render the same content twice — second time should be a cache hit.
        let content1 = ContentBlock {
            lines: vec!["same content".to_string()],
            preceding_command: None,
            start_row: 0,
            end_row: 1,
            timestamp: SystemTime::now(),
        };
        let content2 = ContentBlock {
            lines: vec!["same content".to_string()],
            preceding_command: None,
            start_row: 10,
            end_row: 11,
            timestamp: SystemTime::now(),
        };

        pipeline.trigger_prettify("test", content1);
        pipeline.trigger_prettify("test", content2);

        assert_eq!(pipeline.active_blocks().len(), 2);
        assert!(pipeline.active_blocks()[0].has_rendered());
        assert!(pipeline.active_blocks()[1].has_rendered());

        // Cache should have registered a hit.
        let stats = pipeline.render_cache().stats();
        assert!(stats.hit_count >= 1);
    }

    #[test]
    fn test_source_text_available_via_buffer() {
        let mut pipeline = test_pipeline();

        let content = ContentBlock {
            lines: vec!["original text".to_string()],
            preceding_command: None,
            start_row: 0,
            end_row: 1,
            timestamp: SystemTime::now(),
        };
        pipeline.trigger_prettify("test", content);

        let block = &pipeline.active_blocks()[0];
        assert_eq!(block.buffer.source_text(), "original text");
    }

    #[test]
    fn test_suppress_detection_stores_range() {
        let mut pipeline = test_pipeline();
        assert!(!pipeline.is_suppressed(&(10..20)));

        pipeline.suppress_detection(10..20);
        assert!(pipeline.is_suppressed(&(10..20)));

        // Sub-range is also suppressed (fully contained).
        assert!(pipeline.is_suppressed(&(12..18)));
    }

    #[test]
    fn test_suppress_detection_non_overlapping() {
        let mut pipeline = test_pipeline();
        pipeline.suppress_detection(10..20);

        // Non-overlapping range is not suppressed.
        assert!(!pipeline.is_suppressed(&(0..5)));
        assert!(!pipeline.is_suppressed(&(25..30)));

        // Partially overlapping range is not suppressed (not fully contained).
        assert!(!pipeline.is_suppressed(&(5..15)));
        assert!(!pipeline.is_suppressed(&(15..25)));
    }

    #[test]
    fn test_suppress_detection_deduplicates() {
        let mut pipeline = test_pipeline();
        pipeline.suppress_detection(10..20);
        pipeline.suppress_detection(10..20);
        // Should not add a duplicate.
        assert_eq!(pipeline.suppressed_ranges.len(), 1);
    }

    #[test]
    fn test_handle_block_skips_suppressed() {
        let mut pipeline = test_pipeline();

        // Suppress the range where the block will land.
        pipeline.suppress_detection(0..4);

        // Feed lines that would normally trigger a block at rows 0..2.
        pipeline.process_output("# Hello", 0);
        pipeline.process_output("world", 1);
        pipeline.process_output("", 2);
        pipeline.process_output("", 3);

        // Block should NOT be produced because rows 0..2 are suppressed.
        assert!(pipeline.active_blocks().is_empty());
    }

    #[test]
    fn test_trigger_prettify_confidence_and_source() {
        let mut pipeline = test_pipeline();

        let content = ContentBlock {
            lines: vec!["test".to_string()],
            preceding_command: Some("echo test".to_string()),
            start_row: 5,
            end_row: 6,
            timestamp: SystemTime::now(),
        };

        pipeline.trigger_prettify("test", content);

        let block = &pipeline.active_blocks()[0];
        // Confidence must be exactly 1.0 for trigger-invoked blocks.
        assert!((block.detection.confidence - 1.0).abs() < f32::EPSILON);
        assert_eq!(block.detection.source, DetectionSource::TriggerInvoked);
        // Matched rules should be empty (no detection was run).
        assert!(block.detection.matched_rules.is_empty());
        // Preceding command should be preserved.
        assert_eq!(
            block.content().preceding_command.as_deref(),
            Some("echo test")
        );
    }

    #[test]
    fn test_larger_block_not_replaced_by_smaller() {
        let mut pipeline = test_pipeline();

        // Simulate a full command-output block covering rows 0..100.
        let full_lines: Vec<String> = (0..100).map(|i| format!("line {i}")).collect();
        let full_block = ContentBlock {
            lines: full_lines,
            preceding_command: Some("test cmd".to_string()),
            start_row: 0,
            end_row: 100,
            timestamp: SystemTime::now(),
        };
        pipeline.trigger_prettify("test", full_block);
        assert_eq!(pipeline.active_blocks().len(), 1);
        assert_eq!(pipeline.active_blocks()[0].content().end_row, 100);

        // Simulate a viewport-sized per-frame feed overlapping the big block.
        // It should NOT replace the 100-line block.
        let viewport_lines: Vec<(String, usize)> = (80..100)
            .map(|i| (format!("line {i} updated"), i))
            .collect();
        pipeline.submit_command_output(viewport_lines, None);

        // Should still have exactly 1 block — the original large one.
        assert_eq!(pipeline.active_blocks().len(), 1);
        assert_eq!(pipeline.active_blocks()[0].content().end_row, 100);
    }

    #[test]
    fn test_similar_sized_block_can_replace() {
        let mut pipeline = test_pipeline();

        // Create a block covering rows 0..25.
        let lines: Vec<String> = (0..25).map(|i| format!("line {i}")).collect();
        let block1 = ContentBlock {
            lines,
            preceding_command: None,
            start_row: 0,
            end_row: 25,
            timestamp: SystemTime::now(),
        };
        pipeline.trigger_prettify("test", block1);
        assert_eq!(pipeline.active_blocks().len(), 1);

        // Submit a similarly-sized block with different content.
        // It should replace the original (both are ~viewport-sized).
        let new_lines: Vec<(String, usize)> = (0..24)
            .map(|i| (format!("updated line {i}"), i))
            .collect();
        pipeline.submit_command_output(new_lines, None);

        // The old block should be replaced.
        assert_eq!(pipeline.active_blocks().len(), 1);
        assert_eq!(pipeline.active_blocks()[0].content().end_row, 24);
    }

    #[test]
    fn test_display_lines_via_buffer() {
        let mut pipeline = test_pipeline();

        let content = ContentBlock {
            lines: vec!["raw".to_string()],
            preceding_command: None,
            start_row: 0,
            end_row: 1,
            timestamp: SystemTime::now(),
        };
        pipeline.trigger_prettify("test", content);

        let block = &pipeline.active_blocks()[0];
        // In rendered mode, should show rendered content.
        let lines = block.buffer.display_lines();
        assert_eq!(lines[0].segments[0].text, "rendered");
    }
}

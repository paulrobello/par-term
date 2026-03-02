//! `PrettifierPipeline` — boundary detection → format detection → rendering.

use std::collections::VecDeque;
use std::ops::Range;

use super::super::boundary::{BoundaryConfig, BoundaryDetector, DetectionScope};
use super::super::buffer::DualViewBuffer;
use super::super::cache::RenderCache;
use super::super::claude_code::ClaudeCodeIntegration;
use super::super::registry::RendererRegistry;
use super::super::traits::RendererConfig;
use super::super::types::{ContentBlock, DetectionResult, DetectionSource};
use super::block::PrettifiedBlock;
use super::config::PrettifierConfig;
use crate::config::prettifier::ClaudeCodeConfig;

/// Default maximum number of entries in the render cache.
const DEFAULT_CACHE_SIZE: usize = 64;

/// Maximum active blocks before oldest-first eviction.
pub(super) const MAX_ACTIVE_BLOCKS: usize = 128;

/// Orchestrates boundary detection, format detection, and rendering.
pub struct PrettifierPipeline {
    /// Detects content block boundaries in the terminal output stream.
    pub(super) boundary_detector: BoundaryDetector,
    /// Registry of detectors and renderers.
    pub(super) registry: RendererRegistry,
    /// Blocks that have been detected and (optionally) rendered.
    pub(super) active_blocks: VecDeque<PrettifiedBlock>,
    /// Base enabled state from config.
    enabled: bool,
    /// Per-session override for enabled state (from toggle).
    pub(super) session_override: Option<bool>,
    /// Monotonically increasing block ID counter.
    pub(super) next_block_id: u64,
    /// Terminal environment for renderers.
    pub(super) renderer_config: RendererConfig,
    /// Cache of rendered content to avoid re-rendering unchanged blocks.
    pub(super) render_cache: RenderCache,
    /// Row ranges where auto-detection is suppressed (via `prettify_format: "none"` triggers).
    pub(super) suppressed_ranges: Vec<Range<usize>>,
    /// Claude Code integration for session detection and expand/collapse tracking.
    pub(super) claude_code: ClaudeCodeIntegration,
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
        super::render::re_render_blocks(
            &mut self.active_blocks,
            &mut self.render_cache,
            &self.registry,
            &self.renderer_config,
            terminal_width,
        );
    }

    /// Get a reference to the render cache (for diagnostics).
    pub fn render_cache(&self) -> &RenderCache {
        &self.render_cache
    }

    /// Render content into a `DualViewBuffer`, using the cache when possible.
    pub(super) fn render_into_buffer(
        &mut self,
        buffer: &mut DualViewBuffer,
        format_id: &str,
        terminal_width: usize,
    ) {
        super::render::render_into_buffer(
            &mut self.render_cache,
            &self.registry,
            &self.renderer_config,
            buffer,
            format_id,
            terminal_width,
        );
    }
}

//! Prettifier pipeline: boundary detection → format detection → rendering.
//!
//! `PrettifierPipeline` wires together a `BoundaryDetector`, a `RendererRegistry`,
//! and block tracking into a single flow. Terminal output lines are fed in, content
//! blocks are emitted at boundaries, detected, rendered, and stored for display.

use super::boundary::{BoundaryConfig, BoundaryDetector, DetectionScope};
use super::registry::RendererRegistry;
use super::traits::RendererConfig;
use super::types::{
    ContentBlock, DetectionResult, DetectionSource, RenderedContent, ViewMode,
};

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
#[derive(Debug)]
pub struct PrettifiedBlock {
    /// The original content block.
    pub content: ContentBlock,
    /// The detection result that matched this block.
    pub detection: DetectionResult,
    /// The rendered output (None if rendering failed).
    pub rendered: Option<RenderedContent>,
    /// Current view mode (rendered vs source).
    pub view_mode: ViewMode,
    /// Unique identifier for this block within the session.
    pub block_id: u64,
}

/// Orchestrates boundary detection, format detection, and rendering.
pub struct PrettifierPipeline {
    /// Detects content block boundaries in the terminal output stream.
    boundary_detector: BoundaryDetector,
    /// Registry of detectors and renderers.
    registry: RendererRegistry,
    /// Blocks that have been detected and (optionally) rendered.
    active_blocks: Vec<PrettifiedBlock>,
    /// Base enabled state from config.
    enabled: bool,
    /// Per-session override for enabled state (from toggle).
    session_override: Option<bool>,
    /// Monotonically increasing block ID counter.
    next_block_id: u64,
    /// Terminal environment for renderers.
    renderer_config: RendererConfig,
}

impl PrettifierPipeline {
    /// Create a new pipeline from config, registry, and renderer config.
    pub fn new(
        config: PrettifierConfig,
        mut registry: RendererRegistry,
        renderer_config: RendererConfig,
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
            active_blocks: Vec::new(),
            enabled: config.enabled,
            session_override: None,
            next_block_id: 0,
            renderer_config,
        }
    }

    /// Feed a line of terminal output. May trigger block emission, detection,
    /// and rendering.
    pub fn process_output(&mut self, line: &str, row: usize) {
        if !self.is_enabled() {
            return;
        }
        if let Some(block) = self.boundary_detector.push_line(line, row) {
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
        let detection = DetectionResult {
            format_id: format_id.to_string(),
            confidence: 1.0,
            matched_rules: vec![],
            source: DetectionSource::TriggerInvoked,
        };

        let rendered = self.render_block(&content, format_id);
        let block_id = self.next_block_id;
        self.next_block_id += 1;

        self.active_blocks.push(PrettifiedBlock {
            content,
            detection,
            rendered,
            view_mode: ViewMode::Rendered,
            block_id,
        });
    }

    /// Toggle the global enabled state for this session.
    pub fn toggle_global(&mut self) {
        self.session_override = Some(!self.is_enabled());
    }

    /// Toggle the view mode for a specific block.
    pub fn toggle_block(&mut self, block_id: u64) {
        if let Some(block) = self.active_blocks.iter_mut().find(|b| b.block_id == block_id) {
            block.view_mode = match block.view_mode {
                ViewMode::Rendered => ViewMode::Source,
                ViewMode::Source => ViewMode::Rendered,
            };
        }
    }

    /// Get the list of active prettified blocks.
    pub fn active_blocks(&self) -> &[PrettifiedBlock] {
        &self.active_blocks
    }

    /// Find the prettified block that covers the given row.
    ///
    /// A block covers row `r` if `start_row <= r < end_row`.
    pub fn block_at_row(&self, row: usize) -> Option<&PrettifiedBlock> {
        self.active_blocks.iter().find(|b| {
            row >= b.content.start_row && row < b.content.end_row
        })
    }

    /// Whether the pipeline is effectively enabled.
    pub fn is_enabled(&self) -> bool {
        self.session_override.unwrap_or(self.enabled)
    }

    /// Update the renderer config (e.g., on terminal resize or theme change).
    pub fn update_renderer_config(&mut self, config: RendererConfig) {
        self.renderer_config = config;
    }

    /// Detect format and render a content block, storing it as a `PrettifiedBlock`.
    fn handle_block(&mut self, content: ContentBlock) {
        if let Some(detection) = self.registry.detect(&content) {
            let rendered = self.render_block(&content, &detection.format_id);
            let block_id = self.next_block_id;
            self.next_block_id += 1;

            self.active_blocks.push(PrettifiedBlock {
                content,
                detection,
                rendered,
                view_mode: ViewMode::Rendered,
                block_id,
            });
        }
    }

    /// Attempt to render a content block with the renderer for `format_id`.
    ///
    /// Returns `None` if no renderer is registered or rendering fails.
    fn render_block(&self, content: &ContentBlock, format_id: &str) -> Option<RenderedContent> {
        let renderer = self.registry.get_renderer(format_id)?;
        renderer.render(content, &self.renderer_config).ok()
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
        reg.register_detector(10, Box::new(AlwaysDetector { id: "test", confidence: 0.8 }));
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
        assert!(block.rendered.is_some());
        assert_eq!(block.view_mode, ViewMode::Rendered);
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
        assert!(block.rendered.is_some());
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

        assert_eq!(pipeline.active_blocks()[0].view_mode, ViewMode::Rendered);
        pipeline.toggle_block(0);
        assert_eq!(pipeline.active_blocks()[0].view_mode, ViewMode::Source);
        pipeline.toggle_block(0);
        assert_eq!(pipeline.active_blocks()[0].view_mode, ViewMode::Rendered);

        // Toggling a non-existent block is a no-op.
        pipeline.toggle_block(999);
    }

    #[test]
    fn test_is_enabled_with_session_override() {
        let config = PrettifierConfig {
            enabled: false,
            ..PrettifierConfig::default()
        };
        let mut pipeline = PrettifierPipeline::new(
            config,
            test_registry(0.5),
            RendererConfig::default(),
        );

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
        let mut pipeline = PrettifierPipeline::new(
            config,
            test_registry(0.5),
            RendererConfig::default(),
        );

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
        let mut pipeline = PrettifierPipeline::new(
            config,
            test_registry(0.5),
            RendererConfig::default(),
        );

        pipeline.on_command_start("echo hello");
        pipeline.process_output("hello", 0);
        pipeline.on_command_end();

        assert_eq!(pipeline.active_blocks().len(), 1);
        assert_eq!(pipeline.active_blocks()[0].content.preceding_command.as_deref(), Some("echo hello"));
    }

    #[test]
    fn test_render_failure_stores_none() {
        let mut reg = RendererRegistry::new(0.5);
        reg.register_detector(10, Box::new(AlwaysDetector { id: "fail", confidence: 0.8 }));
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
        assert!(pipeline.active_blocks()[0].rendered.is_none());
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

        let ids: Vec<u64> = pipeline.active_blocks().iter().map(|b| b.block_id).collect();
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
}

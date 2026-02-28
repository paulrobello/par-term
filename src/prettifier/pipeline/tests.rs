//! Tests for the prettifier pipeline.

use super::config::PrettifierConfig;
use super::pipeline_impl::PrettifierPipeline;
use crate::prettifier::boundary::DetectionScope;
use crate::prettifier::registry::RendererRegistry;
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
fn test_overlapping_block_replaces_existing() {
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

    // A viewport-sized per-frame feed with different content
    // replaces the existing block (throttle + hash dedup in
    // the render_pipeline prevent churn).
    let viewport_lines: Vec<(String, usize)> = (80..100)
        .map(|i| (format!("line {i} updated"), i))
        .collect();
    pipeline.submit_command_output(viewport_lines, None);

    // The old block is replaced by the smaller viewport block.
    assert_eq!(pipeline.active_blocks().len(), 1);
    assert_eq!(pipeline.active_blocks()[0].content().start_row, 80);
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
    let new_lines: Vec<(String, usize)> =
        (0..24).map(|i| (format!("updated line {i}"), i)).collect();
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

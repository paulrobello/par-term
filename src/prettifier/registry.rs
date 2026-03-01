//! Renderer registry and detection orchestrator.
//!
//! `RendererRegistry` holds all registered `ContentDetector`s and
//! `ContentRenderer`s, runs the detection pipeline against content blocks,
//! and provides access to renderers by format ID.

use std::collections::HashMap;

use super::traits::{ContentDetector, ContentRenderer};
use super::types::{ContentBlock, DetectionResult};

/// Central registry for content detectors and renderers.
///
/// Detectors are stored in priority-descending order. Detection runs each
/// detector in order, keeping the result with the highest confidence.
pub struct RendererRegistry {
    /// Detectors sorted by priority (highest first). Each entry is (priority, detector).
    detectors: Vec<(i32, Box<dyn ContentDetector>)>,
    /// Renderers keyed by format_id.
    renderers: HashMap<String, Box<dyn ContentRenderer>>,
    /// Minimum confidence required for a detection to be accepted.
    confidence_threshold: f32,
}

impl RendererRegistry {
    /// Create an empty registry with the given global confidence threshold.
    pub fn new(confidence_threshold: f32) -> Self {
        Self {
            detectors: Vec::new(),
            renderers: HashMap::new(),
            confidence_threshold,
        }
    }

    /// Register a detector at the given priority (higher = checked first).
    ///
    /// Maintains descending sort by priority. Within the same priority,
    /// detectors are checked in registration order (FIFO): earlier-registered
    /// detectors are checked first and win on equal confidence.
    pub fn register_detector(&mut self, priority: i32, detector: Box<dyn ContentDetector>) {
        let idx = self.detectors.partition_point(|(p, _)| *p >= priority);
        self.detectors.insert(idx, (priority, detector));
    }

    /// Register a renderer for a format ID.
    pub fn register_renderer(
        &mut self,
        format_id: impl Into<String>,
        renderer: Box<dyn ContentRenderer>,
    ) {
        self.renderers.insert(format_id.into(), renderer);
    }

    /// Look up a renderer by format ID.
    pub fn get_renderer(&self, format_id: &str) -> Option<&dyn ContentRenderer> {
        self.renderers.get(format_id).map(|r| r.as_ref())
    }

    /// Return `(format_id, display_name)` pairs for all registered renderers.
    pub fn registered_formats(&self) -> Vec<(&str, &str)> {
        self.renderers
            .iter()
            .map(|(id, r)| (id.as_str(), r.display_name()))
            .collect()
    }

    /// Run the detection pipeline against a content block.
    ///
    /// 1. Extract `first_lines(5)` for quick_match filtering.
    /// 2. For each detector (priority descending): skip if quick_match fails,
    ///    else call `detect()`.
    /// 3. Keep the result with the highest confidence. Priority is the tiebreaker
    ///    (higher-priority detector wins on equal confidence because it's checked first
    ///    and we only replace on strictly greater confidence).
    /// 4. Return the best result if its confidence meets the threshold, else `None`.
    pub fn detect(&self, content: &ContentBlock) -> Option<DetectionResult> {
        // Sample more lines for quick_match: 5 was too few for content that
        // has preamble (e.g. Claude Code UI) before the structured data.
        let first_lines = content.first_lines(30);
        let mut best: Option<DetectionResult> = None;

        crate::debug_log!(
            "PRETTIFIER",
            "registry::detect: running {} detectors against {} lines (rows={}..{}), first_line={:?}",
            self.detectors.len(),
            content.lines.len(),
            content.start_row,
            content.end_row,
            content
                .lines
                .first()
                .map(|l| &l[..l.floor_char_boundary(60)])
        );

        for (priority, detector) in &self.detectors {
            if !detector.quick_match(&first_lines) {
                crate::debug_trace!(
                    "PRETTIFIER",
                    "registry::detect: {} (priority={}) quick_match=false, skipping",
                    detector.format_id(),
                    priority
                );
                continue;
            }

            if let Some(result) = detector.detect(content) {
                crate::debug_log!(
                    "PRETTIFIER",
                    "registry::detect: {} (priority={}) detected confidence={:.3}, rules={:?}",
                    detector.format_id(),
                    priority,
                    result.confidence,
                    &result.matched_rules[..result.matched_rules.len().min(5)]
                );
                let dominated = match &best {
                    Some(current) => result.confidence > current.confidence,
                    None => true,
                };
                if dominated {
                    best = Some(result);
                }
            } else {
                crate::debug_trace!(
                    "PRETTIFIER",
                    "registry::detect: {} (priority={}) quick_match=true but detect()=None",
                    detector.format_id(),
                    priority
                );
            }
        }

        let result = best.filter(|r| r.confidence >= self.confidence_threshold);
        match &result {
            Some(r) => {
                crate::debug_info!(
                    "PRETTIFIER",
                    "registry::detect: WINNER format={}, confidence={:.3} (threshold={:.3})",
                    r.format_id,
                    r.confidence,
                    self.confidence_threshold
                );
            }
            None => {
                crate::debug_log!(
                    "PRETTIFIER",
                    "registry::detect: no format met threshold {:.3}",
                    self.confidence_threshold
                );
            }
        }
        result
    }

    /// Set the global confidence threshold.
    pub fn set_confidence_threshold(&mut self, threshold: f32) {
        self.confidence_threshold = threshold;
    }

    /// Get the current confidence threshold.
    pub fn confidence_threshold(&self) -> f32 {
        self.confidence_threshold
    }

    /// Number of registered detectors.
    pub fn detector_count(&self) -> usize {
        self.detectors.len()
    }

    /// Number of registered renderers.
    pub fn renderer_count(&self) -> usize {
        self.renderers.len()
    }

    /// Apply rule overrides and additional rules from config to a specific format's detector.
    pub fn apply_rules_for_format(
        &mut self,
        format_id: &str,
        overrides: &[crate::config::prettifier::RuleOverride],
        additional: Vec<super::types::DetectionRule>,
    ) {
        for (_priority, detector) in &mut self.detectors {
            if detector.format_id() == format_id {
                if !overrides.is_empty() {
                    detector.apply_config_overrides(overrides);
                }
                if !additional.is_empty() {
                    detector.merge_config_rules(additional);
                }
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::testing::make_block;
    use crate::prettifier::traits::*;
    use crate::prettifier::types::*;

    /// Minimal detector for testing.
    struct MockDetector {
        id: &'static str,
        name: &'static str,
        quick: bool,
        confidence: Option<f32>,
    }

    impl ContentDetector for MockDetector {
        fn format_id(&self) -> &str {
            self.id
        }
        fn display_name(&self) -> &str {
            self.name
        }
        fn detect(&self, _content: &ContentBlock) -> Option<DetectionResult> {
            self.confidence.map(|c| DetectionResult {
                format_id: self.id.to_string(),
                confidence: c,
                matched_rules: vec!["mock_rule".to_string()],
                source: DetectionSource::AutoDetected,
            })
        }
        fn quick_match(&self, _first_lines: &[&str]) -> bool {
            self.quick
        }
        fn detection_rules(&self) -> &[DetectionRule] {
            &[]
        }
    }

    /// Minimal renderer for testing.
    struct MockRenderer {
        id: &'static str,
        name: &'static str,
    }

    impl ContentRenderer for MockRenderer {
        fn format_id(&self) -> &str {
            self.id
        }
        fn display_name(&self) -> &str {
            self.name
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
                format_badge: "MOCK".to_string(),
            })
        }
        fn format_badge(&self) -> &str {
            "MOCK"
        }
    }

    #[test]
    fn test_priority_ordering() {
        let mut reg = RendererRegistry::new(0.5);
        reg.register_detector(
            10,
            Box::new(MockDetector {
                id: "low",
                name: "Low",
                quick: true,
                confidence: Some(0.7),
            }),
        );
        reg.register_detector(
            50,
            Box::new(MockDetector {
                id: "high",
                name: "High",
                quick: true,
                confidence: Some(0.7),
            }),
        );
        reg.register_detector(
            30,
            Box::new(MockDetector {
                id: "mid",
                name: "Mid",
                quick: true,
                confidence: Some(0.7),
            }),
        );

        // With equal confidence, priority wins (high is checked first, keeps it).
        let result = reg.detect(&make_block(&["test"])).unwrap();
        assert_eq!(result.format_id, "high");
    }

    #[test]
    fn test_quick_match_filtering() {
        let mut reg = RendererRegistry::new(0.5);
        reg.register_detector(
            10,
            Box::new(MockDetector {
                id: "skipped",
                name: "Skipped",
                quick: false,
                confidence: Some(0.9),
            }),
        );
        reg.register_detector(
            5,
            Box::new(MockDetector {
                id: "passes",
                name: "Passes",
                quick: true,
                confidence: Some(0.6),
            }),
        );

        let result = reg.detect(&make_block(&["test"])).unwrap();
        // The higher-confidence detector is skipped by quick_match.
        assert_eq!(result.format_id, "passes");
    }

    #[test]
    fn test_highest_confidence_wins() {
        let mut reg = RendererRegistry::new(0.5);
        reg.register_detector(
            50,
            Box::new(MockDetector {
                id: "lower",
                name: "Lower",
                quick: true,
                confidence: Some(0.6),
            }),
        );
        reg.register_detector(
            10,
            Box::new(MockDetector {
                id: "higher",
                name: "Higher",
                quick: true,
                confidence: Some(0.9),
            }),
        );

        let result = reg.detect(&make_block(&["test"])).unwrap();
        assert_eq!(result.format_id, "higher");
    }

    #[test]
    fn test_threshold_filtering() {
        let mut reg = RendererRegistry::new(0.8);
        reg.register_detector(
            10,
            Box::new(MockDetector {
                id: "weak",
                name: "Weak",
                quick: true,
                confidence: Some(0.5),
            }),
        );

        assert!(reg.detect(&make_block(&["test"])).is_none());
    }

    #[test]
    fn test_registered_formats() {
        let mut reg = RendererRegistry::new(0.5);
        reg.register_renderer(
            "md",
            Box::new(MockRenderer {
                id: "md",
                name: "Markdown",
            }),
        );
        reg.register_renderer(
            "json",
            Box::new(MockRenderer {
                id: "json",
                name: "JSON",
            }),
        );

        let mut formats = reg.registered_formats();
        formats.sort_by_key(|(id, _)| id.to_string());
        assert_eq!(formats.len(), 2);
        assert_eq!(formats[0], ("json", "JSON"));
        assert_eq!(formats[1], ("md", "Markdown"));
    }

    #[test]
    fn test_get_renderer() {
        let mut reg = RendererRegistry::new(0.5);
        reg.register_renderer(
            "md",
            Box::new(MockRenderer {
                id: "md",
                name: "Markdown",
            }),
        );

        assert!(reg.get_renderer("md").is_some());
        assert_eq!(reg.get_renderer("md").unwrap().display_name(), "Markdown");
        assert!(reg.get_renderer("nonexistent").is_none());
    }

    #[test]
    fn test_empty_registry() {
        let reg = RendererRegistry::new(0.5);
        assert!(reg.detect(&make_block(&["test"])).is_none());
        assert_eq!(reg.detector_count(), 0);
        assert_eq!(reg.renderer_count(), 0);
        assert!(reg.registered_formats().is_empty());
    }

    #[test]
    fn test_sorted_insertion() {
        let mut reg = RendererRegistry::new(0.5);
        reg.register_detector(
            20,
            Box::new(MockDetector {
                id: "b",
                name: "B",
                quick: true,
                confidence: None,
            }),
        );
        reg.register_detector(
            50,
            Box::new(MockDetector {
                id: "a",
                name: "A",
                quick: true,
                confidence: None,
            }),
        );
        reg.register_detector(
            30,
            Box::new(MockDetector {
                id: "c",
                name: "C",
                quick: true,
                confidence: None,
            }),
        );
        reg.register_detector(
            50,
            Box::new(MockDetector {
                id: "d",
                name: "D",
                quick: true,
                confidence: None,
            }),
        );

        let priorities: Vec<i32> = reg.detectors.iter().map(|(p, _)| *p).collect();
        assert_eq!(priorities, vec![50, 50, 30, 20]);
        assert_eq!(reg.detector_count(), 4);
    }

    #[test]
    fn test_detect_returns_none_when_all_fail() {
        let mut reg = RendererRegistry::new(0.5);
        reg.register_detector(
            10,
            Box::new(MockDetector {
                id: "none",
                name: "None",
                quick: true,
                confidence: None,
            }),
        );

        assert!(reg.detect(&make_block(&["test"])).is_none());
    }
}

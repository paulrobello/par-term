//! Claude Code integration for the Content Prettifier.
//!
//! Detects Claude Code sessions, tracks expand/collapse state of output blocks
//! (Ctrl+O interactions), renders format badges on collapsed blocks, and handles
//! multi-format responses (Markdown with embedded JSON, diffs, and diagrams).

use std::collections::HashMap;
use std::ops::Range;

use crate::config::prettifier::ClaudeCodeConfig;
use crate::prettifier::types::{ContentBlock, DetectionResult};

// ---------------------------------------------------------------------------
// Expand/collapse state tracking
// ---------------------------------------------------------------------------

/// The expand/collapse state of a Claude Code output block.
#[derive(Debug, Clone)]
pub enum ExpandState {
    /// Content is collapsed (showing "(ctrl+o to expand)").
    Collapsed {
        /// Preview content (first header + format badge).
        preview: Option<RenderedPreview>,
    },
    /// Content is expanded (full content visible).
    Expanded {
        /// Whether the prettifier has processed this expanded content.
        prettified: bool,
    },
}

/// A preview shown for a collapsed Claude Code block.
#[derive(Debug, Clone)]
pub struct RenderedPreview {
    /// Format badge text (e.g., "MD Markdown", "{} JSON").
    pub format_badge: String,
    /// First header extracted from the content (if markdown).
    pub first_header: Option<String>,
    /// Summary of the content (e.g., "42 lines").
    pub content_summary: String,
}

// ---------------------------------------------------------------------------
// Events emitted by the integration
// ---------------------------------------------------------------------------

/// Events emitted by the Claude Code integration layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeCodeEvent {
    /// User pressed Ctrl+O to expand — trigger prettifier on newly visible content.
    ContentExpanded { row_range: Range<usize> },
    /// Content collapsed — show preview with format badge.
    ContentCollapsed { row_range: Range<usize> },
    /// Detected a Claude Code format indicator in output.
    FormatDetected { format: String },
}

// ---------------------------------------------------------------------------
// Ctrl+O pattern detection
// ---------------------------------------------------------------------------

/// Classification of a Ctrl+O-related pattern in terminal output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CtrlOPattern {
    /// The line is a collapse marker — content is hidden behind "ctrl+o to expand".
    CollapseMarker,
}

/// Detect the Claude Code expand/collapse pattern in a terminal output line.
fn detect_ctrl_o_pattern(line: &str) -> Option<CtrlOPattern> {
    let lower = line.to_ascii_lowercase();
    if lower.contains("(ctrl+o to expand)") || lower.contains("ctrl+o") {
        return Some(CtrlOPattern::CollapseMarker);
    }
    None
}

// ---------------------------------------------------------------------------
// ClaudeCodeIntegration
// ---------------------------------------------------------------------------

/// Manages Claude Code integration for the prettifier system.
///
/// Responsibilities:
/// - Detect Claude Code sessions via environment variables or process name.
/// - Track expand/collapse states of output blocks.
/// - Generate previews with format badges for collapsed blocks.
/// - Emit events when content is expanded or collapsed.
pub struct ClaudeCodeIntegration {
    config: ClaudeCodeConfig,
    /// Whether we've detected a Claude Code session.
    is_claude_code_session: bool,
    /// Tracks expanded/collapsed state of Claude Code output blocks, keyed by block ID.
    expand_states: HashMap<u64, ExpandState>,
    /// Maps row indices to block IDs for collapse-marker lookups.
    row_to_block: HashMap<usize, u64>,
    /// Next synthetic block ID for collapse-marker tracking.
    next_collapse_id: u64,
}

impl ClaudeCodeIntegration {
    /// Create a new integration with the given config.
    pub fn new(config: ClaudeCodeConfig) -> Self {
        Self {
            config,
            is_claude_code_session: false,
            expand_states: HashMap::new(),
            row_to_block: HashMap::new(),
            next_collapse_id: 0,
        }
    }

    /// Whether this is a detected Claude Code session.
    pub fn is_active(&self) -> bool {
        self.is_claude_code_session
    }

    /// Access the integration config.
    pub fn config(&self) -> &ClaudeCodeConfig {
        &self.config
    }

    /// Detect if this is a Claude Code session.
    ///
    /// Checks (in order):
    /// 1. `CLAUDE_CODE` environment variable is set.
    /// 2. Process name contains "claude".
    pub fn detect_session(
        &mut self,
        env_vars: &HashMap<String, String>,
        process_name: &str,
    ) -> bool {
        if !self.config.auto_detect {
            return false;
        }

        // 1. Check CLAUDE_CODE environment variable.
        if env_vars.contains_key("CLAUDE_CODE") {
            self.is_claude_code_session = true;
            return true;
        }

        // 2. Check process name.
        let lower_name = process_name.to_ascii_lowercase();
        if lower_name.contains("claude") {
            self.is_claude_code_session = true;
            return true;
        }

        false
    }

    /// Handle a line of terminal output in the context of Claude Code.
    ///
    /// Returns a `ClaudeCodeEvent` if the line is a Claude Code control pattern
    /// (expand/collapse marker). Returns `None` for regular output lines.
    pub fn process_line(&mut self, line: &str, row: usize) -> Option<ClaudeCodeEvent> {
        if !self.is_claude_code_session {
            return None;
        }

        if let Some(CtrlOPattern::CollapseMarker) = detect_ctrl_o_pattern(line) {
            // Register a new collapsed block at this row.
            let block_id = self.next_collapse_id;
            self.next_collapse_id += 1;

            self.expand_states.insert(
                block_id,
                ExpandState::Collapsed {
                    preview: None, // Preview will be set when content is detected.
                },
            );
            self.row_to_block.insert(row, block_id);

            return Some(ClaudeCodeEvent::ContentCollapsed {
                row_range: row..row + 1,
            });
        }

        None
    }

    /// Mark a block as expanded and return the event.
    ///
    /// Called when Ctrl+O expands a previously collapsed block.
    pub fn on_expand(&mut self, block_id: u64, row_range: Range<usize>) -> Option<ClaudeCodeEvent> {
        if let Some(state) = self.expand_states.get_mut(&block_id) {
            *state = ExpandState::Expanded { prettified: false };
            return Some(ClaudeCodeEvent::ContentExpanded { row_range });
        }
        None
    }

    /// Mark a block as collapsed with an optional preview.
    pub fn on_collapse(
        &mut self,
        block_id: u64,
        row_range: Range<usize>,
        preview: Option<RenderedPreview>,
    ) -> Option<ClaudeCodeEvent> {
        if let Some(state) = self.expand_states.get_mut(&block_id) {
            *state = ExpandState::Collapsed { preview };
            return Some(ClaudeCodeEvent::ContentCollapsed { row_range });
        }
        None
    }

    /// Mark a block's expanded content as having been prettified.
    pub fn mark_prettified(&mut self, block_id: u64) {
        if let Some(ExpandState::Expanded { prettified }) = self.expand_states.get_mut(&block_id) {
            *prettified = true;
        }
    }

    /// Check if a row is in a collapsed Claude Code block.
    pub fn is_collapsed(&self, row: usize) -> bool {
        if let Some(&block_id) = self.row_to_block.get(&row) {
            matches!(
                self.expand_states.get(&block_id),
                Some(ExpandState::Collapsed { .. })
            )
        } else {
            false
        }
    }

    /// Get preview content for a collapsed block.
    pub fn get_preview(&self, block_id: u64) -> Option<&RenderedPreview> {
        match self.expand_states.get(&block_id) {
            Some(ExpandState::Collapsed {
                preview: Some(preview),
            }) => Some(preview),
            _ => None,
        }
    }

    /// Get the expand state for a block.
    pub fn get_state(&self, block_id: u64) -> Option<&ExpandState> {
        self.expand_states.get(&block_id)
    }

    /// Look up the block ID associated with a row (if any).
    pub fn block_id_at_row(&self, row: usize) -> Option<u64> {
        self.row_to_block.get(&row).copied()
    }

    /// Generate a preview for a collapsed block based on detection results.
    pub fn generate_preview(
        content: &ContentBlock,
        detection: &DetectionResult,
        show_badges: bool,
    ) -> RenderedPreview {
        let format_badge = if show_badges {
            match detection.format_id.as_str() {
                "markdown" => "MD Markdown".to_string(),
                "json" => "{} JSON".to_string(),
                "diagrams" => "Diagram".to_string(),
                "yaml" => "YAML".to_string(),
                "diff" => "± Diff".to_string(),
                other => other.to_string(),
            }
        } else {
            String::new()
        };

        // Extract first header from content (if markdown).
        let first_header = content
            .lines
            .iter()
            .find(|l| l.starts_with('#'))
            .map(|l| l.trim_start_matches('#').trim().to_string());

        RenderedPreview {
            format_badge,
            first_header,
            content_summary: format!("{} lines", content.lines.len()),
        }
    }
}

// ---------------------------------------------------------------------------
// MarkdownRenderer sub-rendering support
// ---------------------------------------------------------------------------

/// Check if a fenced code block language should be sub-rendered by another renderer.
///
/// Returns `true` for languages that have dedicated renderers in the registry
/// (e.g., Mermaid diagrams, PlantUML).
pub fn should_sub_render(language: &str, registry: &super::registry::RendererRegistry) -> bool {
    registry.get_renderer(language).is_some()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn default_config() -> ClaudeCodeConfig {
        ClaudeCodeConfig::default()
    }

    fn make_env(vars: &[(&str, &str)]) -> HashMap<String, String> {
        vars.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    // -- Session detection --

    #[test]
    fn test_detect_session_via_env_var() {
        let mut integration = ClaudeCodeIntegration::new(default_config());
        let env = make_env(&[("CLAUDE_CODE", "1")]);

        assert!(integration.detect_session(&env, "bash"));
        assert!(integration.is_active());
    }

    #[test]
    fn test_detect_session_via_process_name() {
        let mut integration = ClaudeCodeIntegration::new(default_config());
        let env = make_env(&[]);

        assert!(integration.detect_session(&env, "claude-code"));
        assert!(integration.is_active());
    }

    #[test]
    fn test_detect_session_case_insensitive_process() {
        let mut integration = ClaudeCodeIntegration::new(default_config());
        let env = make_env(&[]);

        assert!(integration.detect_session(&env, "Claude"));
        assert!(integration.is_active());
    }

    #[test]
    fn test_detect_session_no_match() {
        let mut integration = ClaudeCodeIntegration::new(default_config());
        let env = make_env(&[("HOME", "/Users/test")]);

        assert!(!integration.detect_session(&env, "bash"));
        assert!(!integration.is_active());
    }

    #[test]
    fn test_detect_session_auto_detect_disabled() {
        let config = ClaudeCodeConfig {
            auto_detect: false,
            ..default_config()
        };
        let mut integration = ClaudeCodeIntegration::new(config);
        let env = make_env(&[("CLAUDE_CODE", "1")]);

        assert!(!integration.detect_session(&env, "claude"));
        assert!(!integration.is_active());
    }

    // -- Ctrl+O pattern detection --

    #[test]
    fn test_detect_ctrl_o_pattern() {
        assert_eq!(
            detect_ctrl_o_pattern("  (ctrl+o to expand)  "),
            Some(CtrlOPattern::CollapseMarker)
        );
    }

    #[test]
    fn test_detect_ctrl_o_pattern_case_insensitive() {
        assert_eq!(
            detect_ctrl_o_pattern("(Ctrl+O to expand)"),
            Some(CtrlOPattern::CollapseMarker)
        );
    }

    #[test]
    fn test_detect_ctrl_o_no_match() {
        assert_eq!(detect_ctrl_o_pattern("regular output line"), None);
    }

    // -- process_line events --

    #[test]
    fn test_process_line_inactive_session() {
        let mut integration = ClaudeCodeIntegration::new(default_config());
        // Not a Claude Code session, so process_line should always return None.
        assert!(integration.process_line("(ctrl+o to expand)", 0).is_none());
    }

    #[test]
    fn test_process_line_collapse_marker() {
        let mut integration = ClaudeCodeIntegration::new(default_config());
        integration.is_claude_code_session = true;

        let event = integration.process_line("(ctrl+o to expand)", 5);
        assert_eq!(
            event,
            Some(ClaudeCodeEvent::ContentCollapsed { row_range: 5..6 })
        );

        // Row should be tracked as collapsed.
        assert!(integration.is_collapsed(5));
    }

    #[test]
    fn test_process_line_regular_line() {
        let mut integration = ClaudeCodeIntegration::new(default_config());
        integration.is_claude_code_session = true;

        assert!(integration.process_line("# Hello World", 0).is_none());
    }

    // -- Expand/collapse state management --

    #[test]
    fn test_expand_collapse_cycle() {
        let mut integration = ClaudeCodeIntegration::new(default_config());
        integration.is_claude_code_session = true;

        // Collapse triggers state creation.
        integration.process_line("(ctrl+o to expand)", 10);
        let block_id = integration.block_id_at_row(10).unwrap();
        assert!(integration.is_collapsed(10));

        // Expand.
        let event = integration.on_expand(block_id, 10..20);
        assert_eq!(
            event,
            Some(ClaudeCodeEvent::ContentExpanded { row_range: 10..20 })
        );
        assert!(!integration.is_collapsed(10));

        // Collapse again with preview.
        let preview = RenderedPreview {
            format_badge: "MD Markdown".to_string(),
            first_header: Some("Test".to_string()),
            content_summary: "5 lines".to_string(),
        };
        let event = integration.on_collapse(block_id, 10..11, Some(preview));
        assert_eq!(
            event,
            Some(ClaudeCodeEvent::ContentCollapsed { row_range: 10..11 })
        );
        assert!(integration.is_collapsed(10));
    }

    #[test]
    fn test_mark_prettified() {
        let mut integration = ClaudeCodeIntegration::new(default_config());
        integration.is_claude_code_session = true;

        integration.process_line("(ctrl+o to expand)", 0);
        let block_id = integration.block_id_at_row(0).unwrap();
        integration.on_expand(block_id, 0..10);

        // Before marking: not prettified.
        match integration.get_state(block_id) {
            Some(ExpandState::Expanded { prettified }) => assert!(!prettified),
            _ => panic!("Expected Expanded state"),
        }

        integration.mark_prettified(block_id);

        // After marking: prettified.
        match integration.get_state(block_id) {
            Some(ExpandState::Expanded { prettified }) => assert!(*prettified),
            _ => panic!("Expected Expanded state"),
        }
    }

    // -- Preview generation --

    #[test]
    fn test_generate_preview_markdown() {
        let content = ContentBlock {
            lines: vec![
                "# Introduction".to_string(),
                "Some text here.".to_string(),
                "More content.".to_string(),
            ],
            preceding_command: None,
            start_row: 0,
            end_row: 3,
            timestamp: SystemTime::now(),
        };
        let detection = DetectionResult {
            format_id: "markdown".to_string(),
            confidence: 0.9,
            matched_rules: vec![],
            source: crate::prettifier::types::DetectionSource::AutoDetected,
        };

        let preview = ClaudeCodeIntegration::generate_preview(&content, &detection, true);
        assert_eq!(preview.format_badge, "MD Markdown");
        assert_eq!(preview.first_header.as_deref(), Some("Introduction"));
        assert_eq!(preview.content_summary, "3 lines");
    }

    #[test]
    fn test_generate_preview_json() {
        let content = ContentBlock {
            lines: vec![
                "{".to_string(),
                "  \"key\": \"value\"".to_string(),
                "}".to_string(),
            ],
            preceding_command: None,
            start_row: 0,
            end_row: 3,
            timestamp: SystemTime::now(),
        };
        let detection = DetectionResult {
            format_id: "json".to_string(),
            confidence: 0.95,
            matched_rules: vec![],
            source: crate::prettifier::types::DetectionSource::AutoDetected,
        };

        let preview = ClaudeCodeIntegration::generate_preview(&content, &detection, true);
        assert_eq!(preview.format_badge, "{} JSON");
        assert!(preview.first_header.is_none());
        assert_eq!(preview.content_summary, "3 lines");
    }

    #[test]
    fn test_generate_preview_badges_disabled() {
        let content = ContentBlock {
            lines: vec!["test".to_string()],
            preceding_command: None,
            start_row: 0,
            end_row: 1,
            timestamp: SystemTime::now(),
        };
        let detection = DetectionResult {
            format_id: "markdown".to_string(),
            confidence: 0.9,
            matched_rules: vec![],
            source: crate::prettifier::types::DetectionSource::AutoDetected,
        };

        let preview = ClaudeCodeIntegration::generate_preview(&content, &detection, false);
        assert!(preview.format_badge.is_empty());
    }

    #[test]
    fn test_get_preview() {
        let mut integration = ClaudeCodeIntegration::new(default_config());
        integration.is_claude_code_session = true;

        integration.process_line("(ctrl+o to expand)", 0);
        let block_id = integration.block_id_at_row(0).unwrap();

        // Initially no preview.
        assert!(integration.get_preview(block_id).is_none());

        // Collapse with preview.
        let preview = RenderedPreview {
            format_badge: "MD Markdown".to_string(),
            first_header: Some("Title".to_string()),
            content_summary: "10 lines".to_string(),
        };
        integration.on_collapse(block_id, 0..1, Some(preview));

        let p = integration.get_preview(block_id).unwrap();
        assert_eq!(p.format_badge, "MD Markdown");
        assert_eq!(p.first_header.as_deref(), Some("Title"));
    }

    // -- Non-Claude sessions unaffected --

    #[test]
    fn test_non_claude_session_unaffected() {
        let mut integration = ClaudeCodeIntegration::new(default_config());
        // Not detected as Claude Code session.
        assert!(!integration.is_active());
        assert!(!integration.is_collapsed(0));
        assert!(integration.process_line("(ctrl+o to expand)", 0).is_none());
    }

    // -- Sub-rendering --

    #[test]
    fn test_should_sub_render() {
        use crate::prettifier::registry::RendererRegistry;
        use crate::prettifier::traits::*;
        use crate::prettifier::types::*;

        struct MockRenderer;
        impl ContentRenderer for MockRenderer {
            fn format_id(&self) -> &str {
                "mermaid"
            }
            fn display_name(&self) -> &str {
                "Mermaid"
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
                    lines: vec![],
                    line_mapping: vec![],
                    graphics: vec![],
                    format_badge: "MERMAID".to_string(),
                })
            }
            fn format_badge(&self) -> &str {
                "MERMAID"
            }
        }

        let mut registry = RendererRegistry::new(0.5);
        registry.register_renderer("mermaid", Box::new(MockRenderer));

        assert!(should_sub_render("mermaid", &registry));
        assert!(!should_sub_render("python", &registry));
    }
}

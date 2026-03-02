//! Core traits for the Content Prettifier framework.

use super::types::{
    ContentBlock, DetectionResult, DetectionRule, RenderedContent, RendererCapability,
};

/// Configuration passed to renderers describing the terminal environment.
#[derive(Debug, Clone)]
pub struct RendererConfig {
    /// Terminal width in columns.
    pub terminal_width: usize,
    /// Theme colors for styling rendered output.
    pub theme_colors: ThemeColors,
    /// Cell width in pixels (for sizing inline graphics).
    pub cell_width_px: Option<f32>,
    /// Cell height in pixels (for sizing inline graphics).
    pub cell_height_px: Option<f32>,
    /// Allowlist of command names (basename or full path) that `ExternalCommandRenderer`
    /// is permitted to execute. When non-empty, any command not listed here will be
    /// refused and a warning logged. When empty (the default), all commands are allowed
    /// but a warning is emitted for every execution.
    ///
    /// Populated from `prettifier.allowed_commands` in the user config.
    pub allowed_commands: Vec<String>,
}

/// Color palette from the current terminal theme.
#[derive(Debug, Clone)]
pub struct ThemeColors {
    /// Default foreground color [r, g, b].
    pub fg: [u8; 3],
    /// Default background color [r, g, b].
    pub bg: [u8; 3],
    /// The 16 ANSI colors [r, g, b] (indices 0–15).
    pub palette: [[u8; 3]; 16],
}

impl ThemeColors {
    /// Dim grey — tree guides, comments, separators (palette index 8).
    pub fn dim_color(&self) -> [u8; 3] {
        self.palette[8]
    }

    /// Green — string values (palette index 2).
    pub fn string_color(&self) -> [u8; 3] {
        self.palette[2]
    }

    /// Cyan/teal — keys and identifiers (palette index 6).
    pub fn key_color(&self) -> [u8; 3] {
        self.palette[6]
    }

    /// Red — errors and deletions (palette index 1).
    pub fn error_color(&self) -> [u8; 3] {
        self.palette[1]
    }

    /// Bright yellow — numbers (palette index 11).
    pub fn number_color(&self) -> [u8; 3] {
        self.palette[11]
    }

    /// Dim grey — inline comments (same as `dim_color`, palette index 8).
    pub fn comment_color(&self) -> [u8; 3] {
        self.palette[8]
    }

    /// Bright cyan — accent color (palette index 14).
    pub fn accent_color(&self) -> [u8; 3] {
        self.palette[14]
    }
}

impl Default for ThemeColors {
    /// Modern Catppuccin Mocha-inspired palette for vibrant, readable output.
    fn default() -> Self {
        Self {
            fg: [205, 214, 244],
            bg: [30, 30, 46],
            palette: [
                [69, 71, 90],    // 0  Black (Surface0)
                [243, 139, 168], // 1  Red
                [166, 227, 161], // 2  Green
                [249, 226, 175], // 3  Yellow (warm gold)
                [137, 180, 250], // 4  Blue
                [203, 166, 247], // 5  Magenta (mauve)
                [148, 226, 213], // 6  Cyan (teal)
                [186, 194, 222], // 7  White (Subtext0)
                [108, 112, 134], // 8  Bright black (Overlay0)
                [235, 160, 172], // 9  Bright red (maroon)
                [166, 227, 161], // 10 Bright green
                [249, 226, 175], // 11 Bright yellow
                [116, 199, 236], // 12 Bright blue (sapphire)
                [245, 194, 231], // 13 Bright magenta (pink)
                [137, 220, 235], // 14 Bright cyan (sky)
                [205, 214, 244], // 15 Bright white (Text)
            ],
        }
    }
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            terminal_width: 80,
            theme_colors: ThemeColors::default(),
            cell_width_px: None,
            cell_height_px: None,
            allowed_commands: Vec::new(),
        }
    }
}

/// Errors that can occur during content rendering.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// The renderer failed to produce output.
    #[error("render failed: {0}")]
    RenderFailed(String),
    /// A required external command was not found.
    #[error("command not found: {0}")]
    CommandNotFound(String),
    /// A network request failed.
    #[error("network error: {0}")]
    NetworkError(String),
    /// The rendering operation timed out.
    #[error("render timed out after {0}ms")]
    Timeout(u64),
}

/// Identifies whether a content block matches a specific format.
///
/// Implementations must be `Send + Sync` for use across threads.
pub trait ContentDetector: Send + Sync {
    /// Unique identifier for this format (e.g., "markdown", "json", "mermaid").
    fn format_id(&self) -> &str;

    /// Human-readable name for the settings UI.
    fn display_name(&self) -> &str;

    /// Analyze a content block and return a detection result with confidence score.
    /// Returns `None` if this detector cannot handle the content at all.
    fn detect(&self, content: &ContentBlock) -> Option<DetectionResult>;

    /// Quick check — can this detector potentially match this content?
    /// Used for fast filtering before running full detection.
    fn quick_match(&self, first_lines: &[&str]) -> bool;

    /// Return the regex rules powering this detector (for UI inspection/editing).
    fn detection_rules(&self) -> &[DetectionRule];

    /// Whether this detector allows user-added regex rules via config.
    fn accepts_custom_rules(&self) -> bool {
        true
    }

    /// Attempt to obtain a mutable reference to this detector as a
    /// [`ConfigurableDetector`].
    ///
    /// Returns `None` for detectors that do not support runtime config overrides.
    /// `RegexDetector` returns `Some(self)`. This replaces the previous pattern of
    /// empty default implementations of `apply_config_overrides` / `merge_config_rules`
    /// on the base trait, which silently did nothing for non-`RegexDetector` types.
    fn as_configurable_mut(&mut self) -> Option<&mut dyn ConfigurableDetector> {
        None
    }
}

/// Extension trait for detectors that support user-config overrides and rule merging.
///
/// Only `RegexDetector` implements this trait. Moving these methods out of
/// `ContentDetector` eliminates the silent no-op anti-pattern: calling
/// `apply_config_overrides` on a non-configurable detector now requires explicitly
/// opting in via `as_configurable_mut()`, making the "no-op for other types" case
/// an explicit `None` check rather than a hidden default.
///
/// # Usage
///
/// ```ignore
/// if let Some(cd) = detector.as_configurable_mut() {
///     cd.apply_config_overrides(&overrides);
///     cd.merge_config_rules(rules);
/// }
/// ```
pub trait ConfigurableDetector: ContentDetector {
    /// Apply rule overrides (enable/disable, weight changes) from user config.
    fn apply_config_overrides(&mut self, overrides: &[crate::config::prettifier::RuleOverride]);

    /// Merge additional user-defined rules from config.
    fn merge_config_rules(&mut self, rules: Vec<DetectionRule>);
}

/// Renders detected content into styled terminal output.
///
/// Implementations must be `Send + Sync` for use across threads.
pub trait ContentRenderer: Send + Sync {
    /// Unique identifier matching the corresponding detector's format_id.
    fn format_id(&self) -> &str;

    /// Human-readable name for the settings UI.
    fn display_name(&self) -> &str;

    /// Capabilities this renderer requires to function.
    fn capabilities(&self) -> Vec<RendererCapability>;

    /// Render a content block into styled output.
    fn render(
        &self,
        content: &ContentBlock,
        config: &RendererConfig,
    ) -> Result<RenderedContent, RenderError>;

    /// Short badge text for the gutter indicator (e.g., "MD", "JSON").
    fn format_badge(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::types::*;
    use std::time::SystemTime;

    /// Verify `ContentBlock` construction and helper methods.
    #[test]
    fn test_content_block_helpers() {
        let block = ContentBlock {
            lines: vec![
                "# Hello".to_string(),
                "world".to_string(),
                "end".to_string(),
            ],
            preceding_command: Some("echo test".to_string()),
            start_row: 0,
            end_row: 3,
            timestamp: SystemTime::now(),
        };

        assert_eq!(block.line_count(), 3);
        assert_eq!(block.first_lines(2), vec!["# Hello", "world"]);
        assert_eq!(block.last_lines(1), vec!["end"]);
        assert_eq!(block.full_text(), "# Hello\nworld\nend");
    }

    /// Verify `DetectionResult` construction.
    #[test]
    fn test_detection_result_construction() {
        let result = DetectionResult {
            format_id: "markdown".to_string(),
            confidence: 0.85,
            matched_rules: vec!["md_atx_header".to_string(), "md_bold".to_string()],
            source: DetectionSource::AutoDetected,
        };

        assert_eq!(result.format_id, "markdown");
        assert!((result.confidence - 0.85).abs() < f32::EPSILON);
        assert_eq!(result.matched_rules.len(), 2);
        assert_eq!(result.source, DetectionSource::AutoDetected);
    }

    /// Verify `RenderedContent` construction.
    #[test]
    fn test_rendered_content_construction() {
        let rendered = RenderedContent {
            lines: vec![StyledLine::plain("Hello, world!")],
            line_mapping: vec![SourceLineMapping {
                rendered_line: 0,
                source_line: Some(0),
            }],
            graphics: vec![],
            format_badge: "MD".to_string(),
        };

        assert_eq!(rendered.lines.len(), 1);
        assert_eq!(rendered.lines[0].segments.len(), 1);
        assert_eq!(rendered.lines[0].segments[0].text, "Hello, world!");
        assert_eq!(rendered.format_badge, "MD");
    }

    /// Verify trait objects can be created (object safety).
    #[test]
    fn test_trait_object_safety() {
        struct MockDetector;

        impl ContentDetector for MockDetector {
            fn format_id(&self) -> &str {
                "mock"
            }
            fn display_name(&self) -> &str {
                "Mock Format"
            }
            fn detect(&self, _content: &ContentBlock) -> Option<DetectionResult> {
                None
            }
            fn quick_match(&self, _first_lines: &[&str]) -> bool {
                false
            }
            fn detection_rules(&self) -> &[DetectionRule] {
                &[]
            }
        }

        struct MockRenderer;

        impl ContentRenderer for MockRenderer {
            fn format_id(&self) -> &str {
                "mock"
            }
            fn display_name(&self) -> &str {
                "Mock Renderer"
            }
            fn capabilities(&self) -> Vec<RendererCapability> {
                vec![RendererCapability::TextStyling]
            }
            fn render(
                &self,
                _content: &ContentBlock,
                _config: &RendererConfig,
            ) -> Result<RenderedContent, RenderError> {
                Err(RenderError::RenderFailed("not implemented".to_string()))
            }
            fn format_badge(&self) -> &str {
                "MOCK"
            }
        }

        // Verify these can be used as trait objects.
        let _detector: Box<dyn ContentDetector> = Box::new(MockDetector);
        let _renderer: Box<dyn ContentRenderer> = Box::new(MockRenderer);
    }

    /// Verify `RenderError` display messages.
    #[test]
    fn test_render_error_display() {
        let err = RenderError::RenderFailed("bad input".to_string());
        assert_eq!(err.to_string(), "render failed: bad input");

        let err = RenderError::CommandNotFound("mermaid-cli".to_string());
        assert_eq!(err.to_string(), "command not found: mermaid-cli");

        let err = RenderError::NetworkError("connection refused".to_string());
        assert_eq!(err.to_string(), "network error: connection refused");

        let err = RenderError::Timeout(5000);
        assert_eq!(err.to_string(), "render timed out after 5000ms");
    }
}

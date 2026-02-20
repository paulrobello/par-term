//! Core data types for the Content Prettifier framework.

use std::time::SystemTime;

/// A block of raw terminal output to be analyzed for content detection.
#[derive(Debug, Clone)]
pub struct ContentBlock {
    /// The raw text lines of the content block.
    pub lines: Vec<String>,
    /// The command that preceded this output (if known via shell integration).
    pub preceding_command: Option<String>,
    /// The starting row in the scrollback buffer.
    pub start_row: usize,
    /// The ending row (exclusive) in the scrollback buffer.
    pub end_row: usize,
    /// When this content block was captured.
    pub timestamp: SystemTime,
}

impl ContentBlock {
    /// Returns the number of lines in this block.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Returns the first N lines as string slices.
    pub fn first_lines(&self, n: usize) -> Vec<&str> {
        self.lines.iter().take(n).map(|s| s.as_str()).collect()
    }

    /// Returns the last N lines as string slices.
    pub fn last_lines(&self, n: usize) -> Vec<&str> {
        let skip = self.lines.len().saturating_sub(n);
        self.lines.iter().skip(skip).map(|s| s.as_str()).collect()
    }

    /// Returns the entire block joined as a single string.
    pub fn full_text(&self) -> String {
        self.lines.join("\n")
    }
}

/// Result of running content detection on a `ContentBlock`.
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// The format identifier (e.g., "markdown", "json", "mermaid").
    pub format_id: String,
    /// Confidence score from 0.0 to 1.0.
    pub confidence: f32,
    /// Which detection rules matched.
    pub matched_rules: Vec<String>,
    /// How this detection was triggered.
    pub source: DetectionSource,
}

/// How a content detection was triggered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectionSource {
    /// Detected automatically by the regex-based detection pipeline.
    AutoDetected,
    /// Invoked explicitly via a trigger rule or user action.
    TriggerInvoked,
}

/// A single regex rule contributing to format detection.
///
/// Note: `Clone` is not derived because `regex::Regex` does not implement `Clone`
/// cheaply. Use `DetectionRule::id` for identification instead.
#[derive(Debug)]
pub struct DetectionRule {
    /// Unique ID for this rule (for enable/disable and override).
    pub id: String,
    /// The compiled regex pattern.
    pub pattern: regex::Regex,
    /// How much confidence this rule contributes when matched (0.0–1.0).
    pub weight: f32,
    /// Where in the content to apply this pattern.
    pub scope: RuleScope,
    /// Whether this rule alone can trigger detection or needs corroboration.
    pub strength: RuleStrength,
    /// Whether this rule is built-in or user-defined.
    pub source: RuleSource,
    /// Optional: rule only applies after a matching command.
    pub command_context: Option<regex::Regex>,
    /// Human-readable description (shown in Settings UI).
    pub description: String,
    /// Whether this rule is enabled.
    pub enabled: bool,
}

/// Where in a content block a detection rule should be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleScope {
    /// Match against any line in the content block.
    AnyLine,
    /// Match only the first N lines (fast path for format headers).
    FirstLines(usize),
    /// Match only the last N lines (footers, closing brackets).
    LastLines(usize),
    /// Match against the entire content block as a single string (multi-line regex).
    FullBlock,
    /// Match against the preceding command that generated this output.
    PrecedingCommand,
}

/// How strong a signal a detection rule provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleStrength {
    /// A definitive signal — this pattern alone is sufficient to identify the format.
    Definitive,
    /// A strong signal — high confidence when matched, but benefits from corroboration.
    Strong,
    /// A supporting signal — only contributes when combined with other matches.
    Supporting,
}

/// Origin of a detection rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSource {
    /// Shipped with par-term, can be disabled but not deleted.
    BuiltIn,
    /// Added by the user via config, can be edited or removed.
    UserDefined,
}

/// Capabilities that a renderer may require.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererCapability {
    /// Text styling only (colors, bold, italic, underline).
    TextStyling,
    /// Inline graphics via Sixel/iTerm2/Kitty protocols.
    InlineGraphics,
    /// Requires running an external command (e.g., `mermaid-cli`).
    ExternalCommand,
    /// Requires network access (e.g., Kroki API).
    NetworkAccess,
}

/// The rendered output from a `ContentRenderer`.
#[derive(Debug, Clone)]
pub struct RenderedContent {
    /// The styled lines of rendered output.
    pub lines: Vec<StyledLine>,
    /// Mapping from rendered line indices back to source line indices.
    pub line_mapping: Vec<SourceLineMapping>,
    /// Inline graphics to display (e.g., rendered diagrams).
    pub graphics: Vec<InlineGraphic>,
    /// Short badge text indicating the detected format (e.g., "MD", "JSON").
    pub format_badge: String,
}

/// A single line of styled output.
#[derive(Debug, Clone)]
pub struct StyledLine {
    /// The styled segments making up this line.
    pub segments: Vec<StyledSegment>,
}

impl StyledLine {
    /// Creates a new styled line from segments.
    pub fn new(segments: Vec<StyledSegment>) -> Self {
        Self { segments }
    }

    /// Creates a plain unstyled line from text.
    pub fn plain(text: &str) -> Self {
        Self {
            segments: vec![StyledSegment {
                text: text.to_string(),
                fg: None,
                bg: None,
                bold: false,
                italic: false,
                underline: false,
                strikethrough: false,
            }],
        }
    }
}

/// A segment of styled text within a line.
#[derive(Debug, Clone)]
pub struct StyledSegment {
    /// The text content.
    pub text: String,
    /// Foreground color as [r, g, b].
    pub fg: Option<[u8; 3]>,
    /// Background color as [r, g, b].
    pub bg: Option<[u8; 3]>,
    /// Whether this segment is bold.
    pub bold: bool,
    /// Whether this segment is italic.
    pub italic: bool,
    /// Whether this segment is underlined.
    pub underline: bool,
    /// Whether this segment has strikethrough.
    pub strikethrough: bool,
}

/// Maps a rendered line index back to its source line index.
#[derive(Debug, Clone)]
pub struct SourceLineMapping {
    /// Index of the rendered line.
    pub rendered_line: usize,
    /// Index of the corresponding source line (if any).
    pub source_line: Option<usize>,
}

/// An inline graphic to display alongside rendered content.
#[derive(Debug, Clone)]
pub struct InlineGraphic {
    /// The image data (PNG bytes).
    pub data: Vec<u8>,
    /// Row position in the rendered output.
    pub row: usize,
    /// Column position in the rendered output.
    pub col: usize,
    /// Width in terminal cells.
    pub width_cells: usize,
    /// Height in terminal cells.
    pub height_cells: usize,
}

/// Toggle between rendered and source views of a prettified content block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// Show the prettified rendered output.
    Rendered,
    /// Show the original source text.
    Source,
}

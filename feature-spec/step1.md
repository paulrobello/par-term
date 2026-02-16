# Step 1: Core Framework Types & Traits

## Summary

Define the foundational types and traits for the Content Prettifier system. This step creates the `src/prettifier/` module with the core abstractions that all subsequent steps build upon: `ContentDetector`, `ContentRenderer`, `ContentBlock`, `DetectionResult`, `RenderedContent`, and all supporting enums.

## Dependencies

None — this is the first step.

## What to Implement

### New Module: `src/prettifier/mod.rs`

Public module re-exports and the prettifier module structure:

```rust
pub mod types;
pub mod traits;
```

### New File: `src/prettifier/types.rs`

All core data structures:

```rust
/// A block of terminal content to be analyzed for prettifying.
pub struct ContentBlock {
    /// The raw text lines of the content block
    pub lines: Vec<String>,
    /// The command that produced this output (from shell integration / OSC 133)
    pub preceding_command: Option<String>,
    /// Row range in the terminal scrollback buffer
    pub row_range: std::ops::Range<usize>,
    /// Timestamp when this content block was completed
    pub timestamp: std::time::Instant,
}

/// Result of running detection on a content block.
pub struct DetectionResult {
    /// Which format was detected (e.g., "markdown", "json")
    pub format_id: String,
    /// Confidence score 0.0 - 1.0
    pub confidence: f32,
    /// Which rules contributed to this detection
    pub matched_rules: Vec<String>,
    /// Whether this came from auto-detection or a trigger
    pub source: DetectionSource,
}

pub enum DetectionSource {
    /// Auto-detected via the regex-based confidence pipeline
    AutoDetected,
    /// Explicitly triggered via the trigger system (bypasses confidence scoring)
    TriggerInvoked,
}

/// A single regex rule contributing to format detection.
pub struct DetectionRule {
    pub id: String,
    pub pattern: regex::Regex,
    pub weight: f32,
    pub scope: RuleScope,
    pub strength: RuleStrength,
    pub source: RuleSource,
    pub command_context: Option<regex::Regex>,
    pub description: String,
    pub enabled: bool,
}

pub enum RuleScope {
    AnyLine,
    FirstLines(usize),
    LastLines(usize),
    FullBlock,
    PrecedingCommand,
}

pub enum RuleStrength {
    Definitive,
    Strong,
    Supporting,
}

pub enum RuleSource {
    BuiltIn,
    UserDefined,
}

/// What capabilities a renderer needs from the terminal.
pub enum RendererCapability {
    /// Text-only styling (colors, bold, italic, underline)
    TextStyling,
    /// Inline graphics (Sixel, iTerm2, Kitty)
    InlineGraphics,
    /// External command execution
    ExternalCommand,
    /// Network access (e.g., Kroki API)
    NetworkAccess,
}

/// The output of a ContentRenderer.
pub struct RenderedContent {
    /// Styled lines for display (with ANSI attributes or cell-level styling)
    pub styled_lines: Vec<StyledLine>,
    /// Mapping from rendered line index to source line index (for copy, selection)
    pub line_mapping: Vec<SourceLineMapping>,
    /// Optional inline graphics (for diagram renderers)
    pub graphics: Vec<InlineGraphic>,
    /// Format badge text for gutter display
    pub format_badge: String,
}

pub struct StyledLine {
    pub segments: Vec<StyledSegment>,
}

pub struct StyledSegment {
    pub text: String,
    pub fg: Option<[u8; 3]>,
    pub bg: Option<[u8; 3]>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub link_url: Option<String>,
}

pub struct SourceLineMapping {
    pub rendered_line: usize,
    pub source_line: Option<usize>,
}

pub struct InlineGraphic {
    pub row: usize,
    pub col: usize,
    pub width_cells: usize,
    pub height_cells: usize,
    pub image_data: Vec<u8>,  // RGBA pixel data
    pub image_width: u32,
    pub image_height: u32,
}

/// Current view state for a prettified block.
pub enum ViewMode {
    /// Show the rendered/prettified view
    Rendered,
    /// Show the original source text
    Source,
}
```

### New File: `src/prettifier/traits.rs`

The two core traits:

```rust
use super::types::*;

/// Identifies whether a content block matches a specific format.
pub trait ContentDetector: Send + Sync {
    fn format_id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn detect(&self, content: &ContentBlock) -> Option<DetectionResult>;
    fn quick_match(&self, first_lines: &[&str]) -> bool;
    fn detection_rules(&self) -> &[DetectionRule];
    fn accepts_custom_rules(&self) -> bool { true }
}

/// Renders a content block into styled terminal output.
pub trait ContentRenderer: Send + Sync {
    fn format_id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn capabilities(&self) -> Vec<RendererCapability>;

    /// Render the content block. May be async for external tools / network.
    fn render(&self, content: &ContentBlock, config: &RendererConfig) -> Result<RenderedContent, RenderError>;

    /// Whether this renderer can handle the given format
    fn supports_format(&self, format_id: &str) -> bool;
}

/// Configuration passed to renderers at render time.
pub struct RendererConfig {
    pub terminal_width: usize,
    pub theme_colors: ThemeColors,
    // Renderer-specific config will be added in later steps
}

pub struct ThemeColors {
    pub foreground: [u8; 3],
    pub background: [u8; 3],
    pub palette: [[u8; 3]; 16],
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("Rendering failed: {0}")]
    RenderFailed(String),
    #[error("External command not found: {0}")]
    CommandNotFound(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Timeout")]
    Timeout,
}
```

### Modify: `src/main.rs` or `src/lib.rs`

Add `mod prettifier;` to register the new module.

## Key Files

| Action | Path |
|--------|------|
| Create | `src/prettifier/mod.rs` |
| Create | `src/prettifier/types.rs` |
| Create | `src/prettifier/traits.rs` |
| Modify | `src/main.rs` (add `mod prettifier;`) |

## Relevant Spec Sections

- **Lines 64–203**: Trait definitions (`ContentDetector`, `ContentRenderer`), `DetectionRule`, `RuleScope`, `RuleStrength`, `RuleSource`, `RegexDetector`
- **Lines 51–63**: Core design principles (trait-based, detection separate from rendering, source preserved, etc.)
- **Lines 55–62**: Renderer capabilities, lazy/async, user-extensible

## Verification Criteria

- [ ] `cargo build` succeeds with the new module
- [ ] All types compile and are well-documented with doc comments
- [ ] `ContentDetector` and `ContentRenderer` traits are object-safe (can be used as `dyn ContentDetector`)
- [ ] `DetectionRule` fields match the spec (id, pattern, weight, scope, strength, source, command_context, description)
- [ ] `RuleScope` has all five variants: `AnyLine`, `FirstLines(usize)`, `LastLines(usize)`, `FullBlock`, `PrecedingCommand`
- [ ] `RuleStrength` has three variants: `Definitive`, `Strong`, `Supporting`
- [ ] `RenderedContent` includes `styled_lines`, `line_mapping`, `graphics`, and `format_badge`
- [ ] Unit tests for basic type construction and trait object creation

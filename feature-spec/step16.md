# Step 16: Diff Renderer

## Summary

Implement the diff/patch detector and renderer with green/red coloring for additions/deletions, line number gutter, file header styling, hunk header formatting, word-level diff highlighting within changed lines, and optional side-by-side mode.

## Dependencies

- **Step 1**: Core traits and types
- **Step 2**: `RegexDetector`
- **Step 4**: `RendererRegistry`

## What to Implement

### New File: `src/prettifier/detectors/diff.rs`

Diff detection rules from spec (lines 374–410):

```rust
pub fn create_diff_detector() -> RegexDetector {
    RegexDetector::builder("diff", "Diff")
        .confidence_threshold(0.6)
        .min_matching_rules(1)
        .definitive_shortcircuit(true)
        .add_rule(/* diff_git_header */)
        .add_rule(/* diff_unified_header */)
        .add_rule(/* diff_hunk */)
        .add_rule(/* diff_add_line */)
        .add_rule(/* diff_remove_line */)
        .add_rule(/* diff_git_context */)
        .build()
}
```

| Rule ID | Pattern | Weight | Scope | Strength |
|---------|---------|--------|-------|----------|
| `diff_git_header` | `^diff --git\s+` | 0.9 | FirstLines(5) | Definitive |
| `diff_unified_header` | `^---\s+\S+.*\n\+\+\+\s+\S+` | 0.9 | FirstLines(10) | Definitive |
| `diff_hunk` | `^@@\s+-\d+,?\d*\s+\+\d+,?\d*\s+@@` | 0.8 | AnyLine | Definitive |
| `diff_add_line` | `^\+[^+]` | 0.1 | AnyLine | Supporting |
| `diff_remove_line` | `^-[^-]` | 0.1 | AnyLine | Supporting |
| `diff_git_context` | `^git\s+(diff\|log\|show)` | 0.3 | PrecedingCommand | Supporting |

Note: `diff_unified_header` uses a multi-line pattern. This should be applied with `RuleScope::FullBlock` or the first 10 lines joined as a single string.

### New File: `src/prettifier/renderers/diff.rs`

```rust
pub struct DiffRenderer {
    config: DiffRendererConfig,
}

#[derive(Clone, Debug)]
pub struct DiffRendererConfig {
    pub style: DiffStyle,             // Inline | SideBySide | Auto
    pub side_by_side_min_width: usize, // Min cols for side-by-side (default: 160)
    pub word_diff: bool,              // Word-level highlighting (default: true)
    pub show_line_numbers: bool,      // Show line number gutter (default: true)
    pub context_lines: usize,         // Context lines around changes (default: 3)
}

pub enum DiffStyle {
    Inline,
    SideBySide,
    Auto, // Side-by-side if terminal wide enough, else inline
}

impl ContentRenderer for DiffRenderer {
    fn format_id(&self) -> &str { "diff" }
    fn display_name(&self) -> &str { "Diff" }
    fn capabilities(&self) -> Vec<RendererCapability> { vec![RendererCapability::TextStyling] }
    fn supports_format(&self, format_id: &str) -> bool { format_id == "diff" }

    fn render(&self, content: &ContentBlock, config: &RendererConfig) -> Result<RenderedContent, RenderError> { ... }
}
```

**Diff rendering features** (from spec lines 1119–1142):

#### Line-Level Coloring

- **Added lines** (`+`): Green foreground (or green background with slightly lighter shade)
- **Removed lines** (`-`): Red foreground (or red background with slightly lighter shade)
- **Context lines** (no prefix): Default foreground
- **File headers** (`---`, `+++`): Bold, distinct color
- **Hunk headers** (`@@`): Cyan/blue, with line range info

```rust
fn style_diff_line(line: &str, theme: &ThemeColors) -> StyledLine {
    if line.starts_with("diff --git") {
        // Bold file header
        bold_line(line, theme.diff_header_color)
    } else if line.starts_with("---") || line.starts_with("+++") {
        bold_line(line, theme.diff_file_color)
    } else if line.starts_with("@@") {
        styled_line(line, theme.diff_hunk_color)
    } else if line.starts_with('+') {
        styled_line(line, theme.diff_add_color)
    } else if line.starts_with('-') {
        styled_line(line, theme.diff_remove_color)
    } else {
        plain_line(line)
    }
}
```

#### Word-Level Diff Highlighting

When `word_diff` is enabled, highlight the specific words/characters that changed within a line pair (added + removed), not just the whole line:

```rust
/// Compute word-level diff between two lines and apply inline highlighting.
fn word_diff_highlight(
    removed_line: &str,
    added_line: &str,
    theme: &ThemeColors,
) -> (StyledLine, StyledLine) {
    // Use a simple word-diff algorithm (split by whitespace + punctuation)
    // Highlight changed words with a stronger background color
    // Keep unchanged words with the line's base color
    ...
}
```

Algorithm:
1. Split both lines into words
2. Run a diff algorithm (e.g., LCS or patience diff) on the word sequences
3. Mark changed words with a stronger background highlight
4. Unchanged words keep the line-level color

#### Line Number Gutter

When `show_line_numbers` is enabled:
- Show source file line numbers for `-` lines (left gutter)
- Show target file line numbers for `+` lines (right gutter)
- Parse line numbers from hunk headers (`@@ -old_start,old_count +new_start,new_count @@`)

```rust
struct DiffLineState {
    old_line: usize,
    new_line: usize,
}

fn render_line_number_gutter(
    line_type: DiffLineType,
    state: &mut DiffLineState,
    gutter_width: usize,
) -> StyledSegment { ... }
```

#### Side-by-Side Mode

When terminal is wide enough (`>= side_by_side_min_width`):
- Split display into left (removed) and right (added) columns
- Context lines appear on both sides
- Each side has its own line number gutter
- Horizontal divider between the two columns

```rust
fn render_side_by_side(
    hunks: &[DiffHunk],
    terminal_width: usize,
    config: &DiffRendererConfig,
    theme: &ThemeColors,
) -> Vec<StyledLine> { ... }
```

#### Diff Parsing

Parse unified diff format into structured hunks:

```rust
struct DiffFile {
    old_path: String,
    new_path: String,
    hunks: Vec<DiffHunk>,
}

struct DiffHunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    header_text: String,  // Function name hint in @@ header
    lines: Vec<DiffLine>,
}

enum DiffLine {
    Context(String),
    Added(String),
    Removed(String),
}

fn parse_unified_diff(content: &[String]) -> Vec<DiffFile> { ... }
```

## Key Files

| Action | Path |
|--------|------|
| Create | `src/prettifier/detectors/diff.rs` |
| Create | `src/prettifier/renderers/diff.rs` |
| Modify | `src/prettifier/detectors/mod.rs` (add `pub mod diff;`) |
| Modify | `src/prettifier/renderers/mod.rs` (add `pub mod diff;`) |

## Relevant Spec Sections

- **Lines 374–410**: Diff detection rules (6 rules)
- **Lines 1119–1142**: Diff renderer features and config
- **Lines 1340**: Phase 2b — unified diff detection and coloring with word-level highlighting
- **Lines 1364**: Shared diff coloring infrastructure (line-level + word-level)
- **Lines 1472**: Acceptance criteria — unified diff detected with green/red coloring and word-level highlighting

## Verification Criteria

- [ ] `cargo build` succeeds
- [ ] `diff --git` header triggers definitive detection
- [ ] `@@` hunk headers trigger definitive detection
- [ ] Added lines (`+`) render in green
- [ ] Removed lines (`-`) render in red
- [ ] File headers (`---`, `+++`) render bold with distinct color
- [ ] Hunk headers (`@@`) render in cyan/blue with range info
- [ ] Word-level diff highlighting works for changed words within line pairs
- [ ] Line number gutter shows old/new line numbers correctly
- [ ] Side-by-side mode activates when terminal is wide enough
- [ ] Side-by-side mode falls back to inline when terminal is narrow
- [ ] `Auto` style selects based on terminal width
- [ ] Diff parsing handles multiple files and hunks correctly
- [ ] Context lines render with default foreground
- [ ] Unit tests for parsing, line coloring, word diff, side-by-side layout

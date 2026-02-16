# Step 9: Markdown Renderer — Tables & Code Blocks

## Summary

Extend the markdown renderer to handle tables and fenced code blocks — elements that may change line counts relative to the source. Tables are rendered with Unicode box-drawing characters and proper column alignment. Code blocks get background shading and syntax highlighting with a language label.

## Dependencies

- **Step 8**: `MarkdownRenderer` with inline element support
- **Step 1**: `StyledLine`, `StyledSegment`, `SourceLineMapping`

## What to Implement

### Extend: `src/prettifier/renderers/markdown.rs`

#### Fenced Code Blocks

Parse fenced code blocks (` ``` ` or `~~~` delimiters with optional language tag):

1. **Detection**: Track state — when encountering ` ```language `, enter code block mode until the closing ` ``` `
2. **Language extraction**: Parse the language tag from the opening fence (e.g., `rust`, `python`, `json`)
3. **Rendering**:
   - Add a subtle background color to all lines within the code block
   - Display the language label in the gutter or top-right corner of the block
   - Apply syntax highlighting if a highlighter is available for the language
   - Preserve exact whitespace/indentation within the code block
   - Strip the fence markers (` ``` `) from rendered output
4. **Line mapping**: Code block lines map 1:1 to source lines, but fence lines (` ``` `) map to no rendered line (or a decorative border line)

**Syntax highlighting approach**:
- Start with a simple keyword-based highlighter for common languages (Rust, Python, JavaScript, JSON, YAML, Shell)
- Map language keywords, strings, comments, and numbers to distinct colors from the theme
- This can be enhanced later with tree-sitter grammars (noted in spec line 1352)

```rust
/// Simple keyword-based syntax highlighter.
struct SyntaxHighlighter {
    language: String,
    keywords: Vec<String>,
    comment_patterns: Vec<String>,
    string_delimiters: Vec<char>,
}

impl SyntaxHighlighter {
    fn highlight_line(&self, line: &str, theme: &ThemeColors) -> StyledLine { ... }
}

/// Registry of built-in syntax highlighters.
fn get_highlighter(language: &str) -> Option<SyntaxHighlighter> { ... }
```

#### Tables

Parse Markdown tables with pipe-delimited columns:

1. **Detection**: Track state — a table starts with a row of `|...|...|` followed by a separator row `|---|---|`
2. **Column analysis**:
   - Parse all rows to determine column count and max width per column
   - Handle alignment indicators in separator row (`:---`, `:---:`, `---:`)
   - Auto-size columns based on content width (up to terminal width)
3. **Rendering with box-drawing characters**:
   - Use Unicode box-drawing for borders: `┌─┬─┐`, `│ │ │`, `├─┼─┤`, `└─┴─┘`
   - Or rounded variant: `╭─┬─╮`, `│ │ │`, `├─┼─┤`, `╰─┴─╯`
   - Or ASCII: `+---+---+`, `| | |`, `+---+---+`
   - Style based on `table_style` config (`unicode`, `ascii`, `rounded`)
4. **Header row styling**: Bold or colored header row (first row before separator)
5. **Column alignment**: Left-align text, right-align numbers (based on separator row indicators)
6. **Border color**: Use `table_border_color` config (e.g., "dim" = dimmed color)

```rust
/// Shared table rendering infrastructure (also used by CSV, SQL results in later steps).
pub struct TableRenderer {
    style: TableStyle,
    border_color: [u8; 3],
}

pub enum TableStyle {
    Unicode,
    Ascii,
    Rounded,
}

impl TableRenderer {
    /// Render a table from rows of cells.
    pub fn render_table(
        &self,
        headers: &[String],
        rows: &[Vec<String>],
        alignments: &[ColumnAlignment],
        max_width: usize,
    ) -> Vec<StyledLine> { ... }
}

pub enum ColumnAlignment {
    Left,
    Center,
    Right,
}
```

**Important**: The `TableRenderer` is shared infrastructure that will be reused by CSV (Step 18), SQL results (Step 18), and potentially JSON tabular views. Design it as a standalone utility.

#### Updated Rendering Flow

The `render()` method now needs multi-pass processing:

1. **First pass**: Identify block-level elements (paragraphs, headers, code blocks, tables, lists, blockquotes, HRs)
2. **Code block accumulation**: Gather all lines between fences into code block elements
3. **Table accumulation**: Gather contiguous pipe-delimited rows into table elements
4. **Second pass**: Render each block element:
   - Code blocks → syntax-highlighted lines with background
   - Tables → box-drawing formatted lines
   - Other lines → inline element rendering (from Step 8)
5. **Build line mapping**: Since tables and code blocks may produce different line counts than source (e.g., adding border rows), maintain accurate `SourceLineMapping`

### Markdown Config Extensions

Add code block and table config options (from spec lines 941–956):

```rust
pub struct MarkdownRendererConfig {
    // ... existing from Step 8 ...
    pub code_block_theme: String,      // Syntax highlighting theme
    pub code_block_background: bool,   // Show background shading
    pub table_style: TableStyle,       // Unicode | Ascii | Rounded
    pub table_border_color: String,    // "dim" or color name
}
```

## Key Files

| Action | Path |
|--------|------|
| Modify | `src/prettifier/renderers/markdown.rs` (add table + code block rendering) |
| Create | `src/prettifier/renderers/table.rs` (shared table renderer) |
| Modify | `src/prettifier/renderers/mod.rs` (add `pub mod table;`) |

## Relevant Spec Sections

- **Lines 930–932**: Code blocks — background shading, syntax highlighting, language label
- **Lines 933**: Tables — Unicode box-drawing, column alignment, padding
- **Lines 941–956**: Markdown-specific config (code_block_theme, table_style, etc.)
- **Lines 1326**: Box drawing powers tables in Markdown, JSON, CSV, SQL results
- **Lines 1336**: Phase 1b — tables, fenced code blocks with syntax highlighting (line count may change)
- **Lines 1356–1366**: Shared infrastructure — table renderer and syntax highlighter used by multiple renderers

## Verification Criteria

- [ ] `cargo build` succeeds
- [ ] Fenced code blocks render with fence markers stripped
- [ ] Code block language tag is extracted and displayed
- [ ] Code blocks have background shading when enabled
- [ ] Basic syntax highlighting works for at least Rust, Python, and JSON
- [ ] Tables render with proper box-drawing characters
- [ ] Column widths are auto-sized based on content
- [ ] Column alignment respects separator row indicators (`:---`, `:---:`, `---:`)
- [ ] Header row is visually distinct (bold or colored)
- [ ] Table border color respects config
- [ ] `TableRenderer` is usable standalone (for CSV/SQL in later steps)
- [ ] Line mapping correctly handles line count differences (added border rows, stripped fences)
- [ ] Nested inline elements within table cells render correctly
- [ ] Nested inline elements within blockquotes and list items render correctly
- [ ] Unit tests for code block rendering, table rendering, and line mapping

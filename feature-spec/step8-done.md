# Step 8: Markdown Renderer — Inline Elements

## Summary

Implement the first phase of the markdown renderer, handling inline elements that do not change line counts: headers (H1-H6), bold, italic, inline code, links (as OSC 8 hyperlinks), blockquotes, lists (bullet and ordered), and horizontal rules. These are "text-attribute-only" changes — they restyle existing lines without adding or removing lines.

## Dependencies

- **Step 1**: `ContentRenderer` trait, `RenderedContent`, `StyledLine`, `StyledSegment`
- **Step 4**: `RendererRegistry` (to register the markdown renderer)
- **Step 7**: Markdown detection (detector must exist to trigger the renderer)

## What to Implement

### New Directory: `src/prettifier/renderers/`

```
src/prettifier/renderers/
├── mod.rs          # pub mod markdown;
└── markdown.rs     # MarkdownRenderer
```

### New File: `src/prettifier/renderers/markdown.rs`

```rust
/// Renders Markdown content into styled terminal output.
pub struct MarkdownRenderer {
    config: MarkdownRendererConfig,
}

#[derive(Clone, Debug)]
pub struct MarkdownRendererConfig {
    pub render_mode: RenderMode,       // Pretty | Source | Hybrid
    pub header_style: HeaderStyle,     // Colored | Bold | Underlined
    pub link_style: LinkStyle,         // UnderlineColor | InlineUrl | Footnote
    pub horizontal_rule_style: HRStyle, // Thin | Thick | Dashed
}

impl ContentRenderer for MarkdownRenderer {
    fn format_id(&self) -> &str { "markdown" }
    fn display_name(&self) -> &str { "Markdown" }
    fn capabilities(&self) -> Vec<RendererCapability> { vec![RendererCapability::TextStyling] }
    fn supports_format(&self, format_id: &str) -> bool { format_id == "markdown" }

    fn render(&self, content: &ContentBlock, config: &RendererConfig) -> Result<RenderedContent, RenderError> {
        let mut styled_lines = Vec::new();
        let mut line_mapping = Vec::new();

        for (i, line) in content.lines.iter().enumerate() {
            let styled = self.render_line(line, config);
            line_mapping.push(SourceLineMapping {
                rendered_line: styled_lines.len(),
                source_line: Some(i),
            });
            styled_lines.push(styled);
        }

        Ok(RenderedContent {
            styled_lines,
            line_mapping,
            graphics: vec![],
            format_badge: "\u{1F4DD}".to_string(), // 📝
        })
    }
}
```

**Inline element rendering rules:**

#### Headers (H1-H6)
- Parse lines matching `^#{1,6}\s+(.*)$`
- Style based on `header_style` config:
  - **Colored**: Each level gets a distinct color from the theme palette (H1=bright, H6=dim)
  - **Bold**: All headers bold, with decreasing brightness per level
  - **Underlined**: H1/H2 underlined, rest bold
- Visual weight: H1 largest visual impact, H6 smallest
- Strip the `#` prefix in rendered output

#### Bold / Italic
- Parse `**text**` → bold segment, `*text*` → italic segment, `***text***` → bold+italic
- Handle `__text__` and `_text_` variants
- Leverage existing styled font variant support (the terminal already supports bold/italic rendering)

#### Inline Code
- Parse `` `code` `` → segment with subtle background highlight
- Use a slightly different background color from the terminal default

#### Links
- Parse `[text](url)` → styled text with OSC 8 hyperlink
- Set `link_url` on the `StyledSegment`
- Style based on `link_style`:
  - **UnderlineColor**: Underline + link color from theme
  - **InlineUrl**: Show `text (url)`
  - **Footnote**: Show `text[1]` with footnotes at end

#### Blockquotes
- Parse lines starting with `> `
- Render with a left border character (`▎` or `│`) in a distinct color
- Dim or colorize the quoted text

#### Lists
- Parse `- item`, `* item`, `+ item` (unordered) and `1. item`, `2) item` (ordered)
- Render with proper indentation
- Style bullets/numbers distinctly from content text
- Handle nested lists (indentation-based)

#### Horizontal Rules
- Parse lines matching `^([-*_])\s*\1\s*\1[\s\1]*$` (three or more `-`, `*`, or `_`)
- Render as a full-width line using the `horizontal_rule_style`:
  - **Thin**: `─` repeated
  - **Thick**: `━` repeated
  - **Dashed**: `╌` repeated

### Registration

```rust
pub fn register_markdown_renderer(registry: &mut RendererRegistry, config: &MarkdownRendererConfig) {
    registry.register_renderer("markdown", Box::new(MarkdownRenderer { config: config.clone() }));
}
```

### Markdown Parsing Strategy

Use a line-by-line parser rather than a full AST parser for this step:
1. Classify each line as a block-level element (header, blockquote, list, HR, or paragraph)
2. Within paragraph/list lines, parse inline elements (bold, italic, code, links)
3. Produce `StyledLine` with appropriately styled segments

This approach is simpler and sufficient for Phase 1. A full AST parser (e.g., `pulldown-cmark`) can be introduced later if needed for complex nesting.

## Key Files

| Action | Path |
|--------|------|
| Create | `src/prettifier/renderers/mod.rs` |
| Create | `src/prettifier/renderers/markdown.rs` |
| Modify | `src/prettifier/mod.rs` (add `pub mod renderers;`) |

## Relevant Spec Sections

- **Lines 929–956**: Markdown rendered elements — headers, code blocks, inline code, tables, bold/italic, lists, blockquotes, HR, links, images
- **Lines 941–956**: Markdown-specific config options
- **Lines 1325**: Styled font variants (bold, italic, bold-italic) power emphasis rendering
- **Lines 1326**: Box drawing powers tables (deferred to Step 9)
- **Lines 1327**: Command separator infrastructure powers horizontal rules
- **Lines 1335**: Phase 1a — headers, bold/italic, inline code, HR, lists, blockquotes, links (text-attribute-only)

## Verification Criteria

- [ ] `cargo build` succeeds
- [ ] `MarkdownRenderer` implements `ContentRenderer` trait
- [ ] Headers H1-H6 are rendered with visual hierarchy (color/bold differentiation)
- [ ] `**bold**` text renders as bold segments
- [ ] `*italic*` text renders as italic segments
- [ ] `` `inline code` `` renders with background highlight
- [ ] `[text](url)` renders as OSC 8 hyperlink with underline
- [ ] Blockquotes render with left border and dimmed text
- [ ] Bullet lists render with proper indentation and styled bullets
- [ ] Ordered lists render with numbers
- [ ] Horizontal rules render as full-width lines
- [ ] Line mapping correctly maps each rendered line to its source line
- [ ] `format_badge` is set to the markdown emoji
- [ ] All rendering respects theme colors (not hardcoded)
- [ ] Unit tests for each inline element type

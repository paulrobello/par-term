//! Markdown renderer — inline elements, fenced code blocks, and tables.
//!
//! Renders markdown content into styled terminal output using a two-pass parser.
//! **Pass 1** classifies source lines into block-level elements (paragraphs,
//! headers, code blocks, tables, lists, blockquotes, horizontal rules).
//! **Pass 2** renders each block, applying inline formatting within paragraphs
//! and using dedicated renderers for code blocks (with syntax highlighting) and
//! tables (via the shared `TableRenderer`).

use regex::Regex;
use std::sync::OnceLock;

use super::diagrams::DiagramRenderer;
use super::table::{ColumnAlignment, TableRenderer, TableStyle};
use crate::config::prettifier::DiagramRendererConfig;
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::traits::{ContentRenderer, RendererConfig, ThemeColors};
use crate::prettifier::types::{
    ContentBlock, InlineGraphic, RenderedContent, RendererCapability, SourceLineMapping,
    StyledLine, StyledSegment,
};

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// How to style header elements.
#[derive(Clone, Debug, Default)]
pub enum HeaderStyle {
    /// Each level gets a distinct color from the theme palette.
    #[default]
    Colored,
    /// All headers bold, with decreasing brightness per level.
    Bold,
    /// H1/H2 underlined, rest bold.
    Underlined,
}

/// How to render links.
#[derive(Clone, Debug, Default)]
pub enum LinkStyle {
    /// Underline + link color with OSC 8 hyperlink.
    #[default]
    UnderlineColor,
    /// Show `text (url)` inline.
    InlineUrl,
    /// Show `text[1]` with footnotes collected at end (not yet implemented).
    Footnote,
}

/// How to render horizontal rules.
#[derive(Clone, Debug, Default)]
pub enum HorizontalRuleStyle {
    /// `─` repeated.
    #[default]
    Thin,
    /// `━` repeated.
    Thick,
    /// `╌` repeated.
    Dashed,
}

/// Configuration for the `MarkdownRenderer`.
#[derive(Clone, Debug)]
pub struct MarkdownRendererConfig {
    pub header_style: HeaderStyle,
    pub link_style: LinkStyle,
    pub horizontal_rule_style: HorizontalRuleStyle,
    /// Show background shading on fenced code blocks.
    pub code_block_background: bool,
    /// Table border style.
    pub table_style: TableStyle,
    /// Table border color as [r, g, b]. Use dim grey by default.
    pub table_border_color: [u8; 3],
}

impl Default for MarkdownRendererConfig {
    fn default() -> Self {
        Self {
            header_style: HeaderStyle::Colored,
            link_style: LinkStyle::UnderlineColor,
            horizontal_rule_style: HorizontalRuleStyle::Thin,
            code_block_background: true,
            table_style: TableStyle::Unicode,
            table_border_color: [108, 112, 134],
        }
    }
}

// ---------------------------------------------------------------------------
// Compiled regexes
// ---------------------------------------------------------------------------

fn re_header() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(#{1,6})\s+(.*)$").unwrap())
}

fn re_blockquote() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^>\s?(.*)$").unwrap())
}

fn re_unordered_list() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\s*)([-*+])\s+(.*)$").unwrap())
}

fn re_ordered_list() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\s*)(\d+[.)])\s+(.*)$").unwrap())
}

fn re_horizontal_rule() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(?:-[\s-]*-[\s-]*-[\s-]*|\*[\s*]*\*[\s*]*\*[\s*]*|_[\s_]*_[\s_]*_[\s_]*)$")
            .unwrap()
    })
}

fn re_inline_code() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"`([^`]+)`").unwrap())
}

fn re_link() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap())
}

fn re_bold_italic() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\*\*\*(.+?)\*\*\*|___(.+?)___").unwrap())
}

fn re_bold() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\*\*(.+?)\*\*|__(.+?)__").unwrap())
}

fn re_italic() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Use \b around underscore italic to avoid matching snake_case identifiers.
    RE.get_or_init(|| Regex::new(r"\*([^*]+)\*|\b_([^_]+)_\b").unwrap())
}

fn re_fence_open() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\s*)(```|~~~)(\w*)\s*$").unwrap())
}

// ---------------------------------------------------------------------------
// Inline span extraction
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct InlineSpan {
    start: usize,
    end: usize,
    kind: SpanKind,
}

#[derive(Debug, Clone)]
enum SpanKind {
    Code(String),
    Link { text: String, url: String },
    BoldItalic(String),
    Bold(String),
    Italic(String),
}

fn any_occupied(occupied: &[bool], start: usize, end: usize) -> bool {
    occupied[start..end].iter().any(|&b| b)
}

fn mark_occupied(occupied: &mut [bool], start: usize, end: usize) {
    for b in &mut occupied[start..end] {
        *b = true;
    }
}

fn find_in_unoccupied(text: &str, re: &Regex, occupied: &[bool]) -> Vec<(usize, usize)> {
    let mut results = Vec::new();
    let mut pos = 0;
    while pos < text.len() {
        if occupied[pos] {
            pos += 1;
            continue;
        }
        if let Some(m) = re.find_at(text, pos) {
            if !any_occupied(occupied, m.start(), m.end()) {
                results.push((m.start(), m.end()));
                pos = m.end();
            } else {
                pos = m.start() + 1;
            }
        } else {
            break;
        }
    }
    results
}

fn extract_inline_spans(text: &str) -> Vec<InlineSpan> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut occupied = vec![false; text.len()];
    let mut spans = Vec::new();

    // Pass 1: Code spans (highest priority, opaque)
    for (start, end) in find_in_unoccupied(text, re_inline_code(), &occupied) {
        let caps = re_inline_code().captures(&text[start..]).unwrap();
        let content = caps.get(1).unwrap().as_str().to_string();
        mark_occupied(&mut occupied, start, end);
        spans.push(InlineSpan {
            start,
            end,
            kind: SpanKind::Code(content),
        });
    }

    // Pass 2: Links
    for (start, end) in find_in_unoccupied(text, re_link(), &occupied) {
        let caps = re_link().captures(&text[start..]).unwrap();
        let link_text = caps.get(1).unwrap().as_str().to_string();
        let url = caps.get(2).unwrap().as_str().to_string();
        mark_occupied(&mut occupied, start, end);
        spans.push(InlineSpan {
            start,
            end,
            kind: SpanKind::Link {
                text: link_text,
                url,
            },
        });
    }

    // Pass 3: Bold+italic
    for (start, end) in find_in_unoccupied(text, re_bold_italic(), &occupied) {
        let caps = re_bold_italic().captures(&text[start..]).unwrap();
        let content = caps
            .get(1)
            .or_else(|| caps.get(2))
            .unwrap()
            .as_str()
            .to_string();
        mark_occupied(&mut occupied, start, end);
        spans.push(InlineSpan {
            start,
            end,
            kind: SpanKind::BoldItalic(content),
        });
    }

    // Pass 4: Bold
    for (start, end) in find_in_unoccupied(text, re_bold(), &occupied) {
        let caps = re_bold().captures(&text[start..]).unwrap();
        let content = caps
            .get(1)
            .or_else(|| caps.get(2))
            .unwrap()
            .as_str()
            .to_string();
        mark_occupied(&mut occupied, start, end);
        spans.push(InlineSpan {
            start,
            end,
            kind: SpanKind::Bold(content),
        });
    }

    // Pass 5: Italic
    for (start, end) in find_in_unoccupied(text, re_italic(), &occupied) {
        let caps = re_italic().captures(&text[start..]).unwrap();
        let content = caps
            .get(1)
            .or_else(|| caps.get(2))
            .unwrap()
            .as_str()
            .to_string();
        mark_occupied(&mut occupied, start, end);
        spans.push(InlineSpan {
            start,
            end,
            kind: SpanKind::Italic(content),
        });
    }

    spans.sort_by_key(|s| s.start);
    spans
}

// ---------------------------------------------------------------------------
// Block-level element classification
// ---------------------------------------------------------------------------

/// A block-level element identified during the first pass.
enum BlockElement {
    /// A single line rendered with inline formatting.
    Line { source_idx: usize },
    /// A fenced code block (``` or ~~~).
    CodeBlock {
        language: Option<String>,
        /// The content lines (without fence markers).
        lines: Vec<String>,
        /// Source line index of the opening fence.
        fence_open_idx: usize,
        /// Source line index of the closing fence (if present).
        fence_close_idx: Option<usize>,
    },
    /// A pipe-delimited markdown table.
    Table {
        headers: Vec<String>,
        alignments: Vec<ColumnAlignment>,
        rows: Vec<Vec<String>>,
        /// Source line range [start, end) covering header + separator + data rows.
        source_start: usize,
        source_end: usize,
    },
}

/// Check if a line is a markdown table row (has pipe separators).
fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('|') && !trimmed.is_empty()
}

/// Check if a line is a table separator row (e.g., `|---|:---:|---:|`).
fn is_separator_row(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.contains('-') {
        return false;
    }
    let cells = parse_table_cells(trimmed);
    if cells.is_empty() {
        return false;
    }
    cells.iter().all(|cell| {
        let c = cell.trim();
        !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')
    })
}

/// Parse a pipe-delimited table row into cells.
fn parse_table_cells(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    // Strip leading/trailing pipes.
    let inner = trimmed
        .strip_prefix('|')
        .unwrap_or(trimmed)
        .strip_suffix('|')
        .unwrap_or(trimmed);
    inner.split('|').map(|s| s.trim().to_string()).collect()
}

/// Parse alignment from a separator cell (e.g., `:---:` → Center).
fn parse_alignment(cell: &str) -> ColumnAlignment {
    let c = cell.trim();
    let starts_colon = c.starts_with(':');
    let ends_colon = c.ends_with(':');
    match (starts_colon, ends_colon) {
        (true, true) => ColumnAlignment::Center,
        (false, true) => ColumnAlignment::Right,
        _ => ColumnAlignment::Left,
    }
}

/// First pass: classify source lines into block-level elements.
fn classify_blocks(lines: &[String]) -> Vec<BlockElement> {
    let mut blocks = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        // Check for fenced code block opening.
        if let Some(caps) = re_fence_open().captures(&lines[i]) {
            let delimiter = caps.get(2).unwrap().as_str().to_string();
            let lang_str = caps.get(3).unwrap().as_str().to_string();
            let language = if lang_str.is_empty() {
                None
            } else {
                Some(lang_str)
            };

            let fence_open_idx = i;
            let mut code_lines = Vec::new();
            i += 1;

            // Accumulate until closing fence or end of input.
            let mut fence_close_idx = None;
            while i < lines.len() {
                let trimmed = lines[i].trim();
                if trimmed == delimiter || trimmed == format!("{delimiter} ").trim_end() {
                    fence_close_idx = Some(i);
                    i += 1;
                    break;
                }
                code_lines.push(lines[i].clone());
                i += 1;
            }

            blocks.push(BlockElement::CodeBlock {
                language,
                lines: code_lines,
                fence_open_idx,
                fence_close_idx,
            });
            continue;
        }

        // Check for table: current line is a table row and next line is a separator.
        if i + 1 < lines.len() && is_table_row(&lines[i]) && is_separator_row(&lines[i + 1]) {
            let header_cells = parse_table_cells(&lines[i]);
            let sep_cells = parse_table_cells(&lines[i + 1]);
            let alignments: Vec<ColumnAlignment> =
                sep_cells.iter().map(|c| parse_alignment(c)).collect();

            let source_start = i;
            i += 2; // Skip header + separator.

            let mut data_rows = Vec::new();
            while i < lines.len() && is_table_row(&lines[i]) && !is_separator_row(&lines[i]) {
                data_rows.push(parse_table_cells(&lines[i]));
                i += 1;
            }

            blocks.push(BlockElement::Table {
                headers: header_cells,
                alignments,
                rows: data_rows,
                source_start,
                source_end: i,
            });
            continue;
        }

        // Regular line.
        blocks.push(BlockElement::Line { source_idx: i });
        i += 1;
    }

    blocks
}

// ---------------------------------------------------------------------------
// Simple keyword-based syntax highlighting
// ---------------------------------------------------------------------------

/// Language definition for syntax highlighting.
struct LanguageDef {
    keywords: &'static [&'static str],
    comment_prefix: &'static str,
    builtins: &'static [&'static str],
}

fn get_language_def(language: &str) -> Option<LanguageDef> {
    match language.to_lowercase().as_str() {
        "rust" | "rs" => Some(LanguageDef {
            keywords: &[
                "fn", "let", "mut", "const", "static", "if", "else", "match", "for", "while",
                "loop", "return", "break", "continue", "struct", "enum", "impl", "trait", "pub",
                "use", "mod", "crate", "self", "super", "where", "async", "await", "move",
                "unsafe", "type", "as", "in", "ref", "true", "false",
            ],
            comment_prefix: "//",
            builtins: &[
                "Self", "Option", "Result", "Vec", "String", "Box", "Rc", "Arc", "Some", "None",
                "Ok", "Err",
            ],
        }),
        "python" | "py" => Some(LanguageDef {
            keywords: &[
                "def", "class", "if", "elif", "else", "for", "while", "return", "import", "from",
                "as", "try", "except", "finally", "with", "yield", "lambda", "pass", "break",
                "continue", "raise", "and", "or", "not", "in", "is", "True", "False", "None",
                "async", "await",
            ],
            comment_prefix: "#",
            builtins: &[
                "print",
                "len",
                "range",
                "int",
                "str",
                "float",
                "list",
                "dict",
                "set",
                "tuple",
                "bool",
                "type",
                "isinstance",
                "self",
            ],
        }),
        "javascript" | "js" | "typescript" | "ts" => Some(LanguageDef {
            keywords: &[
                "function",
                "const",
                "let",
                "var",
                "if",
                "else",
                "for",
                "while",
                "return",
                "class",
                "new",
                "this",
                "import",
                "export",
                "from",
                "default",
                "try",
                "catch",
                "finally",
                "throw",
                "async",
                "await",
                "yield",
                "switch",
                "case",
                "break",
                "continue",
                "typeof",
                "instanceof",
                "true",
                "false",
                "null",
                "undefined",
            ],
            comment_prefix: "//",
            builtins: &[
                "console", "Promise", "Array", "Object", "Map", "Set", "JSON", "Math", "String",
                "Number", "Boolean", "Error",
            ],
        }),
        "json" => Some(LanguageDef {
            keywords: &["true", "false", "null"],
            comment_prefix: "",
            builtins: &[],
        }),
        "yaml" | "yml" => Some(LanguageDef {
            keywords: &["true", "false", "null", "yes", "no"],
            comment_prefix: "#",
            builtins: &[],
        }),
        "shell" | "sh" | "bash" | "zsh" => Some(LanguageDef {
            keywords: &[
                "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac",
                "function", "return", "exit", "export", "local", "readonly", "in", "select",
                "until", "true", "false",
            ],
            comment_prefix: "#",
            builtins: &[
                "echo", "cd", "ls", "cat", "grep", "sed", "awk", "find", "sort", "uniq", "wc",
                "head", "tail", "mkdir", "rm", "cp", "mv", "chmod", "chown", "curl", "wget",
            ],
        }),
        _ => None,
    }
}

/// Highlight a single code line using simple keyword matching.
///
/// Returns styled segments with colors for keywords, strings, comments,
/// numbers, and builtins.
fn highlight_code_line(
    line: &str,
    lang_def: Option<&LanguageDef>,
    theme: &ThemeColors,
    show_bg: bool,
) -> StyledLine {
    let code_bg = if show_bg {
        Some(subtle_bg(theme))
    } else {
        None
    };

    let Some(def) = lang_def else {
        // No language definition — plain text with optional background.
        return StyledLine::new(vec![StyledSegment {
            text: line.to_string(),
            bg: code_bg,
            ..Default::default()
        }]);
    };

    // Check for full-line comment.
    if !def.comment_prefix.is_empty() && line.trim_start().starts_with(def.comment_prefix) {
        return StyledLine::new(vec![StyledSegment {
            text: line.to_string(),
            fg: Some(theme.palette[8]), // dim grey
            bg: code_bg,
            italic: true,
            ..Default::default()
        }]);
    }

    // Tokenize the line into segments.
    let mut segments = Vec::new();
    let mut chars = line.char_indices().peekable();

    while let Some(&(byte_pos, ch)) = chars.peek() {
        // String literal.
        if ch == '"' || ch == '\'' {
            let quote = ch;
            let start = byte_pos;
            chars.next(); // consume opening quote
            let mut escaped = false;
            while let Some(&(_, c)) = chars.peek() {
                chars.next();
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == quote {
                    break;
                }
            }
            let end = chars.peek().map(|&(i, _)| i).unwrap_or(line.len());
            segments.push(StyledSegment {
                text: line[start..end].to_string(),
                fg: Some(theme.palette[10]), // bright green
                bg: code_bg,
                ..Default::default()
            });
            continue;
        }

        // Inline comment.
        if !def.comment_prefix.is_empty() && line[byte_pos..].starts_with(def.comment_prefix) {
            segments.push(StyledSegment {
                text: line[byte_pos..].to_string(),
                fg: Some(theme.palette[8]),
                bg: code_bg,
                italic: true,
                ..Default::default()
            });
            break;
        }

        // Word (identifier or keyword).
        if ch.is_alphanumeric() || ch == '_' {
            let start = byte_pos;
            while let Some(&(_, c)) = chars.peek() {
                if c.is_alphanumeric() || c == '_' {
                    chars.next();
                } else {
                    break;
                }
            }
            let end = chars.peek().map(|&(i, _)| i).unwrap_or(line.len());
            let word = &line[start..end];

            let fg = if def.keywords.contains(&word) {
                Some(theme.palette[13]) // bright magenta
            } else if def.builtins.contains(&word) {
                Some(theme.palette[14]) // bright cyan
            } else if word
                .chars()
                .all(|c| c.is_ascii_digit() || c == '_' || c == '.')
            {
                Some(theme.palette[11]) // bright yellow
            } else {
                None
            };

            segments.push(StyledSegment {
                text: word.to_string(),
                fg,
                bg: code_bg,
                ..Default::default()
            });
            continue;
        }

        // Number (starting with digit).
        if ch.is_ascii_digit() {
            let start = byte_pos;
            while let Some(&(_, c)) = chars.peek() {
                if c.is_ascii_digit() || c == '.' || c == 'x' || c == 'o' || c == 'b' || c == '_' {
                    chars.next();
                } else {
                    break;
                }
            }
            let end = chars.peek().map(|&(i, _)| i).unwrap_or(line.len());
            segments.push(StyledSegment {
                text: line[start..end].to_string(),
                fg: Some(theme.palette[11]), // bright yellow
                bg: code_bg,
                ..Default::default()
            });
            continue;
        }

        // Other character (punctuation, whitespace, etc.).
        let start = byte_pos;
        chars.next();
        let end = chars.peek().map(|&(i, _)| i).unwrap_or(line.len());
        segments.push(StyledSegment {
            text: line[start..end].to_string(),
            bg: code_bg,
            ..Default::default()
        });
    }

    if segments.is_empty() {
        // Empty line within code block.
        segments.push(StyledSegment {
            text: String::new(),
            bg: code_bg,
            ..Default::default()
        });
    }

    StyledLine::new(segments)
}

// ---------------------------------------------------------------------------
// MarkdownRenderer
// ---------------------------------------------------------------------------

/// Renders Markdown content into styled terminal output.
pub struct MarkdownRenderer {
    config: MarkdownRendererConfig,
    /// Diagram sub-renderer for fenced code blocks with diagram language tags.
    diagram_renderer: DiagramRenderer,
}

impl MarkdownRenderer {
    /// Create a new `MarkdownRenderer` with the given config.
    pub fn new(config: MarkdownRendererConfig) -> Self {
        Self::with_diagram_config(config, DiagramRendererConfig::default())
    }

    /// Create a new `MarkdownRenderer` with explicit diagram renderer config.
    pub fn with_diagram_config(
        config: MarkdownRendererConfig,
        diagram_config: DiagramRendererConfig,
    ) -> Self {
        Self {
            config,
            diagram_renderer: DiagramRenderer::new(diagram_config),
        }
    }

    /// Check if a fenced code block language should be sub-rendered by another renderer.
    ///
    /// Returns `true` for languages that have dedicated renderers in the registry
    /// (e.g., diagram languages like Mermaid, PlantUML). This enables multi-format
    /// handling where Markdown content embeds code blocks in other supported formats.
    pub fn should_sub_render(language: &str, registry: &RendererRegistry) -> bool {
        registry.get_renderer(language).is_some()
    }

    /// Render a single line, classifying it as a block-level element and
    /// then applying inline formatting within.
    fn render_line(
        &self,
        line: &str,
        renderer_config: &RendererConfig,
        footnote_links: &mut Option<Vec<String>>,
    ) -> StyledLine {
        let theme = &renderer_config.theme_colors;
        let width = renderer_config.terminal_width;

        // Header
        if let Some(caps) = re_header().captures(line) {
            let level = caps.get(1).unwrap().as_str().len();
            let content = caps.get(2).unwrap().as_str();
            return self.render_header(level, content, theme, footnote_links);
        }

        // Horizontal rule (check before unordered list since `---` could match list)
        if re_horizontal_rule().is_match(line) {
            return self.render_horizontal_rule(width, theme);
        }

        // Blockquote
        if let Some(caps) = re_blockquote().captures(line) {
            let content = caps.get(1).unwrap().as_str();
            return self.render_blockquote(content, theme, footnote_links);
        }

        // Unordered list
        if let Some(caps) = re_unordered_list().captures(line) {
            let indent = caps.get(1).unwrap().as_str();
            let content = caps.get(3).unwrap().as_str();
            return self.render_unordered_list(indent, content, theme, footnote_links);
        }

        // Ordered list
        if let Some(caps) = re_ordered_list().captures(line) {
            let indent = caps.get(1).unwrap().as_str();
            let number = caps.get(2).unwrap().as_str();
            let content = caps.get(3).unwrap().as_str();
            return self.render_ordered_list(indent, number, content, theme, footnote_links);
        }

        // Paragraph / plain line: apply inline formatting
        let segments = self.render_inline(line, theme, footnote_links);
        StyledLine::new(segments)
    }

    /// Render a header (H1–H6) with visual hierarchy.
    fn render_header(
        &self,
        level: usize,
        content: &str,
        theme: &ThemeColors,
        footnote_links: &mut Option<Vec<String>>,
    ) -> StyledLine {
        let segments = self.render_inline(content, theme, footnote_links);

        let styled = segments
            .into_iter()
            .map(|mut seg| {
                match self.config.header_style {
                    HeaderStyle::Colored => {
                        seg.fg = Some(header_color(level, theme));
                        seg.bold = level <= 2;
                    }
                    HeaderStyle::Bold => {
                        seg.bold = true;
                        seg.fg = Some(header_brightness(level, theme));
                    }
                    HeaderStyle::Underlined => {
                        if level <= 2 {
                            seg.underline = true;
                        }
                        seg.bold = true;
                        seg.fg = Some(header_color(level, theme));
                    }
                }
                seg
            })
            .collect();

        StyledLine::new(styled)
    }

    /// Render a horizontal rule as a full-width line.
    fn render_horizontal_rule(&self, width: usize, theme: &ThemeColors) -> StyledLine {
        let ch = match self.config.horizontal_rule_style {
            HorizontalRuleStyle::Thin => '─',
            HorizontalRuleStyle::Thick => '━',
            HorizontalRuleStyle::Dashed => '╌',
        };
        let rule_text: String = std::iter::repeat_n(ch, width).collect();
        StyledLine::new(vec![StyledSegment {
            text: rule_text,
            fg: Some(theme.palette[8]),
            ..Default::default()
        }])
    }

    /// Render a blockquote with left border and dimmed text.
    fn render_blockquote(
        &self,
        content: &str,
        theme: &ThemeColors,
        footnote_links: &mut Option<Vec<String>>,
    ) -> StyledLine {
        let mut segments = vec![StyledSegment {
            text: "▎ ".to_string(),
            fg: Some(theme.palette[6]),
            ..Default::default()
        }];

        let inline = self.render_inline(content, theme, footnote_links);
        for mut seg in inline {
            if seg.fg.is_none() {
                seg.fg = Some(theme.palette[7]);
            }
            seg.italic = true;
            segments.push(seg);
        }

        StyledLine::new(segments)
    }

    /// Render a bullet list item with styled bullet.
    fn render_unordered_list(
        &self,
        indent: &str,
        content: &str,
        theme: &ThemeColors,
        footnote_links: &mut Option<Vec<String>>,
    ) -> StyledLine {
        let bullet = match indent.len() / 2 {
            0 => "•",
            1 => "◦",
            _ => "▪",
        };

        let mut segments = vec![StyledSegment {
            text: format!("{indent}{bullet} "),
            fg: Some(theme.palette[6]),
            ..Default::default()
        }];

        segments.extend(self.render_inline(content, theme, footnote_links));
        StyledLine::new(segments)
    }

    /// Render an ordered list item with styled number.
    fn render_ordered_list(
        &self,
        indent: &str,
        number: &str,
        content: &str,
        theme: &ThemeColors,
        footnote_links: &mut Option<Vec<String>>,
    ) -> StyledLine {
        let mut segments = vec![StyledSegment {
            text: format!("{indent}{number} "),
            fg: Some(theme.palette[11]),
            bold: true,
            ..Default::default()
        }];

        segments.extend(self.render_inline(content, theme, footnote_links));
        StyledLine::new(segments)
    }

    /// Render inline elements within a text span.
    ///
    /// When `footnote_links` is `Some`, links are rendered with footnote-style
    /// `[N]` references and URLs are collected into the vector for later display.
    fn render_inline(
        &self,
        text: &str,
        theme: &ThemeColors,
        footnote_links: &mut Option<Vec<String>>,
    ) -> Vec<StyledSegment> {
        let spans = extract_inline_spans(text);

        if spans.is_empty() {
            return vec![StyledSegment {
                text: text.to_string(),
                ..Default::default()
            }];
        }

        let mut segments = Vec::new();
        let mut pos = 0;

        for span in &spans {
            if span.start > pos {
                segments.push(StyledSegment {
                    text: text[pos..span.start].to_string(),
                    ..Default::default()
                });
            }

            match &span.kind {
                SpanKind::Code(content) => {
                    segments.push(StyledSegment {
                        text: content.clone(),
                        fg: Some(theme.palette[9]),
                        bg: Some(subtle_bg(theme)),
                        ..Default::default()
                    });
                }
                SpanKind::Link { text: lt, url } => match self.config.link_style {
                    LinkStyle::UnderlineColor => {
                        segments.push(StyledSegment {
                            text: lt.clone(),
                            fg: Some(theme.palette[12]),
                            underline: true,
                            link_url: Some(url.clone()),
                            ..Default::default()
                        });
                    }
                    LinkStyle::InlineUrl => {
                        segments.push(StyledSegment {
                            text: format!("{lt} ({url})"),
                            fg: Some(theme.palette[12]),
                            underline: true,
                            ..Default::default()
                        });
                    }
                    LinkStyle::Footnote => {
                        // In footnote mode, footnote_links must be Some.
                        // We append the reference number inline and collect
                        // the URL for a footnote section at the end.
                        if let Some(footnotes) = footnote_links {
                            footnotes.push(url.clone());
                            let n = footnotes.len();
                            segments.push(StyledSegment {
                                text: lt.clone(),
                                fg: Some(theme.palette[12]),
                                underline: true,
                                ..Default::default()
                            });
                            segments.push(StyledSegment {
                                text: format!("[{n}]"),
                                fg: Some(theme.palette[8]),
                                ..Default::default()
                            });
                        } else {
                            // Fallback if footnote_links is None (shouldn't happen).
                            segments.push(StyledSegment {
                                text: lt.clone(),
                                fg: Some(theme.palette[12]),
                                underline: true,
                                link_url: Some(url.clone()),
                                ..Default::default()
                            });
                        }
                    }
                },
                SpanKind::BoldItalic(content) => {
                    segments.push(StyledSegment {
                        text: content.clone(),
                        bold: true,
                        italic: true,
                        ..Default::default()
                    });
                }
                SpanKind::Bold(content) => {
                    segments.push(StyledSegment {
                        text: content.clone(),
                        bold: true,
                        ..Default::default()
                    });
                }
                SpanKind::Italic(content) => {
                    segments.push(StyledSegment {
                        text: content.clone(),
                        italic: true,
                        ..Default::default()
                    });
                }
            }

            pos = span.end;
        }

        if pos < text.len() {
            segments.push(StyledSegment {
                text: text[pos..].to_string(),
                ..Default::default()
            });
        }

        segments
    }

    /// Render a fenced code block with optional syntax highlighting and background.
    fn render_code_block(
        &self,
        language: &Option<String>,
        code_lines: &[String],
        theme: &ThemeColors,
        width: usize,
    ) -> Vec<StyledLine> {
        let lang_def = language.as_deref().and_then(get_language_def);
        let show_bg = self.config.code_block_background;
        let code_bg = if show_bg {
            Some(subtle_bg(theme))
        } else {
            None
        };

        let mut lines = Vec::new();

        // Language label line (if language is specified).
        if let Some(lang) = language {
            let label = format!(" {lang} ");
            let padding = width.saturating_sub(label.len());
            let padded = format!("{label}{}", " ".repeat(padding));
            lines.push(StyledLine::new(vec![StyledSegment {
                text: padded,
                fg: Some(theme.palette[8]),
                bg: code_bg,
                bold: true,
                ..Default::default()
            }]));
        }

        // Highlighted code lines.
        for line in code_lines {
            lines.push(highlight_code_line(line, lang_def.as_ref(), theme, show_bg));
        }

        lines
    }

    /// Render a markdown table using the shared `TableRenderer`.
    fn render_table(
        &self,
        headers: &[String],
        rows: &[Vec<String>],
        alignments: &[ColumnAlignment],
        theme: &ThemeColors,
        max_width: usize,
    ) -> Vec<StyledLine> {
        let table_renderer = TableRenderer::new(
            self.config.table_style.clone(),
            self.config.table_border_color,
            header_color(3, theme), // use H3 color for table headers
        );
        table_renderer.render_table(headers, rows, alignments, max_width)
    }
}

/// Get the header color for a given level (1–6) from the theme palette.
fn header_color(level: usize, theme: &ThemeColors) -> [u8; 3] {
    match level {
        1 => theme.palette[14],
        2 => theme.palette[10],
        3 => theme.palette[11],
        4 => theme.palette[12],
        5 => theme.palette[13],
        _ => theme.palette[8],
    }
}

/// Get brightness-scaled color for Bold header style.
fn header_brightness(level: usize, theme: &ThemeColors) -> [u8; 3] {
    let base = theme.fg;
    let scale = 1.0 - (level as f32 - 1.0) * 0.12;
    [
        (base[0] as f32 * scale) as u8,
        (base[1] as f32 * scale) as u8,
        (base[2] as f32 * scale) as u8,
    ]
}

/// Compute a subtle background highlight for inline code / code blocks.
fn subtle_bg(theme: &ThemeColors) -> [u8; 3] {
    [
        theme.bg[0].saturating_add(25),
        theme.bg[1].saturating_add(25),
        theme.bg[2].saturating_add(25),
    ]
}

// ---------------------------------------------------------------------------
// ContentRenderer trait implementation
// ---------------------------------------------------------------------------

impl ContentRenderer for MarkdownRenderer {
    fn format_id(&self) -> &str {
        "markdown"
    }

    fn display_name(&self) -> &str {
        "Markdown"
    }

    fn capabilities(&self) -> Vec<RendererCapability> {
        vec![RendererCapability::TextStyling]
    }

    fn render(
        &self,
        content: &ContentBlock,
        config: &RendererConfig,
    ) -> Result<RenderedContent, crate::prettifier::traits::RenderError> {
        let theme = &config.theme_colors;
        let width = config.terminal_width;

        // Initialize footnote collection if using footnote link style.
        let mut footnote_links = match self.config.link_style {
            LinkStyle::Footnote => Some(Vec::new()),
            _ => None,
        };

        // Pass 1: classify lines into block-level elements.
        let blocks = classify_blocks(&content.lines);

        // Pass 2: render each block element.
        let mut lines = Vec::new();
        let mut line_mapping = Vec::new();
        let mut graphics: Vec<InlineGraphic> = Vec::new();

        for block in &blocks {
            match block {
                BlockElement::Line { source_idx } => {
                    let styled =
                        self.render_line(&content.lines[*source_idx], config, &mut footnote_links);
                    line_mapping.push(SourceLineMapping {
                        rendered_line: lines.len(),
                        source_line: Some(*source_idx),
                    });
                    lines.push(styled);
                }

                BlockElement::CodeBlock {
                    language,
                    lines: code_lines,
                    fence_open_idx,
                    fence_close_idx,
                } => {
                    // Check if this code block is a diagram language — if so,
                    // delegate to the DiagramRenderer for full backend rendering
                    // (local CLI / Kroki API / styled text fallback).
                    let is_diagram = language
                        .as_deref()
                        .is_some_and(|lang| self.diagram_renderer.is_diagram_language(lang));

                    if is_diagram {
                        let lang = language.as_deref().unwrap();
                        let source_refs: Vec<&str> =
                            code_lines.iter().map(String::as_str).collect();
                        let (diagram_lines, diagram_mappings, diagram_graphics) = self
                            .diagram_renderer
                            .render_diagram_section(lang, &source_refs, *fence_open_idx, config);

                        // Adjust line mappings to account for current output offset.
                        let offset = lines.len();
                        for mut mapping in diagram_mappings {
                            mapping.rendered_line += offset;
                            line_mapping.push(mapping);
                        }

                        // Adjust graphic row positions for current offset.
                        for mut graphic in diagram_graphics {
                            graphic.row += offset;
                            graphics.push(graphic);
                        }

                        // Closing fence: no rendered line, but record mapping.
                        if let Some(close_idx) = fence_close_idx {
                            line_mapping.push(SourceLineMapping {
                                rendered_line: lines.len() + diagram_lines.len(),
                                source_line: Some(*close_idx),
                            });
                        }

                        lines.extend(diagram_lines);
                    } else {
                        // Standard code block with syntax highlighting.
                        let rendered_code =
                            self.render_code_block(language, code_lines, theme, width);

                        if language.is_some() {
                            // Language label line maps to the opening fence source line.
                            line_mapping.push(SourceLineMapping {
                                rendered_line: lines.len(),
                                source_line: Some(*fence_open_idx),
                            });
                        }

                        // Code content lines map 1:1 to their source lines.
                        let content_start = if language.is_some() { 1 } else { 0 };
                        for (j, _) in rendered_code.iter().enumerate().skip(content_start) {
                            let source_line = fence_open_idx + 1 + (j - content_start);
                            line_mapping.push(SourceLineMapping {
                                rendered_line: lines.len() + j,
                                source_line: Some(source_line),
                            });
                        }

                        // Closing fence: no rendered line, but record mapping.
                        if let Some(close_idx) = fence_close_idx {
                            line_mapping.push(SourceLineMapping {
                                rendered_line: lines.len() + rendered_code.len(),
                                source_line: Some(*close_idx),
                            });
                        }

                        lines.extend(rendered_code);
                    }
                }

                BlockElement::Table {
                    headers,
                    alignments,
                    rows,
                    source_start,
                    source_end,
                } => {
                    let rendered_table = self.render_table(headers, rows, alignments, theme, width);

                    // Map rendered table lines back to source range.
                    // The source has: header (1 line) + separator (1 line) + N data rows.
                    // The rendered has: top border + header + separator + N data rows + bottom border.
                    let source_line_count = source_end - source_start;
                    for (j, _) in rendered_table.iter().enumerate() {
                        // Best-effort mapping: map to nearest source line.
                        let source_line = if source_line_count > 0 {
                            let ratio = j as f64 / rendered_table.len().max(1) as f64;
                            let mapped =
                                *source_start + (ratio * source_line_count as f64) as usize;
                            Some(mapped.min(source_end - 1))
                        } else {
                            Some(*source_start)
                        };
                        line_mapping.push(SourceLineMapping {
                            rendered_line: lines.len() + j,
                            source_line,
                        });
                    }

                    lines.extend(rendered_table);
                }
            }
        }

        // Append footnote references section if any links were collected.
        if let Some(ref footnotes) = footnote_links
            && !footnotes.is_empty()
        {
            // Blank separator line.
            line_mapping.push(SourceLineMapping {
                rendered_line: lines.len(),
                source_line: None,
            });
            lines.push(StyledLine::plain(""));

            // Horizontal rule.
            let rule: String = std::iter::repeat_n('─', width.min(40)).collect();
            line_mapping.push(SourceLineMapping {
                rendered_line: lines.len(),
                source_line: None,
            });
            lines.push(StyledLine::new(vec![StyledSegment {
                text: rule,
                fg: Some(theme.palette[8]),
                ..Default::default()
            }]));

            // Each footnote: [N]: url
            for (i, url) in footnotes.iter().enumerate() {
                line_mapping.push(SourceLineMapping {
                    rendered_line: lines.len(),
                    source_line: None,
                });
                lines.push(StyledLine::new(vec![
                    StyledSegment {
                        text: format!("[{}]", i + 1),
                        fg: Some(theme.palette[8]),
                        bold: true,
                        ..Default::default()
                    },
                    StyledSegment {
                        text: format!(": {url}"),
                        fg: Some(theme.palette[12]),
                        underline: true,
                        link_url: Some(url.clone()),
                        ..Default::default()
                    },
                ]));
            }
        }

        Ok(RenderedContent {
            lines,
            line_mapping,
            graphics,
            format_badge: "\u{1F4DD}".to_string(), // 📝
        })
    }

    fn format_badge(&self) -> &str {
        "MD"
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the markdown renderer with the registry.
pub fn register_markdown_renderer(
    registry: &mut RendererRegistry,
    config: &MarkdownRendererConfig,
) {
    registry.register_renderer("markdown", Box::new(MarkdownRenderer::new(config.clone())));
}

/// Register the markdown renderer with diagram sub-rendering support.
pub fn register_markdown_renderer_with_diagrams(
    registry: &mut RendererRegistry,
    config: &MarkdownRendererConfig,
    diagram_config: &DiagramRendererConfig,
) {
    registry.register_renderer(
        "markdown",
        Box::new(MarkdownRenderer::with_diagram_config(
            config.clone(),
            diagram_config.clone(),
        )),
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::traits::RendererConfig;
    use crate::prettifier::types::ContentBlock;
    use std::time::SystemTime;

    fn test_config() -> RendererConfig {
        RendererConfig {
            terminal_width: 80,
            ..Default::default()
        }
    }

    fn renderer() -> MarkdownRenderer {
        MarkdownRenderer::new(MarkdownRendererConfig::default())
    }

    fn make_block(lines: &[&str]) -> ContentBlock {
        ContentBlock {
            lines: lines.iter().map(|s| s.to_string()).collect(),
            preceding_command: None,
            start_row: 0,
            end_row: lines.len(),
            timestamp: SystemTime::now(),
        }
    }

    fn render_line(line: &str) -> StyledLine {
        renderer().render_line(line, &test_config(), &mut None)
    }

    fn segment_texts(line: &StyledLine) -> Vec<&str> {
        line.segments.iter().map(|s| s.text.as_str()).collect()
    }

    // -- ContentRenderer trait --

    #[test]
    fn test_format_id() {
        let r = renderer();
        assert_eq!(r.format_id(), "markdown");
        assert_eq!(r.display_name(), "Markdown");
        assert_eq!(r.format_badge(), "MD");
        assert_eq!(r.capabilities(), vec![RendererCapability::TextStyling]);
    }

    #[test]
    fn test_render_produces_correct_line_count() {
        let r = renderer();
        let block = make_block(&["# Hello", "World", "---"]);
        let result = r.render(&block, &test_config()).unwrap();
        assert_eq!(result.lines.len(), 3);
        assert_eq!(result.format_badge, "\u{1F4DD}");
    }

    #[test]
    fn test_line_mapping_correctness() {
        let r = renderer();
        let block = make_block(&["line0", "line1", "line2"]);
        let result = r.render(&block, &test_config()).unwrap();
        for (i, mapping) in result.line_mapping.iter().enumerate() {
            assert_eq!(mapping.rendered_line, i);
            assert_eq!(mapping.source_line, Some(i));
        }
    }

    // -- Headers --

    #[test]
    fn test_header_h1() {
        let line = render_line("# Hello World");
        let texts = segment_texts(&line);
        assert_eq!(texts, vec!["Hello World"]);
        assert!(line.segments[0].bold);
        assert!(line.segments[0].fg.is_some());
    }

    #[test]
    fn test_header_h2() {
        let line = render_line("## Subtitle");
        let texts = segment_texts(&line);
        assert_eq!(texts, vec!["Subtitle"]);
        assert!(line.segments[0].bold);
    }

    #[test]
    fn test_header_h3_through_h6() {
        for (level, prefix) in [(3, "###"), (4, "####"), (5, "#####"), (6, "######")] {
            let line = render_line(&format!("{prefix} Title"));
            let texts = segment_texts(&line);
            assert_eq!(texts, vec!["Title"], "H{level} text should be 'Title'");
            assert!(line.segments[0].fg.is_some(), "H{level} should have color");
        }
    }

    #[test]
    fn test_header_visual_hierarchy() {
        let theme = ThemeColors::default();
        let h1_color = header_color(1, &theme);
        let h6_color = header_color(6, &theme);
        assert_eq!(h1_color, theme.palette[14]);
        assert_eq!(h6_color, theme.palette[8]);
    }

    #[test]
    fn test_header_strips_prefix() {
        let line = render_line("## Keep this text");
        for seg in &line.segments {
            assert!(!seg.text.contains("##"), "Header prefix should be stripped");
        }
    }

    #[test]
    fn test_header_with_inline_bold() {
        let line = render_line("# Hello **World**");
        assert!(line.segments.len() >= 2);
        let bold_seg = line.segments.iter().find(|s| s.text == "World").unwrap();
        assert!(bold_seg.bold);
    }

    // -- Bold --

    #[test]
    fn test_bold_asterisks() {
        let line = render_line("This is **bold** text");
        assert_eq!(line.segments.len(), 3);
        assert_eq!(line.segments[0].text, "This is ");
        assert!(!line.segments[0].bold);
        assert_eq!(line.segments[1].text, "bold");
        assert!(line.segments[1].bold);
        assert_eq!(line.segments[2].text, " text");
    }

    #[test]
    fn test_bold_underscores() {
        let line = render_line("This is __bold__ text");
        let bold_seg = line.segments.iter().find(|s| s.text == "bold").unwrap();
        assert!(bold_seg.bold);
    }

    // -- Italic --

    #[test]
    fn test_italic_asterisks() {
        let line = render_line("This is *italic* text");
        let italic_seg = line.segments.iter().find(|s| s.text == "italic").unwrap();
        assert!(italic_seg.italic);
        assert!(!italic_seg.bold);
    }

    #[test]
    fn test_italic_underscores() {
        let line = render_line("This is _italic_ text");
        let italic_seg = line.segments.iter().find(|s| s.text == "italic").unwrap();
        assert!(italic_seg.italic);
    }

    // -- Bold + Italic --

    #[test]
    fn test_bold_italic() {
        let line = render_line("This is ***bold italic*** text");
        let bi_seg = line
            .segments
            .iter()
            .find(|s| s.text == "bold italic")
            .unwrap();
        assert!(bi_seg.bold);
        assert!(bi_seg.italic);
    }

    #[test]
    fn test_bold_italic_underscores() {
        let line = render_line("This is ___bold italic___ text");
        let bi_seg = line
            .segments
            .iter()
            .find(|s| s.text == "bold italic")
            .unwrap();
        assert!(bi_seg.bold);
        assert!(bi_seg.italic);
    }

    // -- Inline code --

    #[test]
    fn test_inline_code() {
        let line = render_line("Use `cargo build` to compile");
        let code_seg = line
            .segments
            .iter()
            .find(|s| s.text == "cargo build")
            .unwrap();
        assert!(code_seg.bg.is_some(), "Inline code should have background");
        assert!(
            code_seg.fg.is_some(),
            "Inline code should have foreground color"
        );
    }

    #[test]
    fn test_inline_code_is_opaque() {
        let line = render_line("Use `**not bold**` here");
        let code_seg = line
            .segments
            .iter()
            .find(|s| s.text == "**not bold**")
            .unwrap();
        assert!(code_seg.bg.is_some());
        assert!(!code_seg.bold);
    }

    // -- Links --

    #[test]
    fn test_link_underline_color() {
        let line = render_line("Visit [Example](https://example.com) now");
        let link_seg = line.segments.iter().find(|s| s.text == "Example").unwrap();
        assert!(link_seg.underline);
        assert!(link_seg.fg.is_some());
        assert_eq!(link_seg.link_url.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn test_link_inline_url_style() {
        let r = MarkdownRenderer::new(MarkdownRendererConfig {
            link_style: LinkStyle::InlineUrl,
            ..Default::default()
        });
        let line = r.render_line("See [Docs](https://docs.rs)", &test_config(), &mut None);
        let link_seg = line
            .segments
            .iter()
            .find(|s| s.text.contains("Docs"))
            .unwrap();
        assert!(link_seg.text.contains("https://docs.rs"));
    }

    // -- Blockquotes --

    #[test]
    fn test_blockquote() {
        let line = render_line("> This is a quote");
        assert!(line.segments[0].text.contains('▎'));
        let quote_seg = line
            .segments
            .iter()
            .find(|s| s.text.contains("This is a quote"))
            .unwrap();
        assert!(quote_seg.italic);
    }

    #[test]
    fn test_blockquote_with_inline() {
        let line = render_line("> This has **bold** text");
        let bold_seg = line.segments.iter().find(|s| s.text == "bold").unwrap();
        assert!(bold_seg.bold);
        assert!(bold_seg.italic);
    }

    // -- Lists --

    #[test]
    fn test_unordered_list_dash() {
        let line = render_line("- First item");
        assert!(line.segments[0].text.contains('•'));
        assert!(line.segments[0].fg.is_some());
    }

    #[test]
    fn test_unordered_list_asterisk() {
        let line = render_line("* Second item");
        assert!(line.segments[0].text.contains('•'));
    }

    #[test]
    fn test_unordered_list_plus() {
        let line = render_line("+ Third item");
        assert!(line.segments[0].text.contains('•'));
    }

    #[test]
    fn test_nested_unordered_list() {
        let line = render_line("  - Nested item");
        assert!(line.segments[0].text.contains('◦'));
    }

    #[test]
    fn test_deeply_nested_list() {
        let line = render_line("    - Deep item");
        assert!(line.segments[0].text.contains('▪'));
    }

    #[test]
    fn test_ordered_list() {
        let line = render_line("1. First step");
        assert!(line.segments[0].text.contains("1."));
        assert!(line.segments[0].bold);
    }

    #[test]
    fn test_ordered_list_paren() {
        let line = render_line("2) Second step");
        assert!(line.segments[0].text.contains("2)"));
    }

    #[test]
    fn test_list_with_inline_formatting() {
        let line = render_line("- This is **important**");
        let bold_seg = line
            .segments
            .iter()
            .find(|s| s.text == "important")
            .unwrap();
        assert!(bold_seg.bold);
    }

    // -- Horizontal rules --

    #[test]
    fn test_horizontal_rule_dashes() {
        let line = render_line("---");
        assert_eq!(line.segments.len(), 1);
        assert!(line.segments[0].text.contains('─'));
        assert_eq!(line.segments[0].text.chars().count(), 80);
    }

    #[test]
    fn test_horizontal_rule_asterisks() {
        let line = render_line("***");
        assert!(line.segments[0].text.contains('─'));
    }

    #[test]
    fn test_horizontal_rule_underscores() {
        let line = render_line("___");
        assert!(line.segments[0].text.contains('─'));
    }

    #[test]
    fn test_horizontal_rule_thick_style() {
        let r = MarkdownRenderer::new(MarkdownRendererConfig {
            horizontal_rule_style: HorizontalRuleStyle::Thick,
            ..Default::default()
        });
        let line = r.render_line("---", &test_config(), &mut None);
        assert!(line.segments[0].text.contains('━'));
    }

    #[test]
    fn test_horizontal_rule_dashed_style() {
        let r = MarkdownRenderer::new(MarkdownRendererConfig {
            horizontal_rule_style: HorizontalRuleStyle::Dashed,
            ..Default::default()
        });
        let line = r.render_line("---", &test_config(), &mut None);
        assert!(line.segments[0].text.contains('╌'));
    }

    // -- Plain text --

    #[test]
    fn test_plain_text_passthrough() {
        let line = render_line("Just plain text");
        assert_eq!(line.segments.len(), 1);
        assert_eq!(line.segments[0].text, "Just plain text");
        assert!(!line.segments[0].bold);
        assert!(!line.segments[0].italic);
    }

    #[test]
    fn test_empty_line() {
        let line = render_line("");
        assert_eq!(line.segments.len(), 1);
        assert_eq!(line.segments[0].text, "");
    }

    // -- Multiple inline elements --

    #[test]
    fn test_multiple_inline_elements() {
        let line = render_line("**Bold** and *italic* and `code`");
        assert!(line.segments.iter().any(|s| s.text == "Bold" && s.bold));
        assert!(line.segments.iter().any(|s| s.text == "italic" && s.italic));
        assert!(
            line.segments
                .iter()
                .any(|s| s.text == "code" && s.bg.is_some())
        );
    }

    // -- Registration --

    #[test]
    fn test_register_markdown_renderer() {
        let mut registry = RendererRegistry::new(0.6);
        register_markdown_renderer(&mut registry, &MarkdownRendererConfig::default());
        assert_eq!(registry.renderer_count(), 1);
        assert!(registry.get_renderer("markdown").is_some());
        assert_eq!(
            registry.get_renderer("markdown").unwrap().display_name(),
            "Markdown"
        );
    }

    // -- Header styles --

    #[test]
    fn test_header_bold_style() {
        let r = MarkdownRenderer::new(MarkdownRendererConfig {
            header_style: HeaderStyle::Bold,
            ..Default::default()
        });
        let line = r.render_line("# Title", &test_config(), &mut None);
        assert!(line.segments[0].bold);
    }

    #[test]
    fn test_header_underlined_style() {
        let r = MarkdownRenderer::new(MarkdownRendererConfig {
            header_style: HeaderStyle::Underlined,
            ..Default::default()
        });
        let line = r.render_line("# Title", &test_config(), &mut None);
        assert!(line.segments[0].underline);
        assert!(line.segments[0].bold);

        let line = r.render_line("### Title", &test_config(), &mut None);
        assert!(line.segments[0].bold);
        assert!(!line.segments[0].underline);
    }

    // -- Footnote link style --

    #[test]
    fn test_link_footnote_style_inline_ref() {
        let r = MarkdownRenderer::new(MarkdownRendererConfig {
            link_style: LinkStyle::Footnote,
            ..Default::default()
        });
        let block = make_block(&["See [Example](https://example.com) and [Docs](https://docs.rs)"]);
        let result = r.render(&block, &test_config()).unwrap();
        // Should have: content line + blank + rule + 2 footnote lines = 5 lines
        assert!(result.lines.len() >= 4);
        // The content line should have [1] and [2] references.
        let content_text: String = result.lines[0]
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(
            content_text.contains("[1]"),
            "Should have [1] reference: {content_text}"
        );
        assert!(
            content_text.contains("[2]"),
            "Should have [2] reference: {content_text}"
        );
        // Last two lines should be footnote references.
        let last = &result.lines[result.lines.len() - 1];
        let last_text: String = last.segments.iter().map(|s| s.text.as_str()).collect();
        assert!(last_text.contains("docs.rs"));
    }

    #[test]
    fn test_link_footnote_style_no_links() {
        let r = MarkdownRenderer::new(MarkdownRendererConfig {
            link_style: LinkStyle::Footnote,
            ..Default::default()
        });
        let block = make_block(&["No links here"]);
        let result = r.render(&block, &test_config()).unwrap();
        // No footnotes should be appended.
        assert_eq!(result.lines.len(), 1);
    }

    // -- Edge cases --

    #[test]
    fn test_code_span_prevents_bold_parsing() {
        let line = render_line("Check `**this**` out");
        assert!(line.segments.iter().any(|s| s.text == "**this**"));
        assert!(!line.segments.iter().any(|s| s.text == "this" && s.bold));
    }

    #[test]
    fn test_adjacent_formatting() {
        let line = render_line("**bold***italic*");
        assert!(line.segments.iter().any(|s| s.text == "bold" && s.bold));
        assert!(line.segments.iter().any(|s| s.text == "italic" && s.italic));
    }

    #[test]
    fn test_link_with_special_chars_in_url() {
        let line = render_line("[API](https://api.example.com/v1?key=val&foo=bar)");
        let link_seg = line.segments.iter().find(|s| s.link_url.is_some()).unwrap();
        assert_eq!(
            link_seg.link_url.as_deref(),
            Some("https://api.example.com/v1?key=val&foo=bar")
        );
    }

    // -- Helper function tests --

    #[test]
    fn test_subtle_bg() {
        let theme = ThemeColors::default();
        let bg = subtle_bg(&theme);
        assert_eq!(bg, [55, 55, 71]);
    }

    #[test]
    fn test_header_brightness_scaling() {
        let theme = ThemeColors::default();
        let h1 = header_brightness(1, &theme);
        let h6 = header_brightness(6, &theme);
        assert!(h1[0] >= h6[0]);
        assert!(h1[1] >= h6[1]);
        assert!(h1[2] >= h6[2]);
    }

    // =====================================================================
    // Fenced code block tests
    // =====================================================================

    #[test]
    fn test_code_block_fence_markers_stripped() {
        let r = renderer();
        let block = make_block(&["```rust", "let x = 42;", "```"]);
        let result = r.render(&block, &test_config()).unwrap();
        // Fence markers should not appear in rendered output.
        for line in &result.lines {
            for seg in &line.segments {
                assert!(
                    !seg.text.contains("```"),
                    "Fence markers should be stripped, got: {:?}",
                    seg.text
                );
            }
        }
    }

    #[test]
    fn test_code_block_language_label() {
        let r = renderer();
        let block = make_block(&["```python", "print('hello')", "```"]);
        let result = r.render(&block, &test_config()).unwrap();
        // First rendered line should contain the language label.
        let first_text: String = result.lines[0]
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(
            first_text.contains("python"),
            "Language label should be displayed"
        );
    }

    #[test]
    fn test_code_block_no_language() {
        let r = renderer();
        let block = make_block(&["```", "plain code", "```"]);
        let result = r.render(&block, &test_config()).unwrap();
        // Without a language, no label line is added.
        // Should have just the code line.
        assert_eq!(result.lines.len(), 1);
        let text: String = result.lines[0]
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(text.contains("plain code"));
    }

    #[test]
    fn test_code_block_background_shading() {
        let r = renderer();
        let block = make_block(&["```", "code line", "```"]);
        let result = r.render(&block, &test_config()).unwrap();
        // Code lines should have background set.
        assert!(result.lines[0].segments[0].bg.is_some());
    }

    #[test]
    fn test_code_block_background_disabled() {
        let r = MarkdownRenderer::new(MarkdownRendererConfig {
            code_block_background: false,
            ..Default::default()
        });
        let block = make_block(&["```", "code line", "```"]);
        let result = r.render(&block, &test_config()).unwrap();
        // Background should be None when disabled.
        assert!(result.lines[0].segments[0].bg.is_none());
    }

    #[test]
    fn test_code_block_preserves_whitespace() {
        let r = renderer();
        let block = make_block(&["```", "  indented", "    more indent", "```"]);
        let result = r.render(&block, &test_config()).unwrap();
        let line0_text: String = result.lines[0]
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(line0_text.contains("  indented"));
        let line1_text: String = result.lines[1]
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(line1_text.contains("    more indent"));
    }

    #[test]
    fn test_code_block_tilde_fences() {
        let r = renderer();
        let block = make_block(&["~~~", "tilde code", "~~~"]);
        let result = r.render(&block, &test_config()).unwrap();
        let text: String = result.lines[0]
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(text.contains("tilde code"));
    }

    #[test]
    fn test_code_block_line_mapping() {
        let r = renderer();
        // Source: 0=fence, 1=code, 2=code, 3=fence
        let block = make_block(&["```rust", "line1", "line2", "```"]);
        let result = r.render(&block, &test_config()).unwrap();
        // Rendered: 0=label, 1=line1, 2=line2
        assert_eq!(result.lines.len(), 3);
        // Verify mappings exist and point to valid source lines.
        for mapping in &result.line_mapping {
            assert!(mapping.source_line.is_some());
        }
    }

    // -- Syntax highlighting --

    #[test]
    fn test_rust_keyword_highlighting() {
        let theme = ThemeColors::default();
        let lang_def = get_language_def("rust").unwrap();
        let line = highlight_code_line("let x = 42;", Some(&lang_def), &theme, false);
        // "let" should be highlighted as a keyword (bright magenta).
        let let_seg = line.segments.iter().find(|s| s.text == "let").unwrap();
        assert_eq!(let_seg.fg, Some(theme.palette[13]));
    }

    #[test]
    fn test_python_keyword_highlighting() {
        let theme = ThemeColors::default();
        let lang_def = get_language_def("python").unwrap();
        let line = highlight_code_line("def hello():", Some(&lang_def), &theme, false);
        let def_seg = line.segments.iter().find(|s| s.text == "def").unwrap();
        assert_eq!(def_seg.fg, Some(theme.palette[13]));
    }

    #[test]
    fn test_comment_highlighting() {
        let theme = ThemeColors::default();
        let lang_def = get_language_def("rust").unwrap();
        let line = highlight_code_line("// this is a comment", Some(&lang_def), &theme, false);
        // Entire line should be comment-colored (dim grey, italic).
        assert_eq!(line.segments.len(), 1);
        assert_eq!(line.segments[0].fg, Some(theme.palette[8]));
        assert!(line.segments[0].italic);
    }

    #[test]
    fn test_string_highlighting() {
        let theme = ThemeColors::default();
        let lang_def = get_language_def("rust").unwrap();
        let line = highlight_code_line(r#"let s = "hello";"#, Some(&lang_def), &theme, false);
        let str_seg = line
            .segments
            .iter()
            .find(|s| s.text.contains("hello"))
            .unwrap();
        assert_eq!(str_seg.fg, Some(theme.palette[10])); // bright green
    }

    #[test]
    fn test_json_highlighting() {
        assert!(get_language_def("json").is_some());
    }

    #[test]
    fn test_unknown_language() {
        assert!(get_language_def("brainfuck").is_none());
    }

    #[test]
    fn test_highlight_no_language_def() {
        let theme = ThemeColors::default();
        let line = highlight_code_line("just text", None, &theme, true);
        assert_eq!(line.segments.len(), 1);
        assert_eq!(line.segments[0].text, "just text");
        assert!(line.segments[0].bg.is_some());
    }

    // =====================================================================
    // Table rendering tests
    // =====================================================================

    #[test]
    fn test_table_renders_with_box_drawing() {
        let r = renderer();
        let block = make_block(&[
            "| Name  | Age |",
            "|-------|-----|",
            "| Alice | 30  |",
            "| Bob   | 25  |",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        // Should produce: top border + header + separator + 2 data rows + bottom border = 6 lines.
        assert_eq!(result.lines.len(), 6);

        // Top border should use box-drawing.
        let top_text: String = result.lines[0]
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(top_text.contains('┌') || top_text.contains('+'));
    }

    #[test]
    fn test_table_header_is_bold() {
        let r = renderer();
        let block = make_block(&["| Name  | Age |", "|-------|-----|", "| Alice | 30  |"]);
        let result = r.render(&block, &test_config()).unwrap();
        // Header row (index 1): cell segments should be bold.
        let header_row = &result.lines[1];
        let name_seg = header_row.segments.iter().find(|s| s.text.contains("Name"));
        assert!(name_seg.is_some());
        assert!(name_seg.unwrap().bold);
    }

    #[test]
    fn test_table_column_alignment() {
        let r = renderer();
        let block = make_block(&[
            "| Left | Center | Right |",
            "|:-----|:------:|------:|",
            "| a    | b      | c     |",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        // Should render without errors and produce table lines.
        assert!(result.lines.len() >= 5);
    }

    #[test]
    fn test_table_border_color() {
        let r = MarkdownRenderer::new(MarkdownRendererConfig {
            table_border_color: [200, 100, 50],
            ..Default::default()
        });
        let block = make_block(&["| A |", "|---|", "| B |"]);
        let result = r.render(&block, &test_config()).unwrap();
        // Top border should use the configured color.
        assert_eq!(result.lines[0].segments[0].fg, Some([200, 100, 50]));
    }

    #[test]
    fn test_table_line_mapping() {
        let r = renderer();
        let block = make_block(&["| A |", "|---|", "| 1 |"]);
        let result = r.render(&block, &test_config()).unwrap();
        // All rendered lines should have source line mappings.
        for mapping in &result.line_mapping {
            assert!(mapping.source_line.is_some());
        }
    }

    #[test]
    fn test_table_followed_by_text() {
        let r = renderer();
        let block = make_block(&[
            "| A | B |",
            "|---|---|",
            "| 1 | 2 |",
            "",
            "Some text after table",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        // Last line should be the plain text.
        let last_text: String = result
            .lines
            .last()
            .unwrap()
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(last_text.contains("Some text after table"));
    }

    #[test]
    fn test_mixed_content() {
        let r = renderer();
        let block = make_block(&[
            "# Title",
            "",
            "```rust",
            "let x = 1;",
            "```",
            "",
            "| A |",
            "|---|",
            "| 1 |",
            "",
            "End",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        // Should render without panics.
        assert!(result.lines.len() >= 8);
    }

    // -- Table helper tests --

    #[test]
    fn test_is_table_row() {
        assert!(is_table_row("| A | B |"));
        assert!(is_table_row("| A |"));
        assert!(!is_table_row("no pipes here"));
        assert!(!is_table_row(""));
    }

    #[test]
    fn test_is_separator_row() {
        assert!(is_separator_row("|---|---|"));
        assert!(is_separator_row("| --- | --- |"));
        assert!(is_separator_row("|:---|:---:|---:|"));
        assert!(!is_separator_row("| A | B |"));
        assert!(!is_separator_row("plain text"));
    }

    #[test]
    fn test_parse_table_cells() {
        let cells = parse_table_cells("| A | B | C |");
        assert_eq!(cells, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_parse_alignment() {
        assert_eq!(parse_alignment(":---"), ColumnAlignment::Left);
        assert_eq!(parse_alignment(":---:"), ColumnAlignment::Center);
        assert_eq!(parse_alignment("---:"), ColumnAlignment::Right);
        assert_eq!(parse_alignment("---"), ColumnAlignment::Left);
    }

    // -- Block classification tests --

    #[test]
    fn test_classify_code_block() {
        let lines: Vec<String> = vec![
            "```rust".to_string(),
            "let x = 1;".to_string(),
            "```".to_string(),
        ];
        let blocks = classify_blocks(&lines);
        assert_eq!(blocks.len(), 1);
        assert!(
            matches!(&blocks[0], BlockElement::CodeBlock { language: Some(lang), .. } if lang == "rust")
        );
    }

    #[test]
    fn test_classify_table() {
        let lines: Vec<String> = vec![
            "| A | B |".to_string(),
            "|---|---|".to_string(),
            "| 1 | 2 |".to_string(),
        ];
        let blocks = classify_blocks(&lines);
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], BlockElement::Table { .. }));
    }

    #[test]
    fn test_classify_mixed() {
        let lines: Vec<String> = vec![
            "Hello".to_string(),
            "```".to_string(),
            "code".to_string(),
            "```".to_string(),
            "World".to_string(),
        ];
        let blocks = classify_blocks(&lines);
        assert_eq!(blocks.len(), 3); // Line, CodeBlock, Line
    }

    #[test]
    fn test_unclosed_code_block() {
        let lines: Vec<String> = vec![
            "```rust".to_string(),
            "let x = 1;".to_string(),
            // No closing fence.
        ];
        let blocks = classify_blocks(&lines);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            BlockElement::CodeBlock {
                fence_close_idx, ..
            } => {
                assert!(fence_close_idx.is_none());
            }
            _ => panic!("Expected CodeBlock"),
        }
    }
}

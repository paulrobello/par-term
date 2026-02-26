//! Block-level element classification for the Markdown renderer.
//!
//! Defines [`BlockElement`] and the [`classify_blocks`] first-pass function
//! that scans source lines and groups them into fenced code blocks, pipe-
//! delimited tables, and individual text lines.

use regex::Regex;
use std::sync::OnceLock;

use crate::prettifier::renderers::table::ColumnAlignment;

// ---------------------------------------------------------------------------
// Compiled block-level regexes (shared with markdown.rs main module)
// ---------------------------------------------------------------------------

pub(super) fn re_fence_open() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(\s*)(```|~~~)(\w*)\s*$")
            .expect("re_fence_open: pattern is valid and should always compile")
    })
}

// ---------------------------------------------------------------------------
// Block element type
// ---------------------------------------------------------------------------

/// A block-level element identified during the first pass.
pub(super) enum BlockElement {
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

// ---------------------------------------------------------------------------
// Table helper functions
// ---------------------------------------------------------------------------

/// Check if a line is a markdown table row (has pipe separators).
pub(super) fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('|') && !trimmed.is_empty()
}

/// Check if a line is a table separator row (e.g., `|---|:---:|---:|`).
pub(super) fn is_separator_row(line: &str) -> bool {
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
pub(super) fn parse_table_cells(line: &str) -> Vec<String> {
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
pub(super) fn parse_alignment(cell: &str) -> ColumnAlignment {
    let c = cell.trim();
    let starts_colon = c.starts_with(':');
    let ends_colon = c.ends_with(':');
    match (starts_colon, ends_colon) {
        (true, true) => ColumnAlignment::Center,
        (false, true) => ColumnAlignment::Right,
        _ => ColumnAlignment::Left,
    }
}

// ---------------------------------------------------------------------------
// Block classifier
// ---------------------------------------------------------------------------

/// First pass: classify source lines into block-level elements.
pub(super) fn classify_blocks(lines: &[String]) -> Vec<BlockElement> {
    let mut blocks = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        // Check for fenced code block opening.
        if let Some(caps) = re_fence_open().captures(&lines[i]) {
            let delimiter = caps
                .get(2)
                .expect("re_fence_open capture group 2 (delimiter) must be present after a match")
                .as_str()
                .to_string();
            let lang_str = caps
                .get(3)
                .expect("re_fence_open capture group 3 (language) must be present after a match")
                .as_str()
                .to_string();
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

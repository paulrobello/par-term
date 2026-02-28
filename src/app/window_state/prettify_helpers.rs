//! Helpers for prettifying terminal output from Claude Code and similar tools.
//!
//! These functions reconstruct markdown syntax from ANSI-styled cells so that
//! the prettifier's markdown detector can recognise patterns such as
//! `# Header` and `**bold**`, and pre-process raw Claude Code segment text
//! before detection (line-number stripping, UI-chrome filtering).

/// Reconstruct markdown syntax from cell attributes for Claude Code output.
///
/// Claude Code pre-renders markdown with ANSI sequences (bold for headers/emphasis,
/// italic for emphasis), stripping the original syntax markers. This function
/// reconstructs markdown syntax from cell attributes so the prettifier's markdown
/// detector can recognize patterns like `# Header` and `**bold**`.
pub(crate) fn reconstruct_markdown_from_cells(cells: &[par_term_config::Cell]) -> String {
    // First, extract plain text and trim trailing whitespace.
    let trimmed_len = cells
        .iter()
        .rposition(|c| {
            let g = c.grapheme.as_str();
            !(g.is_empty() || g == "\0" || g == " ")
        })
        .map(|i| i + 1)
        .unwrap_or(0);

    if trimmed_len == 0 {
        return String::new();
    }

    let cells = &cells[..trimmed_len];

    // Find the first non-whitespace cell index.
    let first_nonws = cells
        .iter()
        .position(|c| {
            let g = c.grapheme.as_str();
            !(g.is_empty() || g == "\0" || g == " ")
        })
        .unwrap_or(0);

    // Check if all non-whitespace cells share the same attribute pattern (for header detection).
    let all_bold = cells[first_nonws..].iter().all(|c| {
        let g = c.grapheme.as_str();
        (g.is_empty() || g == "\0" || g == " ") || c.bold
    });

    let all_underline = all_bold
        && cells[first_nonws..].iter().all(|c| {
            let g = c.grapheme.as_str();
            (g.is_empty() || g == "\0" || g == " ") || c.underline
        });

    // Extract the plain text content.
    let plain_text: String = cells
        .iter()
        .map(|c| {
            let g = c.grapheme.as_str();
            if g.is_empty() || g == "\0" { " " } else { g }
        })
        .collect::<String>()
        .trim_end()
        .to_string();

    // Header detection: if every non-ws cell is bold, it's a header.
    // Bold + underline → H1 (`# `), bold only → H2 (`## `).
    if all_bold && !plain_text.trim().is_empty() {
        let trimmed = plain_text.trim_start();
        // Don't add header markers to lines that already look like list items, tables, etc.
        if !trimmed.starts_with('-')
            && !trimmed.starts_with('*')
            && !trimmed.starts_with('|')
            && !trimmed.starts_with('#')
        {
            return if all_underline {
                format!("# {trimmed}")
            } else {
                format!("## {trimmed}")
            };
        }
    }

    // Inline bold/italic reconstruction: track attribute transitions.
    let mut result = String::with_capacity(plain_text.len() + 32);
    let mut in_bold = false;
    let mut in_italic = false;

    for cell in cells {
        let g = cell.grapheme.as_str();
        let ch = if g.is_empty() || g == "\0" { " " } else { g };

        // Bold transitions (skip if whole line is bold — already handled as header).
        if !all_bold {
            if cell.bold && !in_bold {
                result.push_str("**");
                in_bold = true;
            } else if !cell.bold && in_bold {
                result.push_str("**");
                in_bold = false;
            }
        }

        // Italic transitions.
        if cell.italic && !in_italic {
            result.push('*');
            in_italic = true;
        } else if !cell.italic && in_italic {
            result.push('*');
            in_italic = false;
        }

        result.push_str(ch);
    }

    // Close any open markers.
    if in_bold {
        result.push_str("**");
    }
    if in_italic {
        result.push('*');
    }

    result.trim_end().to_string()
}

/// Preprocess a Claude Code segment before detection.
///
/// 1. **Line-number stripping**: File previews show numbered lines like
///    `"       1 # Defect Report"`. We strip the prefix so detectors see
///    `"# Defect Report"` and can match ATX headers / fenced code.
/// 2. **UI chrome filtering**: Remove tool headers reconstructed as `## Write(`,
///    tree connectors (`└`, `├`), and other TUI elements that confuse detectors.
pub(crate) fn preprocess_claude_code_segment(lines: &mut Vec<(String, usize)>) {
    use std::sync::LazyLock;

    /// Matches Claude Code line-number prefixes: leading whitespace + digits + space.
    /// Examples: "    1 ", "   10 ", "  100 "
    static LINE_NUMBER_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"^\s+\d+\s").expect("LINE_NUMBER_RE is a valid static regex pattern")
    });

    /// Captures the line-number prefix for stripping.
    static LINE_NUMBER_STRIP_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"^(\s+\d+) ")
            .expect("LINE_NUMBER_STRIP_RE is a valid static regex pattern")
    });

    if lines.is_empty() {
        return;
    }

    // Detect line-numbered content: if ≥50% of non-empty lines have a line-number
    // prefix, this is a file preview. Strip the prefix so detectors see raw content.
    let non_empty: Vec<&(String, usize)> =
        lines.iter().filter(|(l, _)| !l.trim().is_empty()).collect();
    if !non_empty.is_empty() {
        let numbered_count = non_empty
            .iter()
            .filter(|(l, _)| LINE_NUMBER_RE.is_match(l))
            .count();
        if numbered_count * 2 >= non_empty.len() {
            for (line, _) in lines.iter_mut() {
                if let Some(m) = LINE_NUMBER_STRIP_RE.find(line) {
                    *line = line[m.end()..].to_string();
                }
            }
        }
    }

    // Filter out Claude Code UI chrome lines that would confuse detectors:
    // - Tool headers reconstructed as ## headers: "## Write(", "## Bash(", "## Read("
    // - Result indicators with tree connectors: starts with "└"
    // - "Wrote N lines to..." result summaries
    lines.retain(|(line, _)| {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return true; // Keep blank lines (they serve as block separators)
        }
        // Skip tool headers that were reconstructed as markdown headers.
        if let Some(after_header) = trimmed.strip_prefix("## ")
            && (after_header.starts_with("Write(")
                || after_header.starts_with("Bash(")
                || after_header.starts_with("Read(")
                || after_header.starts_with("Glob(")
                || after_header.starts_with("Grep(")
                || after_header.starts_with("Edit(")
                || after_header.starts_with("Task(")
                || after_header.starts_with("Wrote ")
                || after_header.starts_with("Done"))
        {
            return false;
        }
        // Skip tree-connector result lines.
        if trimmed.starts_with('└') || trimmed.starts_with('├') {
            return false;
        }
        true
    });
}

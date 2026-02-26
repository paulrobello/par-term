//! Shared table rendering infrastructure.
//!
//! Renders tabular data using Unicode box-drawing characters with configurable
//! styles. Used by the Markdown renderer for tables and will be reused by CSV,
//! SQL result, and JSON tabular renderers in later steps.

use crate::prettifier::types::{StyledLine, StyledSegment};

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// Visual style for table borders.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum TableStyle {
    /// Unicode box-drawing: `┌─┬─┐`, `│ │ │`, `├─┼─┤`, `└─┴─┘`
    #[default]
    Unicode,
    /// ASCII: `+---+---+`, `| | |`, `+---+---+`
    Ascii,
    /// Rounded corners: `╭─┬─╮`, `│ │ │`, `├─┼─┤`, `╰─┴─╯`
    Rounded,
}

/// Column alignment.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ColumnAlignment {
    #[default]
    Left,
    Center,
    Right,
}

// ---------------------------------------------------------------------------
// Box-drawing character sets
// ---------------------------------------------------------------------------

struct BoxChars {
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
    horizontal: char,
    vertical: char,
    top_tee: char,
    bottom_tee: char,
    left_tee: char,
    right_tee: char,
    cross: char,
}

impl BoxChars {
    fn for_style(style: &TableStyle) -> Self {
        match style {
            TableStyle::Unicode => Self {
                top_left: '┌',
                top_right: '┐',
                bottom_left: '└',
                bottom_right: '┘',
                horizontal: '─',
                vertical: '│',
                top_tee: '┬',
                bottom_tee: '┴',
                left_tee: '├',
                right_tee: '┤',
                cross: '┼',
            },
            TableStyle::Ascii => Self {
                top_left: '+',
                top_right: '+',
                bottom_left: '+',
                bottom_right: '+',
                horizontal: '-',
                vertical: '|',
                top_tee: '+',
                bottom_tee: '+',
                left_tee: '+',
                right_tee: '+',
                cross: '+',
            },
            TableStyle::Rounded => Self {
                top_left: '╭',
                top_right: '╮',
                bottom_left: '╰',
                bottom_right: '╯',
                horizontal: '─',
                vertical: '│',
                top_tee: '┬',
                bottom_tee: '┴',
                left_tee: '├',
                right_tee: '┤',
                cross: '┼',
            },
        }
    }
}

// ---------------------------------------------------------------------------
// TableRenderer
// ---------------------------------------------------------------------------

/// Shared table rendering infrastructure.
///
/// Renders tabular data with box-drawing borders, column alignment, and header
/// styling. Designed to be reused across Markdown tables, CSV, SQL results, etc.
pub struct TableRenderer {
    /// Visual style for table borders.
    pub style: TableStyle,
    /// Border color as \[r, g, b\].
    pub border_color: [u8; 3],
    /// Header foreground color (bold) as \[r, g, b\].
    pub header_fg: [u8; 3],
}

impl TableRenderer {
    /// Create a new `TableRenderer` with the given style and colors.
    pub fn new(style: TableStyle, border_color: [u8; 3], header_fg: [u8; 3]) -> Self {
        Self {
            style,
            border_color,
            header_fg,
        }
    }

    /// Render a table from headers, rows, and alignments.
    ///
    /// Returns a `Vec<StyledLine>` containing the top border, header row, header
    /// separator, data rows, and bottom border.
    ///
    /// `max_width` limits the total table width; columns are shrunk proportionally
    /// if the natural width exceeds it.
    pub fn render_table(
        &self,
        headers: &[String],
        rows: &[Vec<String>],
        alignments: &[ColumnAlignment],
        max_width: usize,
    ) -> Vec<StyledLine> {
        if headers.is_empty() {
            return vec![];
        }

        let col_count = headers.len();

        // Compute natural column widths from content.
        let mut col_widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
        for row in rows {
            for (i, cell) in row.iter().enumerate() {
                if i < col_count {
                    col_widths[i] = col_widths[i].max(cell.chars().count());
                }
            }
        }

        // Ensure minimum width of 1 per column.
        for w in &mut col_widths {
            if *w == 0 {
                *w = 1;
            }
        }

        // Shrink columns if total exceeds max_width.
        // Total = borders (col_count + 1) + padding (2 per col) + content widths.
        let overhead = col_count + 1 + col_count * 2;
        let content_width: usize = col_widths.iter().sum();
        let total = overhead + content_width;

        if total > max_width && content_width > 0 {
            let available = max_width.saturating_sub(overhead);
            if available > 0 {
                // Proportional shrink.
                let scale = available as f64 / content_width as f64;
                for w in &mut col_widths {
                    *w = ((*w as f64 * scale).floor() as usize).max(1);
                }
            }
        }

        let chars = BoxChars::for_style(&self.style);
        let mut lines = Vec::new();

        // Top border.
        lines.push(self.render_horizontal_border(
            &col_widths,
            chars.top_left,
            chars.top_tee,
            chars.top_right,
            chars.horizontal,
        ));

        // Header row.
        lines.push(self.render_data_row(headers, &col_widths, alignments, chars.vertical, true));

        // Header separator.
        lines.push(self.render_horizontal_border(
            &col_widths,
            chars.left_tee,
            chars.cross,
            chars.right_tee,
            chars.horizontal,
        ));

        // Data rows.
        for row in rows {
            lines.push(self.render_data_row(row, &col_widths, alignments, chars.vertical, false));
        }

        // Bottom border.
        lines.push(self.render_horizontal_border(
            &col_widths,
            chars.bottom_left,
            chars.bottom_tee,
            chars.bottom_right,
            chars.horizontal,
        ));

        lines
    }

    /// Render a horizontal border line (top, separator, or bottom).
    fn render_horizontal_border(
        &self,
        col_widths: &[usize],
        left: char,
        mid: char,
        right: char,
        fill: char,
    ) -> StyledLine {
        let mut text = String::new();
        text.push(left);
        for (i, &w) in col_widths.iter().enumerate() {
            // +2 for padding on each side.
            for _ in 0..w + 2 {
                text.push(fill);
            }
            if i < col_widths.len() - 1 {
                text.push(mid);
            }
        }
        text.push(right);

        StyledLine::new(vec![StyledSegment {
            text,
            fg: Some(self.border_color),
            ..Default::default()
        }])
    }

    /// Render a data row (header or body).
    fn render_data_row(
        &self,
        cells: &[String],
        col_widths: &[usize],
        alignments: &[ColumnAlignment],
        vertical: char,
        is_header: bool,
    ) -> StyledLine {
        let mut segments = Vec::new();
        let vert = vertical.to_string();

        // Leading border.
        segments.push(StyledSegment {
            text: vert.clone(),
            fg: Some(self.border_color),
            ..Default::default()
        });

        for (i, width) in col_widths.iter().enumerate() {
            let cell_text = cells.get(i).map(|s| s.as_str()).unwrap_or("");
            let alignment = alignments.get(i).unwrap_or(&ColumnAlignment::Left);
            let padded = align_text(cell_text, *width, alignment);

            // Space padding around content.
            segments.push(StyledSegment {
                text: format!(" {padded} "),
                fg: if is_header {
                    Some(self.header_fg)
                } else {
                    None
                },
                bold: is_header,
                ..Default::default()
            });

            // Column separator or trailing border.
            segments.push(StyledSegment {
                text: vert.clone(),
                fg: Some(self.border_color),
                ..Default::default()
            });
        }

        StyledLine::new(segments)
    }
}

/// Align text within a field of the given width.
fn align_text(text: &str, width: usize, alignment: &ColumnAlignment) -> String {
    let text_len = text.chars().count();
    if text_len >= width {
        return text.chars().take(width).collect();
    }

    let padding = width - text_len;
    match alignment {
        ColumnAlignment::Left => format!("{text}{}", " ".repeat(padding)),
        ColumnAlignment::Right => format!("{}{text}", " ".repeat(padding)),
        ColumnAlignment::Center => {
            let left_pad = padding / 2;
            let right_pad = padding - left_pad;
            format!("{}{text}{}", " ".repeat(left_pad), " ".repeat(right_pad))
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_renderer() -> TableRenderer {
        TableRenderer::new(
            TableStyle::Unicode,
            [108, 112, 134], // overlay border
            [205, 214, 244], // text header
        )
    }

    #[test]
    fn test_empty_headers() {
        let r = default_renderer();
        let result = r.render_table(&[], &[], &[], 80);
        assert!(result.is_empty());
    }

    #[test]
    fn test_single_column_table() {
        let r = default_renderer();
        let headers = vec!["Name".to_string()];
        let rows = vec![vec!["Alice".to_string()], vec!["Bob".to_string()]];
        let alignments = vec![ColumnAlignment::Left];
        let result = r.render_table(&headers, &rows, &alignments, 80);

        // Should have: top border + header + separator + 2 data rows + bottom border = 6 lines.
        assert_eq!(result.len(), 6);
    }

    #[test]
    fn test_multi_column_table() {
        let r = default_renderer();
        let headers = vec!["Name".to_string(), "Age".to_string()];
        let rows = vec![
            vec!["Alice".to_string(), "30".to_string()],
            vec!["Bob".to_string(), "25".to_string()],
        ];
        let alignments = vec![ColumnAlignment::Left, ColumnAlignment::Right];
        let result = r.render_table(&headers, &rows, &alignments, 80);

        assert_eq!(result.len(), 6);

        // Check that borders use box-drawing characters.
        let top_text = &result[0].segments[0].text;
        assert!(top_text.starts_with('┌'));
        assert!(top_text.ends_with('┐'));
        assert!(top_text.contains('┬'));
    }

    #[test]
    fn test_ascii_style() {
        let r = TableRenderer::new(TableStyle::Ascii, [108, 112, 134], [205, 214, 244]);
        let headers = vec!["A".to_string(), "B".to_string()];
        let rows = vec![vec!["1".to_string(), "2".to_string()]];
        let result = r.render_table(&headers, &rows, &[], 80);

        let top_text = &result[0].segments[0].text;
        assert!(top_text.starts_with('+'));
        assert!(top_text.contains('-'));
    }

    #[test]
    fn test_rounded_style() {
        let r = TableRenderer::new(TableStyle::Rounded, [108, 112, 134], [205, 214, 244]);
        let headers = vec!["X".to_string()];
        let rows = vec![vec!["Y".to_string()]];
        let result = r.render_table(&headers, &rows, &[], 80);

        let top_text = &result[0].segments[0].text;
        assert!(top_text.starts_with('╭'));
        assert!(top_text.ends_with('╮'));
    }

    #[test]
    fn test_column_alignment() {
        assert_eq!(align_text("hi", 6, &ColumnAlignment::Left), "hi    ");
        assert_eq!(align_text("hi", 6, &ColumnAlignment::Right), "    hi");
        assert_eq!(align_text("hi", 6, &ColumnAlignment::Center), "  hi  ");
    }

    #[test]
    fn test_center_alignment_odd() {
        // Odd padding: left gets fewer, right gets more.
        assert_eq!(align_text("hi", 5, &ColumnAlignment::Center), " hi  ");
    }

    #[test]
    fn test_text_truncation() {
        assert_eq!(
            align_text("hello world", 5, &ColumnAlignment::Left),
            "hello"
        );
    }

    #[test]
    fn test_header_is_bold() {
        let r = default_renderer();
        let headers = vec!["Name".to_string()];
        let rows = vec![vec!["Alice".to_string()]];
        let result = r.render_table(&headers, &rows, &[], 80);

        // Header row (index 1): should have bold cell segment.
        let header_row = &result[1];
        let cell_seg = header_row.segments.iter().find(|s| s.text.contains("Name"));
        assert!(cell_seg.is_some());
        assert!(cell_seg.unwrap().bold);

        // Data row (index 3): should NOT be bold.
        let data_row = &result[3];
        let cell_seg = data_row.segments.iter().find(|s| s.text.contains("Alice"));
        assert!(cell_seg.is_some());
        assert!(!cell_seg.unwrap().bold);
    }

    #[test]
    fn test_border_color() {
        let color = [100, 200, 50];
        let r = TableRenderer::new(TableStyle::Unicode, color, [205, 214, 244]);
        let headers = vec!["A".to_string()];
        let result = r.render_table(&headers, &[], &[], 80);

        // Top border should use the specified color.
        assert_eq!(result[0].segments[0].fg, Some(color));
    }

    #[test]
    fn test_width_constraint() {
        let r = default_renderer();
        let headers = vec![
            "A very long header".to_string(),
            "Another long one".to_string(),
        ];
        let rows = vec![vec!["short".to_string(), "s".to_string()]];
        let result = r.render_table(&headers, &rows, &[], 30);

        // The table should fit within 30 columns (approximately).
        // Just verify it doesn't panic and produces output.
        assert!(!result.is_empty());
    }

    #[test]
    fn test_missing_cells() {
        let r = default_renderer();
        let headers = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        // Row with fewer cells than headers.
        let rows = vec![vec!["1".to_string()]];
        let result = r.render_table(&headers, &rows, &[], 80);

        // Should not panic and should produce output.
        assert_eq!(result.len(), 5); // top + header + sep + 1 row + bottom
    }
}

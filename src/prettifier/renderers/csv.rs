//! CSV/TSV renderer with tabular display using box-drawing characters.
//!
//! Parses CSV or TSV content and renders it as a formatted table using the
//! shared `TableRenderer` from step 9. Features include:
//!
//! - **Tabular display using box-drawing**: Reuses `TableRenderer`
//! - **Column alignment**: Right-align numeric columns, left-align text columns
//! - **Header row styling**: Bold header row (first row)
//! - **Row striping**: Alternating row background for readability
//! - **Column width auto-sizing**: Based on content width, up to terminal width

use super::table::{ColumnAlignment, TableRenderer, TableStyle};
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::traits::{ContentRenderer, RenderError, RendererConfig};
use crate::prettifier::types::{
    ContentBlock, RenderedContent, RendererCapability, SourceLineMapping, StyledLine,
};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the CSV renderer.
#[derive(Clone, Debug)]
pub struct CsvRendererConfig {
    /// Table border style (default: Unicode).
    pub table_style: TableStyle,
    /// Border color as [r, g, b] (default: dim grey).
    pub border_color: [u8; 3],
    /// Header foreground color as [r, g, b] (default: white).
    pub header_fg: [u8; 3],
    /// Stripe color for alternating rows as [r, g, b] (default: subtle grey).
    pub stripe_color: [u8; 3],
}

impl Default for CsvRendererConfig {
    fn default() -> Self {
        Self {
            table_style: TableStyle::Unicode,
            border_color: [85, 85, 85],
            header_fg: [255, 255, 255],
            stripe_color: [30, 30, 30],
        }
    }
}

// ---------------------------------------------------------------------------
// CSV parsing helpers
// ---------------------------------------------------------------------------

/// Detect whether the content uses comma or tab as delimiter.
fn detect_delimiter(lines: &[String]) -> char {
    let comma_count: usize = lines.iter().take(5).map(|l| l.matches(',').count()).sum();
    let tab_count: usize = lines.iter().take(5).map(|l| l.matches('\t').count()).sum();

    if tab_count > comma_count { '\t' } else { ',' }
}

/// Parse a single CSV line respecting RFC 4180 quoted fields.
fn parse_csv_line(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut chars = line.chars().peekable();
    let mut field = String::new();

    while chars.peek().is_some() {
        // Skip leading whitespace before a field.
        while chars.peek() == Some(&' ') {
            chars.next();
        }

        if chars.peek() == Some(&'"') {
            // Quoted field: consume opening quote.
            chars.next();
            field.clear();
            loop {
                match chars.next() {
                    Some('"') => {
                        if chars.peek() == Some(&'"') {
                            // Escaped quote inside quoted field.
                            field.push('"');
                            chars.next();
                        } else {
                            // End of quoted field.
                            break;
                        }
                    }
                    Some(c) => field.push(c),
                    None => break, // Unterminated quote — best effort.
                }
            }
            fields.push(field.clone());
            // Skip to next delimiter or end.
            while let Some(&c) = chars.peek() {
                chars.next();
                if c == delimiter {
                    break;
                }
            }
        } else {
            // Unquoted field: read until delimiter.
            field.clear();
            loop {
                match chars.peek() {
                    Some(&c) if c == delimiter => {
                        chars.next();
                        break;
                    }
                    Some(_) => field.push(chars.next().unwrap()),
                    None => break,
                }
            }
            fields.push(field.trim().to_string());
        }
    }

    fields
}

/// Parse CSV/TSV lines into rows of fields.
fn parse_csv(lines: &[String], delimiter: char) -> Vec<Vec<String>> {
    lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|line| parse_csv_line(line, delimiter))
        .collect()
}

/// Infer column alignment from data rows: right-align numeric columns.
fn infer_column_alignments(data_rows: &[Vec<String>]) -> Vec<ColumnAlignment> {
    if data_rows.is_empty() {
        return vec![];
    }

    let col_count = data_rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut alignments = vec![ColumnAlignment::Left; col_count];

    for (col_idx, alignment) in alignments.iter_mut().enumerate() {
        let mut numeric_count = 0;
        let mut total_count = 0;

        for row in data_rows {
            if let Some(cell) = row.get(col_idx) {
                let trimmed = cell.trim();
                if !trimmed.is_empty() {
                    total_count += 1;
                    if trimmed.parse::<f64>().is_ok() {
                        numeric_count += 1;
                    }
                }
            }
        }

        // If majority of non-empty cells are numeric, right-align.
        if total_count > 0 && numeric_count * 2 > total_count {
            *alignment = ColumnAlignment::Right;
        }
    }

    alignments
}

/// Apply alternating row background striping to rendered table lines.
fn apply_row_striping(lines: Vec<StyledLine>, stripe_color: [u8; 3]) -> Vec<StyledLine> {
    // Table structure: top_border, header, separator, data_rows..., bottom_border
    // We stripe only data rows (indices 3..len-1, 0-indexed).
    let len = lines.len();
    if len < 5 {
        return lines;
    }

    let mut result = lines;
    let data_start = 3; // First data row
    let data_end = len - 1; // Before bottom border

    let mut stripe = false;
    for line in result.iter_mut().take(data_end).skip(data_start) {
        if stripe {
            for seg in &mut line.segments {
                if seg.bg.is_none() {
                    seg.bg = Some(stripe_color);
                }
            }
        }
        stripe = !stripe;
    }

    result
}

// ---------------------------------------------------------------------------
// CsvRenderer
// ---------------------------------------------------------------------------

/// Renders CSV/TSV content as a formatted table with box-drawing borders.
pub struct CsvRenderer {
    config: CsvRendererConfig,
}

impl CsvRenderer {
    /// Create a new CSV renderer with the given configuration.
    pub fn new(config: CsvRendererConfig) -> Self {
        Self { config }
    }
}

// ---------------------------------------------------------------------------
// ContentRenderer implementation
// ---------------------------------------------------------------------------

impl ContentRenderer for CsvRenderer {
    fn format_id(&self) -> &str {
        "csv"
    }

    fn display_name(&self) -> &str {
        "CSV/TSV"
    }

    fn capabilities(&self) -> Vec<RendererCapability> {
        vec![RendererCapability::TextStyling]
    }

    fn render(
        &self,
        content: &ContentBlock,
        config: &RendererConfig,
    ) -> Result<RenderedContent, RenderError> {
        let delimiter = detect_delimiter(&content.lines);
        let rows = parse_csv(&content.lines, delimiter);

        if rows.is_empty() {
            return Ok(RenderedContent {
                lines: vec![],
                line_mapping: vec![],
                graphics: vec![],
                format_badge: "CSV".to_string(),
            });
        }

        let headers = rows[0].clone();
        let data_rows = if rows.len() > 1 { &rows[1..] } else { &[] };
        let alignments = infer_column_alignments(data_rows);

        let table_renderer = TableRenderer::new(
            self.config.table_style.clone(),
            self.config.border_color,
            self.config.header_fg,
        );
        let styled_lines =
            table_renderer.render_table(&headers, data_rows, &alignments, config.terminal_width);

        let styled_lines = apply_row_striping(styled_lines, self.config.stripe_color);

        let line_mapping: Vec<SourceLineMapping> = styled_lines
            .iter()
            .enumerate()
            .map(|(i, _)| SourceLineMapping {
                rendered_line: i,
                source_line: None,
            })
            .collect();

        Ok(RenderedContent {
            lines: styled_lines,
            line_mapping,
            graphics: vec![],
            format_badge: "CSV".to_string(),
        })
    }

    fn format_badge(&self) -> &str {
        "CSV"
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the CSV renderer with the registry.
pub fn register_csv_renderer(registry: &mut RendererRegistry, config: &CsvRendererConfig) {
    registry.register_renderer("csv", Box::new(CsvRenderer::new(config.clone())));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::traits::{RendererConfig, ThemeColors};
    use crate::prettifier::types::ContentBlock;
    use std::time::SystemTime;

    fn test_config() -> RendererConfig {
        RendererConfig {
            terminal_width: 80,
            theme_colors: ThemeColors::default(),
        }
    }

    fn renderer() -> CsvRenderer {
        CsvRenderer::new(CsvRendererConfig::default())
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

    fn all_text(lines: &[StyledLine]) -> String {
        lines
            .iter()
            .map(|l| {
                l.segments
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn test_format_id() {
        let r = renderer();
        assert_eq!(r.format_id(), "csv");
        assert_eq!(r.display_name(), "CSV/TSV");
        assert_eq!(r.format_badge(), "CSV");
        assert_eq!(r.capabilities(), vec![RendererCapability::TextStyling]);
    }

    #[test]
    fn test_render_csv() {
        let r = renderer();
        let block = make_block(&["name,age,city", "Alice,30,NYC", "Bob,25,London"]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("name"));
        assert!(text.contains("Alice"));
        assert!(text.contains("Bob"));
        assert_eq!(result.format_badge, "CSV");
    }

    #[test]
    fn test_render_tsv() {
        let r = renderer();
        let block = make_block(&["name\tage\tcity", "Alice\t30\tNYC"]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("name"));
        assert!(text.contains("Alice"));
    }

    #[test]
    fn test_numeric_column_alignment() {
        let alignments = infer_column_alignments(&[
            vec!["Alice".to_string(), "30".to_string()],
            vec!["Bob".to_string(), "25".to_string()],
        ]);
        assert_eq!(alignments[0], ColumnAlignment::Left); // text column
        assert_eq!(alignments[1], ColumnAlignment::Right); // numeric column
    }

    #[test]
    fn test_delimiter_detection_comma() {
        let lines = vec!["a,b,c".to_string(), "1,2,3".to_string()];
        assert_eq!(detect_delimiter(&lines), ',');
    }

    #[test]
    fn test_delimiter_detection_tab() {
        let lines = vec!["a\tb\tc".to_string(), "1\t2\t3".to_string()];
        assert_eq!(detect_delimiter(&lines), '\t');
    }

    #[test]
    fn test_box_drawing_borders() {
        let r = renderer();
        let block = make_block(&["name,age", "Alice,30"]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        // Should contain box-drawing characters
        assert!(text.contains('┌'));
        assert!(text.contains('┐'));
        assert!(text.contains('│'));
    }

    #[test]
    fn test_header_is_bold() {
        let r = renderer();
        let block = make_block(&["name,age", "Alice,30"]);
        let result = r.render(&block, &test_config()).unwrap();

        // Header row (index 1 in table output): find bold segment
        let header_row = &result.lines[1];
        let name_seg = header_row.segments.iter().find(|s| s.text.contains("name"));
        assert!(name_seg.is_some());
        assert!(name_seg.unwrap().bold);
    }

    #[test]
    fn test_row_striping() {
        let r = renderer();
        let block = make_block(&["name,age", "Alice,30", "Bob,25", "Charlie,35", "Diana,28"]);
        let result = r.render(&block, &test_config()).unwrap();

        // Data rows start at index 3; even rows (0,2,...) should not have stripe,
        // odd rows (1,3,...) should have stripe background.
        if result.lines.len() >= 6 {
            // Second data row (index 4) should have stripe background.
            let striped_row = &result.lines[4];
            let has_bg = striped_row.segments.iter().any(|s| s.bg.is_some());
            assert!(has_bg);
        }
    }

    #[test]
    fn test_empty_csv() {
        let r = renderer();
        let block = make_block(&[]);
        let result = r.render(&block, &test_config()).unwrap();
        assert!(result.lines.is_empty());
    }

    #[test]
    fn test_single_row_csv() {
        let r = renderer();
        let block = make_block(&["name,age,city"]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("name"));
    }

    #[test]
    fn test_line_mappings_populated() {
        let r = renderer();
        let block = make_block(&["a,b", "1,2"]);
        let result = r.render(&block, &test_config()).unwrap();
        assert_eq!(result.line_mapping.len(), result.lines.len());
    }

    #[test]
    fn test_register_csv_renderer() {
        let mut registry = RendererRegistry::new(0.6);
        register_csv_renderer(&mut registry, &CsvRendererConfig::default());
        assert_eq!(registry.renderer_count(), 1);
        assert!(registry.get_renderer("csv").is_some());
        assert_eq!(
            registry.get_renderer("csv").unwrap().display_name(),
            "CSV/TSV"
        );
    }

    #[test]
    fn test_config_defaults() {
        let config = CsvRendererConfig::default();
        assert_eq!(config.table_style, TableStyle::Unicode);
        assert_eq!(config.border_color, [85, 85, 85]);
        assert_eq!(config.header_fg, [255, 255, 255]);
    }

    #[test]
    fn test_parse_csv_helper() {
        let lines = vec!["name,age,city".to_string(), "Alice,30,NYC".to_string()];
        let rows = parse_csv(&lines, ',');
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec!["name", "age", "city"]);
        assert_eq!(rows[1], vec!["Alice", "30", "NYC"]);
    }

    #[test]
    fn test_infer_alignments_empty() {
        let alignments = infer_column_alignments(&[]);
        assert!(alignments.is_empty());
    }
}

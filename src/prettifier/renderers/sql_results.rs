//! SQL result set renderer with table display using box-drawing characters.
//!
//! Parses SQL result set output (from psql, mysql, etc.) and re-renders it
//! as a clean table using the shared `TableRenderer`. Features include:
//!
//! - **Clean table rendering with box-drawing**: Reuses `TableRenderer`
//! - **NULL value highlighting**: NULL rendered in distinct dimmed italic style
//! - **Numeric column right-alignment**: Auto-detect numeric columns
//! - **Row count footer**: Styled row count summary at the bottom

use std::sync::OnceLock;

use regex::Regex;

use super::table::{ColumnAlignment, TableRenderer, TableStyle};
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::traits::{ContentRenderer, RenderError, RendererConfig};
use crate::prettifier::types::{
    ContentBlock, RenderedContent, RendererCapability, SourceLineMapping, StyledLine, StyledSegment,
};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the SQL results renderer.
#[derive(Clone, Debug)]
pub struct SqlResultsRendererConfig {
    /// Table border style (default: Unicode).
    pub table_style: TableStyle,
    /// Border color as [r, g, b] (default: dim grey).
    pub border_color: [u8; 3],
    /// Header foreground color as [r, g, b] (default: white).
    pub header_fg: [u8; 3],
    /// NULL value color as [r, g, b] (default: dimmed).
    pub null_color: [u8; 3],
}

impl Default for SqlResultsRendererConfig {
    fn default() -> Self {
        Self {
            table_style: TableStyle::Unicode,
            border_color: [108, 112, 134],
            header_fg: [205, 214, 244],
            null_color: [108, 112, 134],
        }
    }
}

// ---------------------------------------------------------------------------
// Regex helpers
// ---------------------------------------------------------------------------

fn re_mysql_border() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\+[-+]+\+$").expect("regex pattern is valid and should always compile")
    })
}

fn re_psql_separator() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^[-+]+$").expect("regex pattern is valid and should always compile")
    })
}

fn re_row_count() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\(?(\d+) rows?\)?")
            .expect("re_row_count: pattern is valid and should always compile")
    })
}

fn re_pipe_row() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\|.*\|$").expect("regex pattern is valid and should always compile")
    })
}

// ---------------------------------------------------------------------------
// SQL result parsing
// ---------------------------------------------------------------------------

/// Detected SQL result format.
#[derive(Debug, PartialEq)]
enum SqlFormat {
    /// MySQL-style: `+---+` borders, `| data |` rows
    Mysql,
    /// PostgreSQL-style: `col | col`, `---+---` separator
    Psql,
}

/// Parsed SQL result set: (headers, data_rows, optional_row_count).
type SqlParseResult = (Vec<String>, Vec<Vec<String>>, Option<String>);

/// Parse SQL result set output into headers, data rows, and optional row count.
fn parse_sql_results(lines: &[String]) -> Option<SqlParseResult> {
    let format = detect_sql_format(lines)?;

    match format {
        SqlFormat::Mysql => parse_mysql_results(lines),
        SqlFormat::Psql => parse_psql_results(lines),
    }
}

fn detect_sql_format(lines: &[String]) -> Option<SqlFormat> {
    for line in lines {
        let trimmed = line.trim();
        if re_mysql_border().is_match(trimmed) {
            return Some(SqlFormat::Mysql);
        }
    }
    for line in lines {
        let trimmed = line.trim();
        if re_psql_separator().is_match(trimmed) {
            return Some(SqlFormat::Psql);
        }
    }
    None
}

fn parse_mysql_results(lines: &[String]) -> Option<SqlParseResult> {
    let mut headers = Vec::new();
    let mut data_rows = Vec::new();
    let mut row_count = None;
    let mut found_header = false;
    let mut after_header_sep = false;

    for line in lines {
        let trimmed = line.trim();

        // Skip border lines
        if re_mysql_border().is_match(trimmed) {
            if found_header {
                after_header_sep = true;
            }
            continue;
        }

        // Check for row count
        if let Some(caps) = re_row_count().captures(trimmed) {
            row_count = Some(caps[0].to_string());
            continue;
        }

        // Parse pipe-delimited row
        if re_pipe_row().is_match(trimmed) {
            let cells: Vec<String> = trimmed
                .trim_matches('|')
                .split('|')
                .map(|c| c.trim().to_string())
                .collect();

            if !found_header {
                headers = cells;
                found_header = true;
            } else if after_header_sep {
                data_rows.push(cells);
            }
        }
    }

    if headers.is_empty() {
        return None;
    }

    Some((headers, data_rows, row_count))
}

fn parse_psql_results(lines: &[String]) -> Option<SqlParseResult> {
    let mut headers = Vec::new();
    let mut data_rows = Vec::new();
    let mut row_count = None;
    let mut found_separator = false;

    for line in lines {
        let trimmed = line.trim();

        // Check for row count footer
        if re_row_count().is_match(trimmed) {
            row_count = Some(trimmed.to_string());
            continue;
        }

        // Separator line
        if re_psql_separator().is_match(trimmed) {
            found_separator = true;
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        // Parse pipe-delimited row
        if trimmed.contains('|') {
            let cells: Vec<String> = trimmed
                .trim_matches('|')
                .split('|')
                .map(|c| c.trim().to_string())
                .collect();

            if !found_separator {
                headers = cells;
            } else {
                data_rows.push(cells);
            }
        }
    }

    if headers.is_empty() {
        return None;
    }

    Some((headers, data_rows, row_count))
}

/// Infer column alignment from data rows.
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
                if !trimmed.is_empty() && trimmed.to_uppercase() != "NULL" {
                    total_count += 1;
                    if trimmed.parse::<f64>().is_ok() {
                        numeric_count += 1;
                    }
                }
            }
        }

        if total_count > 0 && numeric_count * 2 > total_count {
            *alignment = ColumnAlignment::Right;
        }
    }

    alignments
}

// ---------------------------------------------------------------------------
// SqlResultsRenderer
// ---------------------------------------------------------------------------

/// Renders SQL result set output as a clean table with box-drawing borders.
pub struct SqlResultsRenderer {
    config: SqlResultsRendererConfig,
}

impl SqlResultsRenderer {
    /// Create a new SQL results renderer with the given configuration.
    pub fn new(config: SqlResultsRendererConfig) -> Self {
        Self { config }
    }

    /// Apply NULL highlighting to table lines by replacing NULL cell segments.
    fn highlight_nulls(&self, lines: &mut [StyledLine]) {
        for line in lines.iter_mut() {
            for seg in &mut line.segments {
                if seg.text.trim() == "NULL" {
                    seg.fg = Some(self.config.null_color);
                    seg.italic = true;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ContentRenderer implementation
// ---------------------------------------------------------------------------

impl ContentRenderer for SqlResultsRenderer {
    fn format_id(&self) -> &str {
        "sql_results"
    }

    fn display_name(&self) -> &str {
        "SQL Results"
    }

    fn capabilities(&self) -> Vec<RendererCapability> {
        vec![RendererCapability::TextStyling]
    }

    fn render(
        &self,
        content: &ContentBlock,
        config: &RendererConfig,
    ) -> Result<RenderedContent, RenderError> {
        let theme = &config.theme_colors;

        let (headers, data_rows, row_count) =
            parse_sql_results(&content.lines).ok_or_else(|| {
                RenderError::RenderFailed("Could not parse SQL result set".to_string())
            })?;

        let alignments = infer_column_alignments(&data_rows);

        let table_renderer = TableRenderer::new(
            self.config.table_style.clone(),
            self.config.border_color,
            self.config.header_fg,
        );
        let mut styled_lines =
            table_renderer.render_table(&headers, &data_rows, &alignments, config.terminal_width);

        // Highlight NULL values
        self.highlight_nulls(&mut styled_lines);

        // Add row count footer
        if let Some(count_text) = row_count {
            styled_lines.push(StyledLine::new(vec![StyledSegment {
                text: count_text,
                fg: Some(theme.palette[8]), // Dim
                italic: true,
                ..Default::default()
            }]));
        }

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
            format_badge: "SQL".to_string(),
        })
    }

    fn format_badge(&self) -> &str {
        "SQL"
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the SQL results renderer with the registry.
pub fn register_sql_results_renderer(
    registry: &mut RendererRegistry,
    config: &SqlResultsRendererConfig,
) {
    registry.register_renderer(
        "sql_results",
        Box::new(SqlResultsRenderer::new(config.clone())),
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::testing::{make_block, test_renderer_config};
    use crate::prettifier::traits::{RendererConfig, ThemeColors};

    fn test_config() -> RendererConfig {
        test_renderer_config()
    }

    fn renderer() -> SqlResultsRenderer {
        SqlResultsRenderer::new(SqlResultsRendererConfig::default())
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
        assert_eq!(r.format_id(), "sql_results");
        assert_eq!(r.display_name(), "SQL Results");
        assert_eq!(r.format_badge(), "SQL");
        assert_eq!(r.capabilities(), vec![RendererCapability::TextStyling]);
    }

    #[test]
    fn test_render_mysql_results() {
        let r = renderer();
        let block = make_block(&[
            "+----+-------+-----+",
            "| id | name  | age |",
            "+----+-------+-----+",
            "|  1 | Alice |  30 |",
            "|  2 | Bob   |  25 |",
            "+----+-------+-----+",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("id"));
        assert!(text.contains("Alice"));
        assert!(text.contains("Bob"));
        assert_eq!(result.format_badge, "SQL");
    }

    #[test]
    fn test_render_psql_results() {
        let r = renderer();
        let block = make_block(&[
            " id | name  | age",
            "----+-------+----",
            "  1 | Alice |  30",
            "  2 | Bob   |  25",
            "(2 rows)",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("id"));
        assert!(text.contains("Alice"));
        assert!(text.contains("(2 rows)"));
    }

    #[test]
    fn test_null_highlighting() {
        let r = renderer();
        let block = make_block(&[
            "+----+-------+",
            "| id | name  |",
            "+----+-------+",
            "|  1 | NULL  |",
            "+----+-------+",
        ]);
        let result = r.render(&block, &test_config()).unwrap();

        // Find a segment containing "NULL" and check it's styled
        let null_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.trim() == "NULL");
        assert!(null_seg.is_some());
        let null_seg = null_seg.unwrap();
        assert_eq!(null_seg.fg, Some([108, 112, 134]));
        assert!(null_seg.italic);
    }

    #[test]
    fn test_row_count_footer_styled() {
        let r = renderer();
        let block = make_block(&[" id | name", "----+------", "  1 | Alice", "(1 row)"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();

        // Last line should be the row count footer
        let last_line = result.lines.last().unwrap();
        let footer_seg = &last_line.segments[0];
        assert!(footer_seg.text.contains("1 row"));
        assert_eq!(footer_seg.fg, Some(theme.palette[8]));
        assert!(footer_seg.italic);
    }

    #[test]
    fn test_numeric_column_alignment() {
        let alignments = infer_column_alignments(&[
            vec!["1".to_string(), "Alice".to_string()],
            vec!["2".to_string(), "Bob".to_string()],
        ]);
        assert_eq!(alignments[0], ColumnAlignment::Right); // numeric
        assert_eq!(alignments[1], ColumnAlignment::Left); // text
    }

    #[test]
    fn test_null_excluded_from_alignment() {
        let alignments = infer_column_alignments(&[
            vec!["1".to_string(), "NULL".to_string()],
            vec!["2".to_string(), "NULL".to_string()],
        ]);
        // Second column is all NULLs, should default to left
        assert_eq!(alignments[1], ColumnAlignment::Left);
    }

    #[test]
    fn test_invalid_sql_results() {
        let r = renderer();
        let block = make_block(&["just plain text", "no SQL here"]);
        let result = r.render(&block, &test_config());
        assert!(result.is_err());
    }

    #[test]
    fn test_box_drawing_borders() {
        let r = renderer();
        let block = make_block(&[
            "+----+-------+",
            "| id | name  |",
            "+----+-------+",
            "|  1 | Alice |",
            "+----+-------+",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains('┌'));
        assert!(text.contains('│'));
    }

    #[test]
    fn test_line_mappings_populated() {
        let r = renderer();
        let block = make_block(&[
            "+----+-------+",
            "| id | name  |",
            "+----+-------+",
            "|  1 | Alice |",
            "+----+-------+",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        assert_eq!(result.line_mapping.len(), result.lines.len());
    }

    #[test]
    fn test_register_sql_results_renderer() {
        let mut registry = RendererRegistry::new(0.6);
        register_sql_results_renderer(&mut registry, &SqlResultsRendererConfig::default());
        assert_eq!(registry.renderer_count(), 1);
        assert!(registry.get_renderer("sql_results").is_some());
        assert_eq!(
            registry.get_renderer("sql_results").unwrap().display_name(),
            "SQL Results"
        );
    }

    #[test]
    fn test_config_defaults() {
        let config = SqlResultsRendererConfig::default();
        assert_eq!(config.table_style, TableStyle::Unicode);
        assert_eq!(config.border_color, [108, 112, 134]);
        assert_eq!(config.header_fg, [205, 214, 244]);
        assert_eq!(config.null_color, [108, 112, 134]);
    }

    #[test]
    fn test_detect_mysql_format() {
        let lines = vec![
            "+----+".to_string(),
            "| id |".to_string(),
            "+----+".to_string(),
        ];
        assert_eq!(detect_sql_format(&lines), Some(SqlFormat::Mysql));
    }

    #[test]
    fn test_detect_psql_format() {
        let lines = vec!["id | name".to_string(), "---+-----".to_string()];
        assert_eq!(detect_sql_format(&lines), Some(SqlFormat::Psql));
    }
}

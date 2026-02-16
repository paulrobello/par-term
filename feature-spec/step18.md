# Step 18: XML, CSV/TSV & SQL Results Renderers

## Summary

Implement detectors and renderers for XML/HTML, CSV/TSV, and SQL result sets. XML uses the shared tree renderer for collapsible element hierarchy. CSV/TSV and SQL results use the shared table renderer (from Step 9) for formatted tabular display with box-drawing characters.

## Dependencies

- **Step 1**: Core traits and types
- **Step 2**: `RegexDetector`
- **Step 4**: `RendererRegistry`
- **Step 9**: Shared `TableRenderer` (for CSV/TSV and SQL)
- **Step 14**: Shared `tree_renderer` (for XML)

## What to Implement

### XML Detector & Renderer

#### New File: `src/prettifier/detectors/xml.rs`

```rust
pub fn create_xml_detector() -> RegexDetector {
    RegexDetector::builder("xml", "XML/HTML")
        .confidence_threshold(0.6)
        .min_matching_rules(1)
        .definitive_shortcircuit(true)
        .add_rule(DetectionRule {
            id: "xml_declaration".into(),
            pattern: Regex::new(r"^<\?xml\s+").unwrap(),
            weight: 0.9,
            scope: RuleScope::FirstLines(3),
            strength: RuleStrength::Definitive,
            description: "XML declaration (<?xml ...)".into(),
            ..
        })
        .add_rule(DetectionRule {
            id: "xml_doctype".into(),
            pattern: Regex::new(r"^<!DOCTYPE\s+").unwrap(),
            weight: 0.8,
            scope: RuleScope::FirstLines(5),
            strength: RuleStrength::Definitive,
            description: "DOCTYPE declaration".into(),
            ..
        })
        .add_rule(DetectionRule {
            id: "xml_opening_tag".into(),
            pattern: Regex::new(r"^\s*<[a-zA-Z][\w:-]*(\s+[\w:-]+=)?").unwrap(),
            weight: 0.3,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            description: "XML/HTML opening tag with optional attributes".into(),
            ..
        })
        .add_rule(DetectionRule {
            id: "xml_closing_tag".into(),
            pattern: Regex::new(r"^\s*</[a-zA-Z][\w:-]*>").unwrap(),
            weight: 0.2,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            description: "XML/HTML closing tag".into(),
            ..
        })
        .add_rule(DetectionRule {
            id: "xml_self_closing".into(),
            pattern: Regex::new(r"/>\s*$").unwrap(),
            weight: 0.15,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            description: "Self-closing tag".into(),
            ..
        })
        .build()
}
```

#### New File: `src/prettifier/renderers/xml.rs`

```rust
pub struct XmlRenderer {
    config: XmlRendererConfig,
}

impl ContentRenderer for XmlRenderer {
    fn format_id(&self) -> &str { "xml" }
    fn display_name(&self) -> &str { "XML/HTML" }
    // ...
}
```

**XML rendering features** (from spec lines 1097–1106):

1. **Tag hierarchy with indentation and guide lines**: Use shared tree renderer for `│` guides at each nesting level
2. **Attribute highlighting**: Tag names in one color, attribute names in another, attribute values in a third
3. **Collapsible elements**: Click a tag to collapse its children (show `<tag>...</tag>`)
4. **Namespace coloring**: XML namespace prefixes in a distinct color
5. **CDATA/comment distinction**: CDATA sections and comments styled differently from regular content

```rust
fn style_xml_tag(tag: &str, theme: &ThemeColors) -> Vec<StyledSegment> {
    // Parse: <namespace:tagname attr1="value1" attr2="value2">
    // Color: < > brackets: dim
    // Tag name: bold color
    // Namespace prefix: distinct color
    // Attribute names: secondary color
    // Attribute values: string color (quoted)
    ...
}
```

### CSV/TSV Detector & Renderer

#### New File: `src/prettifier/detectors/csv.rs`

```rust
pub fn create_csv_detector() -> RegexDetector {
    RegexDetector::builder("csv", "CSV/TSV")
        .confidence_threshold(0.6)
        .min_matching_rules(2)
        .definitive_shortcircuit(false)
        // Consistent comma-separated fields across multiple lines
        .add_rule(DetectionRule {
            id: "csv_comma_consistent".into(),
            pattern: Regex::new(r"^[^,]+,[^,]+,").unwrap(),
            weight: 0.3,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            description: "Multiple comma-separated fields".into(),
            ..
        })
        // TSV: consistent tab-separated fields
        .add_rule(DetectionRule {
            id: "csv_tab_consistent".into(),
            pattern: Regex::new(r"^[^\t]+\t[^\t]+\t").unwrap(),
            weight: 0.3,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            description: "Multiple tab-separated fields".into(),
            ..
        })
        // Header-like first line (word,word,word pattern)
        .add_rule(DetectionRule {
            id: "csv_header_row".into(),
            pattern: Regex::new(r"^[a-zA-Z_]\w*(,[a-zA-Z_]\w*)+\s*$").unwrap(),
            weight: 0.4,
            scope: RuleScope::FirstLines(1),
            strength: RuleStrength::Strong,
            description: "Header row with word-like column names".into(),
            ..
        })
        // Command context
        .add_rule(DetectionRule {
            id: "csv_command_context".into(),
            pattern: Regex::new(r"(csvtool|csvkit|cut|awk)").unwrap(),
            weight: 0.2,
            scope: RuleScope::PrecedingCommand,
            strength: RuleStrength::Supporting,
            ..
        })
        .build()
}
```

**CSV detection challenge**: CSV is hard to distinguish from other comma-separated text. The detector should verify that the field count is consistent across lines before triggering detection. This requires a custom validation step after regex matching:

```rust
/// Additional validation: check that field count is consistent across lines.
fn validate_csv_consistency(content: &ContentBlock, delimiter: char) -> bool {
    let field_counts: Vec<usize> = content.lines.iter()
        .take(10)
        .map(|line| line.split(delimiter).count())
        .collect();

    if field_counts.is_empty() || field_counts[0] < 2 { return false; }
    field_counts.windows(2).all(|w| w[0] == w[1])
}
```

#### New File: `src/prettifier/renderers/csv.rs`

```rust
pub struct CsvRenderer {
    config: CsvRendererConfig,
}
```

**CSV rendering features** (from spec lines 1108–1117):

1. **Tabular display using box-drawing** — reuse `TableRenderer` from Step 9
2. **Column alignment**: Right-align numeric columns, left-align text columns
3. **Header row styling**: Bold header row (first row)
4. **Row striping**: Alternating row background for readability
5. **Column width auto-sizing**: Based on content width, up to terminal width

```rust
impl ContentRenderer for CsvRenderer {
    fn render(&self, content: &ContentBlock, config: &RendererConfig) -> Result<RenderedContent, RenderError> {
        let delimiter = detect_delimiter(&content.lines)?; // ',' or '\t'
        let rows = parse_csv(&content.lines, delimiter);
        let headers = rows.first().cloned().unwrap_or_default();
        let data_rows = &rows[1..];

        let alignments = infer_column_alignments(data_rows);

        let table_renderer = TableRenderer::new(self.config.table_style, self.config.border_color);
        let styled_lines = table_renderer.render_table(&headers, data_rows, &alignments, config.terminal_width);

        // Add row striping
        let styled_lines = apply_row_striping(styled_lines, self.config.stripe_color);

        Ok(RenderedContent {
            styled_lines,
            line_mapping: build_csv_line_mapping(&content.lines, &styled_lines),
            graphics: vec![],
            format_badge: "\u{1F4C9}".to_string(), // 📉
        })
    }
}
```

### SQL Results Detector & Renderer

#### New File: `src/prettifier/detectors/sql_results.rs`

```rust
pub fn create_sql_results_detector() -> RegexDetector {
    RegexDetector::builder("sql_results", "SQL Results")
        .confidence_threshold(0.6)
        .min_matching_rules(2)
        .definitive_shortcircuit(true)
        // psql-style table header
        .add_rule(DetectionRule {
            id: "sql_psql_separator".into(),
            pattern: Regex::new(r"^[-+]+$").unwrap(),
            weight: 0.4,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            ..
        })
        // mysql-style table border
        .add_rule(DetectionRule {
            id: "sql_mysql_border".into(),
            pattern: Regex::new(r"^\+[-+]+\+$").unwrap(),
            weight: 0.6,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            ..
        })
        // Row count footer (e.g., "(5 rows)", "5 rows in set")
        .add_rule(DetectionRule {
            id: "sql_row_count".into(),
            pattern: Regex::new(r"^\(?(\d+) rows?\)?").unwrap(),
            weight: 0.3,
            scope: RuleScope::LastLines(3),
            strength: RuleStrength::Supporting,
            ..
        })
        // Command context: sql tools
        .add_rule(DetectionRule {
            id: "sql_command_context".into(),
            pattern: Regex::new(r"(psql|mysql|sqlite3|pgcli|mycli)").unwrap(),
            weight: 0.3,
            scope: RuleScope::PrecedingCommand,
            strength: RuleStrength::Supporting,
            ..
        })
        .build()
}
```

#### New File: `src/prettifier/renderers/sql_results.rs`

```rust
pub struct SqlResultsRenderer {
    config: SqlResultsRendererConfig,
}
```

**SQL results rendering features** (from spec lines 1156–1162):

1. **Clean table rendering with box-drawing**: Reuse `TableRenderer`
2. **NULL value highlighting**: Render NULL in distinct style (dimmed, italic, or colored)
3. **Numeric column right-alignment**: Auto-detect numeric columns
4. **Row count footer**: Style the row count summary at the bottom

## Key Files

| Action | Path |
|--------|------|
| Create | `src/prettifier/detectors/xml.rs` |
| Create | `src/prettifier/detectors/csv.rs` |
| Create | `src/prettifier/detectors/sql_results.rs` |
| Create | `src/prettifier/renderers/xml.rs` |
| Create | `src/prettifier/renderers/csv.rs` |
| Create | `src/prettifier/renderers/sql_results.rs` |
| Modify | `src/prettifier/detectors/mod.rs` (add modules) |
| Modify | `src/prettifier/renderers/mod.rs` (add modules) |

## Relevant Spec Sections

- **Lines 1097–1106**: XML renderer features
- **Lines 1108–1117**: CSV/TSV renderer features
- **Lines 1156–1162**: SQL result set renderer features
- **Lines 1342**: Phase 2d — CSV/TSV and SQL result set rendering share table infrastructure
- **Lines 1360**: Shared table renderer infrastructure (box-drawing, column alignment, header styling)
- **Lines 1474**: Acceptance criteria — CSV/TSV rendered as formatted tables

## Verification Criteria

- [ ] `cargo build` succeeds
- [ ] XML with `<?xml` declaration is definitively detected
- [ ] XML tags render with colored tag names and attribute highlighting
- [ ] XML elements are collapsible
- [ ] Namespace prefixes have distinct coloring
- [ ] CSV with consistent comma-separated fields is detected
- [ ] TSV with tab-separated fields is detected
- [ ] CSV/TSV renders as formatted table with box-drawing borders
- [ ] Header row is visually distinct (bold)
- [ ] Numeric columns are right-aligned
- [ ] Row striping alternates background colors
- [ ] SQL results from psql/mysql format are detected
- [ ] SQL NULL values are highlighted distinctly
- [ ] SQL row count footer is styled
- [ ] All three renderers reuse shared `TableRenderer` / `tree_renderer`
- [ ] Unit tests for each detector and renderer

//! TOML renderer with syntax highlighting, section headers, key-value alignment,
//! and tree guide indentation.
//!
//! Uses a line-by-line classifier to parse TOML content without a full parser.
//! Features include:
//!
//! - **Section headers**: `[section]` rendered prominently with bold styling
//! - **Array table headers**: `[[array]]` styled distinctly from regular sections
//! - **Key-value alignment**: `=` signs aligned within sections for readability
//! - **Type-aware value coloring**: strings, integers, floats, booleans, dates
//! - **Comment dimming**: comments styled as dimmed italic text
//! - **Tree guides**: indentation guides for nested section hierarchy

use std::sync::OnceLock;

use regex::Regex;

use super::{push_line, tree_renderer};
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::traits::{ContentRenderer, RenderError, RendererConfig, ThemeColors};
use crate::prettifier::types::{ContentBlock, RenderedContent, RendererCapability, StyledSegment};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the TOML renderer.
#[derive(Clone, Debug)]
pub struct TomlRendererConfig {
    /// Align `=` signs within sections (default: true).
    pub align_equals: bool,
    /// Auto-collapse sections beyond this depth (default: 4).
    pub max_depth_expanded: usize,
}

impl Default for TomlRendererConfig {
    fn default() -> Self {
        Self {
            align_equals: true,
            max_depth_expanded: 4,
        }
    }
}

// ---------------------------------------------------------------------------
// TOML line classification
// ---------------------------------------------------------------------------

/// Classification of a single TOML line.
#[derive(Debug)]
enum TomlLineType {
    /// `[section]` header.
    SectionHeader { name: String, depth: usize },
    /// `[[array]]` table header.
    ArrayTable { name: String, depth: usize },
    /// Comment line.
    Comment(String),
    /// Key-value pair.
    KeyValue { key: String, value: String },
    /// Empty line.
    Empty,
    /// Other line (continuation, inline table, etc.).
    Other(String),
}

fn re_section_header() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\[([\w.-]+)\]\s*$").unwrap())
}

fn re_array_table() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\[\[([\w.-]+)\]\]\s*$").unwrap())
}

fn re_key_value() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([\w.-]+)\s*=\s*(.*)$").unwrap())
}

fn classify_toml_line(line: &str) -> TomlLineType {
    let trimmed = line.trim();

    if trimmed.is_empty() {
        return TomlLineType::Empty;
    }

    if trimmed.starts_with('#') {
        return TomlLineType::Comment(trimmed.to_string());
    }

    // Array table `[[name]]` — must check before section header
    if let Some(caps) = re_array_table().captures(trimmed) {
        let name = caps.get(1).map_or("", |m| m.as_str()).to_string();
        let depth = name.matches('.').count();
        return TomlLineType::ArrayTable { name, depth };
    }

    // Section header `[name]`
    if let Some(caps) = re_section_header().captures(trimmed) {
        let name = caps.get(1).map_or("", |m| m.as_str()).to_string();
        let depth = name.matches('.').count();
        return TomlLineType::SectionHeader { name, depth };
    }

    // Key-value pair
    if let Some(caps) = re_key_value().captures(trimmed) {
        let key = caps.get(1).map_or("", |m| m.as_str()).to_string();
        let value = caps.get(2).map_or("", |m| m.as_str()).trim().to_string();
        return TomlLineType::KeyValue { key, value };
    }

    TomlLineType::Other(trimmed.to_string())
}

/// Compute the column to align `=` signs to for a section's key-value pairs.
fn compute_align_column(lines: &[String], start: usize) -> usize {
    let mut max_key_len = 0;
    for line in lines.iter().skip(start) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Stop at next section header
        if trimmed.starts_with('[') {
            break;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(caps) = re_key_value().captures(trimmed) {
            let key_len = caps.get(1).map_or(0, |m| m.as_str().len());
            max_key_len = max_key_len.max(key_len);
        }
    }
    max_key_len
}

// ---------------------------------------------------------------------------
// TomlRenderer
// ---------------------------------------------------------------------------

/// Renders TOML content with syntax highlighting and aligned key-value pairs.
pub struct TomlRenderer {
    config: TomlRendererConfig,
}

impl TomlRenderer {
    /// Create a new TOML renderer with the given configuration.
    pub fn new(config: TomlRendererConfig) -> Self {
        Self { config }
    }

    /// Style a TOML value with type-aware coloring.
    fn style_value(&self, value: &str, theme: &ThemeColors) -> Vec<StyledSegment> {
        if value.is_empty() {
            return vec![plain_segment("")];
        }

        let trimmed = value.trim();

        // Boolean
        if trimmed == "true" || trimmed == "false" {
            return vec![StyledSegment {
                text: trimmed.to_string(),
                fg: Some(theme.palette[5]), // Magenta
                ..Default::default()
            }];
        }

        // String (double-quoted)
        if trimmed.starts_with('"') && trimmed.ends_with('"') {
            return vec![StyledSegment {
                text: trimmed.to_string(),
                fg: Some(theme.palette[2]), // Green
                ..Default::default()
            }];
        }

        // String (single-quoted / literal)
        if trimmed.starts_with('\'') && trimmed.ends_with('\'') {
            return vec![StyledSegment {
                text: trimmed.to_string(),
                fg: Some(theme.palette[2]), // Green
                ..Default::default()
            }];
        }

        // Multi-line string start
        if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
            return vec![StyledSegment {
                text: trimmed.to_string(),
                fg: Some(theme.palette[2]),
                ..Default::default()
            }];
        }

        // Array
        if trimmed.starts_with('[') {
            return vec![StyledSegment {
                text: trimmed.to_string(),
                fg: Some(theme.palette[3]), // Yellow
                ..Default::default()
            }];
        }

        // Inline table
        if trimmed.starts_with('{') {
            return vec![StyledSegment {
                text: trimmed.to_string(),
                fg: Some(theme.palette[3]),
                ..Default::default()
            }];
        }

        // Date/time (ISO 8601 patterns)
        if is_toml_datetime(trimmed) {
            return vec![StyledSegment {
                text: trimmed.to_string(),
                fg: Some(theme.palette[14]), // Bright cyan
                ..Default::default()
            }];
        }

        // Number (integer or float, including hex/oct/bin)
        if is_toml_number(trimmed) {
            return vec![StyledSegment {
                text: trimmed.to_string(),
                fg: Some(theme.palette[11]), // Bright yellow
                ..Default::default()
            }];
        }

        // Fallback — unquoted value
        vec![StyledSegment {
            text: trimmed.to_string(),
            fg: Some(theme.palette[2]),
            ..Default::default()
        }]
    }
}

/// Check if a value looks like a TOML datetime.
fn is_toml_datetime(s: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| Regex::new(r"^\d{4}-\d{2}-\d{2}([T ]\d{2}:\d{2}(:\d{2})?)?").unwrap());
    re.is_match(s)
}

/// Check if a value looks like a TOML number.
fn is_toml_number(s: &str) -> bool {
    // Allow underscores in numbers
    let cleaned: String = s.chars().filter(|c| *c != '_').collect();
    // Hex, octal, binary prefixes
    if cleaned.starts_with("0x") || cleaned.starts_with("0o") || cleaned.starts_with("0b") {
        return cleaned.len() > 2;
    }
    // Special float values
    if cleaned == "inf"
        || cleaned == "+inf"
        || cleaned == "-inf"
        || cleaned == "nan"
        || cleaned == "+nan"
        || cleaned == "-nan"
    {
        return true;
    }
    cleaned.parse::<f64>().is_ok()
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn guide_segment(prefix: &str, theme: &ThemeColors) -> StyledSegment {
    StyledSegment {
        text: prefix.to_string(),
        fg: Some(theme.palette[8]),
        ..Default::default()
    }
}

fn plain_segment(text: &str) -> StyledSegment {
    StyledSegment {
        text: text.to_string(),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// ContentRenderer implementation
// ---------------------------------------------------------------------------

impl ContentRenderer for TomlRenderer {
    fn format_id(&self) -> &str {
        "toml"
    }

    fn display_name(&self) -> &str {
        "TOML"
    }

    fn capabilities(&self) -> Vec<RendererCapability> {
        vec![RendererCapability::TextStyling]
    }

    fn render(
        &self,
        content: &ContentBlock,
        config: &RendererConfig,
    ) -> Result<RenderedContent, RenderError> {
        let _ = config;
        let theme = &config.theme_colors;
        let mut lines = Vec::new();
        let mut line_mapping = Vec::new();
        let mut current_depth: usize = 0;
        let mut section_start: Option<usize> = None;
        let mut align_col: usize = 0;

        for (line_idx, line) in content.lines.iter().enumerate() {
            let classified = classify_toml_line(line);

            match classified {
                TomlLineType::SectionHeader { name, depth } => {
                    current_depth = depth;
                    let prefix = tree_renderer::tree_guides(current_depth);

                    if current_depth >= self.config.max_depth_expanded {
                        let child_count = count_section_keys(&content.lines, line_idx + 1);
                        let summary = tree_renderer::collapsed_summary("object", child_count);
                        push_line(
                            &mut lines,
                            &mut line_mapping,
                            vec![
                                guide_segment(&prefix, theme),
                                StyledSegment {
                                    text: format!("[{name}]"),
                                    fg: Some(theme.palette[4]), // Blue
                                    bold: true,
                                    ..Default::default()
                                },
                                StyledSegment {
                                    text: format!(" {summary}"),
                                    fg: Some(theme.palette[8]),
                                    italic: true,
                                    ..Default::default()
                                },
                            ],
                            Some(line_idx),
                        );
                    } else {
                        push_line(
                            &mut lines,
                            &mut line_mapping,
                            vec![
                                guide_segment(&prefix, theme),
                                StyledSegment {
                                    text: format!("[{name}]"),
                                    fg: Some(theme.palette[4]), // Blue
                                    bold: true,
                                    ..Default::default()
                                },
                            ],
                            Some(line_idx),
                        );
                    }

                    // Compute alignment for this section's key-value pairs
                    section_start = Some(line_idx + 1);
                    if self.config.align_equals {
                        align_col = compute_align_column(&content.lines, line_idx + 1);
                    }
                }
                TomlLineType::ArrayTable { name, depth } => {
                    current_depth = depth;
                    let prefix = tree_renderer::tree_guides(current_depth);
                    push_line(
                        &mut lines,
                        &mut line_mapping,
                        vec![
                            guide_segment(&prefix, theme),
                            StyledSegment {
                                text: format!("[[{name}]]"),
                                fg: Some(theme.palette[12]), // Bright blue
                                bold: true,
                                ..Default::default()
                            },
                        ],
                        Some(line_idx),
                    );

                    section_start = Some(line_idx + 1);
                    if self.config.align_equals {
                        align_col = compute_align_column(&content.lines, line_idx + 1);
                    }
                }
                TomlLineType::Comment(text) => {
                    let prefix = tree_renderer::tree_guides(current_depth);
                    push_line(
                        &mut lines,
                        &mut line_mapping,
                        vec![
                            guide_segment(&prefix, theme),
                            StyledSegment {
                                text,
                                fg: Some(theme.palette[8]),
                                italic: true,
                                ..Default::default()
                            },
                        ],
                        Some(line_idx),
                    );
                }
                TomlLineType::KeyValue { key, value } => {
                    // If we haven't seen a section header yet, compute alignment
                    // for the top-level keys.
                    if section_start.is_none() && self.config.align_equals {
                        section_start = Some(0);
                        align_col = compute_align_column(&content.lines, 0);
                    }

                    let prefix = tree_renderer::tree_guides(current_depth);
                    let padding = if self.config.align_equals && align_col > key.len() {
                        " ".repeat(align_col - key.len())
                    } else {
                        String::new()
                    };

                    let mut segments = vec![
                        guide_segment(&prefix, theme),
                        StyledSegment {
                            text: key,
                            fg: Some(theme.palette[6]), // Cyan
                            ..Default::default()
                        },
                        plain_segment(&format!("{padding} = ")),
                    ];
                    segments.extend(self.style_value(&value, theme));
                    push_line(&mut lines, &mut line_mapping, segments, Some(line_idx));
                }
                TomlLineType::Empty => {
                    push_line(
                        &mut lines,
                        &mut line_mapping,
                        vec![plain_segment("")],
                        Some(line_idx),
                    );
                }
                TomlLineType::Other(text) => {
                    let prefix = tree_renderer::tree_guides(current_depth);
                    push_line(
                        &mut lines,
                        &mut line_mapping,
                        vec![guide_segment(&prefix, theme), plain_segment(&text)],
                        Some(line_idx),
                    );
                }
            }
        }

        Ok(RenderedContent {
            lines,
            line_mapping,
            graphics: vec![],
            format_badge: "TOML".to_string(),
        })
    }

    fn format_badge(&self) -> &str {
        "TOML"
    }
}

/// Count key-value pairs in a section (until next section header or end).
fn count_section_keys(lines: &[String], start: usize) -> usize {
    let mut count = 0;
    for line in lines.iter().skip(start) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('[') {
            break;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        if re_key_value().is_match(trimmed) {
            count += 1;
        }
    }
    count
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the TOML renderer with the registry.
pub fn register_toml_renderer(registry: &mut RendererRegistry, config: &TomlRendererConfig) {
    registry.register_renderer("toml", Box::new(TomlRenderer::new(config.clone())));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::traits::RendererConfig;
    use crate::prettifier::types::{ContentBlock, StyledLine};
    use std::time::SystemTime;

    fn test_config() -> RendererConfig {
        RendererConfig {
            terminal_width: 80,
            ..Default::default()
        }
    }

    fn renderer() -> TomlRenderer {
        TomlRenderer::new(TomlRendererConfig::default())
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
        assert_eq!(r.format_id(), "toml");
        assert_eq!(r.display_name(), "TOML");
        assert_eq!(r.format_badge(), "TOML");
        assert_eq!(r.capabilities(), vec![RendererCapability::TextStyling]);
    }

    #[test]
    fn test_render_section_header() {
        let r = renderer();
        let block = make_block(&["[package]", "name = \"par-term\""]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("[package]"));
        assert!(text.contains("name"));
        assert!(text.contains("\"par-term\""));
        assert_eq!(result.format_badge, "TOML");
    }

    #[test]
    fn test_section_header_bold_blue() {
        let r = renderer();
        let block = make_block(&["[package]"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();
        let header_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains("[package]"))
            .unwrap();
        assert_eq!(header_seg.fg, Some(theme.palette[4]));
        assert!(header_seg.bold);
    }

    #[test]
    fn test_render_array_table() {
        let r = renderer();
        let block = make_block(&["[[bin]]", "name = \"par-term\""]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("[[bin]]"));
    }

    #[test]
    fn test_array_table_bright_blue() {
        let r = renderer();
        let block = make_block(&["[[bin]]"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();
        let header_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains("[[bin]]"))
            .unwrap();
        assert_eq!(header_seg.fg, Some(theme.palette[12]));
        assert!(header_seg.bold);
    }

    #[test]
    fn test_render_comment() {
        let r = renderer();
        let block = make_block(&["# This is a comment"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();
        let comment_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains("# This is a comment"))
            .unwrap();
        assert_eq!(comment_seg.fg, Some(theme.palette[8]));
        assert!(comment_seg.italic);
    }

    #[test]
    fn test_string_value_coloring() {
        let r = renderer();
        let block = make_block(&["name = \"par-term\""]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();
        let str_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains("\"par-term\""))
            .unwrap();
        assert_eq!(str_seg.fg, Some(theme.palette[2]));
    }

    #[test]
    fn test_number_value_coloring() {
        let r = renderer();
        let block = make_block(&["port = 8080"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();
        let num_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text == "8080")
            .unwrap();
        assert_eq!(num_seg.fg, Some(theme.palette[11]));
    }

    #[test]
    fn test_boolean_value_coloring() {
        let r = renderer();
        let block = make_block(&["enabled = true"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();
        let bool_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text == "true")
            .unwrap();
        assert_eq!(bool_seg.fg, Some(theme.palette[5]));
    }

    #[test]
    fn test_datetime_value_coloring() {
        let r = renderer();
        let block = make_block(&["created = 2024-01-15T10:30:00"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();
        let date_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains("2024-01-15"))
            .unwrap();
        assert_eq!(date_seg.fg, Some(theme.palette[14]));
    }

    #[test]
    fn test_key_coloring() {
        let r = renderer();
        let block = make_block(&["mykey = \"value\""]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();
        let key_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text == "mykey")
            .unwrap();
        assert_eq!(key_seg.fg, Some(theme.palette[6]));
    }

    #[test]
    fn test_key_value_alignment() {
        let r = renderer();
        let block = make_block(&[
            "[package]",
            "name = \"par-term\"",
            "version = \"0.16.0\"",
            "edition = \"2024\"",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        // "edition" is 7 chars, "name" is 4 chars — with alignment, name should
        // have extra padding before the `=`
        assert!(text.contains("name"));
        assert!(text.contains("version"));
        assert!(text.contains("edition"));
    }

    #[test]
    fn test_nested_section_depth() {
        let r = renderer();
        let block = make_block(&[
            "[server]",
            "host = \"localhost\"",
            "",
            "[server.tls]",
            "enabled = true",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        // Nested section should have tree guides
        assert!(text.contains("[server.tls]"));
    }

    #[test]
    fn test_line_mappings_populated() {
        let r = renderer();
        let block = make_block(&["[package]", "name = \"test\""]);
        let result = r.render(&block, &test_config()).unwrap();
        assert_eq!(result.line_mapping.len(), result.lines.len());
    }

    #[test]
    fn test_auto_collapse_deep_section() {
        let r = TomlRenderer::new(TomlRendererConfig {
            max_depth_expanded: 0,
            ..Default::default()
        });
        let block = make_block(&["[package]", "name = \"test\"", "version = \"1.0\""]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("keys"));
    }

    #[test]
    fn test_empty_lines_preserved() {
        let r = renderer();
        let block = make_block(&["[a]", "x = 1", "", "[b]", "y = 2"]);
        let result = r.render(&block, &test_config()).unwrap();
        assert_eq!(result.lines.len(), 5);
    }

    #[test]
    fn test_register_toml_renderer() {
        let mut registry = RendererRegistry::new(0.6);
        register_toml_renderer(&mut registry, &TomlRendererConfig::default());
        assert_eq!(registry.renderer_count(), 1);
        assert!(registry.get_renderer("toml").is_some());
        assert_eq!(
            registry.get_renderer("toml").unwrap().display_name(),
            "TOML"
        );
    }

    #[test]
    fn test_config_defaults() {
        let config = TomlRendererConfig::default();
        assert!(config.align_equals);
        assert_eq!(config.max_depth_expanded, 4);
    }

    #[test]
    fn test_is_toml_number() {
        assert!(is_toml_number("42"));
        assert!(is_toml_number("3.14"));
        assert!(is_toml_number("-17"));
        assert!(is_toml_number("1_000"));
        assert!(is_toml_number("0xFF"));
        assert!(is_toml_number("0o77"));
        assert!(is_toml_number("0b1010"));
        assert!(is_toml_number("inf"));
        assert!(is_toml_number("+inf"));
        assert!(is_toml_number("nan"));
        assert!(!is_toml_number("hello"));
    }

    #[test]
    fn test_is_toml_datetime() {
        assert!(is_toml_datetime("2024-01-15"));
        assert!(is_toml_datetime("2024-01-15T10:30:00"));
        assert!(is_toml_datetime("2024-01-15 10:30"));
        assert!(!is_toml_datetime("hello"));
        assert!(!is_toml_datetime("42"));
    }

    #[test]
    fn test_array_value() {
        let r = renderer();
        let block = make_block(&["tags = [\"rust\", \"terminal\"]"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();
        let arr_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains('['))
            .unwrap();
        assert_eq!(arr_seg.fg, Some(theme.palette[3]));
    }

    #[test]
    fn test_inline_table_value() {
        let r = renderer();
        let block = make_block(&["point = { x = 1, y = 2 }"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();
        let table_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains('{'))
            .unwrap();
        assert_eq!(table_seg.fg, Some(theme.palette[3]));
    }

    #[test]
    fn test_no_alignment_when_disabled() {
        let r = TomlRenderer::new(TomlRendererConfig {
            align_equals: false,
            ..Default::default()
        });
        let block = make_block(&["a = 1", "longkey = 2"]);
        let result = r.render(&block, &test_config()).unwrap();
        // Without alignment, "a" should not have extra padding
        let first_line_text: String = result.lines[0]
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(first_line_text.contains("a = "));
    }
}

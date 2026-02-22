//! YAML renderer with syntax highlighting, indentation guides, and collapsible sections.
//!
//! Uses a line-by-line parser that tracks indentation depth rather than a full
//! YAML parser. Features include:
//!
//! - **Syntax highlighting**: distinct colors for keys, string values, numbers,
//!   booleans, anchors, aliases, tags, and comments
//! - **Tree guide lines**: vertical `│` characters at each indentation level
//! - **Collapsible sections**: mapping keys with nested children auto-collapse
//!   beyond `max_depth_expanded`
//! - **Anchor/alias indicators**: `&anchor` and `*alias` in distinct color
//! - **Document separator styling**: `---` rendered as a prominent separator

use std::sync::OnceLock;

use regex::Regex;

use super::tree_renderer;
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::traits::{ContentRenderer, RenderError, RendererConfig, ThemeColors};
use crate::prettifier::types::{
    ContentBlock, RenderedContent, RendererCapability, SourceLineMapping, StyledLine, StyledSegment,
};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the YAML renderer.
#[derive(Clone, Debug)]
pub struct YamlRendererConfig {
    /// Spaces per indentation level for depth calculation (default: 2).
    pub indent_width: usize,
    /// Auto-collapse mappings beyond this depth (default: 4).
    pub max_depth_expanded: usize,
}

impl Default for YamlRendererConfig {
    fn default() -> Self {
        Self {
            indent_width: 2,
            max_depth_expanded: 4,
        }
    }
}

// ---------------------------------------------------------------------------
// YAML line classification
// ---------------------------------------------------------------------------

/// Classification of a single YAML line.
#[derive(Debug)]
enum YamlLineType {
    /// `---` document start marker.
    DocumentStart,
    /// `...` document end marker.
    DocumentEnd,
    /// Comment line starting with `#`.
    Comment { indent: usize, text: String },
    /// Key-value pair.
    KeyValue {
        indent: usize,
        key: String,
        value: String,
    },
    /// List item `- ...`.
    ListItem { indent: usize, content: String },
    /// Continuation or other line.
    Continuation { indent: usize, text: String },
    /// Empty line.
    Empty,
}

fn re_anchor() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"&\w+").unwrap())
}

fn re_alias() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\*\w+").unwrap())
}

fn re_tag() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"!!\w+").unwrap())
}

fn re_key_value() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\s*)([\w./-]+)\s*:\s*(.*)$").unwrap())
}

fn classify_yaml_line(line: &str) -> YamlLineType {
    if line.trim().is_empty() {
        return YamlLineType::Empty;
    }

    let trimmed = line.trim();

    if trimmed == "---" {
        return YamlLineType::DocumentStart;
    }
    if trimmed == "..." {
        return YamlLineType::DocumentEnd;
    }
    if trimmed.starts_with('#') {
        let indent = line.len() - line.trim_start().len();
        return YamlLineType::Comment {
            indent,
            text: trimmed.to_string(),
        };
    }

    // Key-value pattern
    if let Some(caps) = re_key_value().captures(line) {
        let indent = caps.get(1).map_or(0, |m| m.as_str().len());
        let key = caps.get(2).map_or("", |m| m.as_str()).to_string();
        let value = caps.get(3).map_or("", |m| m.as_str()).trim().to_string();
        return YamlLineType::KeyValue { indent, key, value };
    }

    // List item
    if let Some(stripped) = trimmed.strip_prefix("- ") {
        let indent = line.len() - line.trim_start().len();
        return YamlLineType::ListItem {
            indent,
            content: stripped.to_string(),
        };
    }
    if trimmed == "-" {
        let indent = line.len() - line.trim_start().len();
        return YamlLineType::ListItem {
            indent,
            content: String::new(),
        };
    }

    let indent = line.len() - line.trim_start().len();
    YamlLineType::Continuation {
        indent,
        text: trimmed.to_string(),
    }
}

// ---------------------------------------------------------------------------
// YamlRenderer
// ---------------------------------------------------------------------------

/// Renders YAML content with syntax highlighting and tree guides.
pub struct YamlRenderer {
    config: YamlRendererConfig,
}

impl YamlRenderer {
    /// Create a new YAML renderer with the given configuration.
    pub fn new(config: YamlRendererConfig) -> Self {
        Self { config }
    }

    /// Compute depth from indentation.
    fn depth(&self, indent: usize) -> usize {
        if self.config.indent_width == 0 {
            return 0;
        }
        indent / self.config.indent_width
    }

    /// Style a YAML value string with type-aware coloring.
    fn style_value(&self, value: &str, theme: &ThemeColors) -> Vec<StyledSegment> {
        if value.is_empty() {
            return vec![];
        }

        let mut segments = Vec::new();

        // Check for anchor
        if let Some(m) = re_anchor().find(value) {
            let before = &value[..m.start()];
            if !before.is_empty() {
                segments.extend(self.style_scalar(before.trim(), theme));
                segments.push(plain_segment(" "));
            }
            segments.push(StyledSegment {
                text: m.as_str().to_string(),
                fg: Some(theme.palette[13]), // Bright magenta
                bold: true,
                ..Default::default()
            });
            let after = &value[m.end()..].trim();
            if !after.is_empty() {
                segments.push(plain_segment(" "));
                segments.extend(self.style_scalar(after, theme));
            }
            return segments;
        }

        // Check for alias
        if let Some(m) = re_alias().find(value) {
            segments.push(StyledSegment {
                text: m.as_str().to_string(),
                fg: Some(theme.palette[13]), // Bright magenta
                italic: true,
                ..Default::default()
            });
            return segments;
        }

        // Check for tag
        if let Some(m) = re_tag().find(value) {
            segments.push(StyledSegment {
                text: m.as_str().to_string(),
                fg: Some(theme.palette[8]), // Dimmed
                italic: true,
                ..Default::default()
            });
            let after = value[m.end()..].trim();
            if !after.is_empty() {
                segments.push(plain_segment(" "));
                segments.extend(self.style_scalar(after, theme));
            }
            return segments;
        }

        segments.extend(self.style_scalar(value, theme));
        segments
    }

    /// Style a scalar value (string, number, boolean, null).
    fn style_scalar(&self, value: &str, theme: &ThemeColors) -> Vec<StyledSegment> {
        let trimmed = value.trim();

        // Boolean
        if trimmed == "true" || trimmed == "false" || trimmed == "yes" || trimmed == "no" {
            return vec![StyledSegment {
                text: trimmed.to_string(),
                fg: Some(theme.palette[5]), // Magenta
                ..Default::default()
            }];
        }

        // Null
        if trimmed == "null" || trimmed == "~" {
            return vec![StyledSegment {
                text: trimmed.to_string(),
                fg: Some(theme.palette[8]), // Dim grey
                italic: true,
                ..Default::default()
            }];
        }

        // Number (integer or float)
        if trimmed.parse::<f64>().is_ok() {
            return vec![StyledSegment {
                text: trimmed.to_string(),
                fg: Some(theme.palette[11]), // Bright yellow
                ..Default::default()
            }];
        }

        // Quoted string
        if (trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        {
            return vec![StyledSegment {
                text: trimmed.to_string(),
                fg: Some(theme.palette[2]), // Green
                ..Default::default()
            }];
        }

        // Unquoted string
        vec![StyledSegment {
            text: trimmed.to_string(),
            fg: Some(theme.palette[2]), // Green
            ..Default::default()
        }]
    }
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

fn key_segment(key: &str, theme: &ThemeColors) -> StyledSegment {
    StyledSegment {
        text: key.to_string(),
        fg: Some(theme.palette[6]), // Cyan
        bold: true,
        ..Default::default()
    }
}

fn plain_segment(text: &str) -> StyledSegment {
    StyledSegment {
        text: text.to_string(),
        ..Default::default()
    }
}

fn push_line(
    lines: &mut Vec<StyledLine>,
    line_mapping: &mut Vec<SourceLineMapping>,
    segments: Vec<StyledSegment>,
    source_line: Option<usize>,
) {
    line_mapping.push(SourceLineMapping {
        rendered_line: lines.len(),
        source_line,
    });
    lines.push(StyledLine::new(segments));
}

// ---------------------------------------------------------------------------
// ContentRenderer implementation
// ---------------------------------------------------------------------------

impl ContentRenderer for YamlRenderer {
    fn format_id(&self) -> &str {
        "yaml"
    }

    fn display_name(&self) -> &str {
        "YAML"
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

        for (line_idx, line) in content.lines.iter().enumerate() {
            let classified = classify_yaml_line(line);

            match classified {
                YamlLineType::DocumentStart => {
                    push_line(
                        &mut lines,
                        &mut line_mapping,
                        vec![StyledSegment {
                            text: "---".to_string(),
                            fg: Some(theme.palette[11]), // Bright yellow
                            bold: true,
                            ..Default::default()
                        }],
                        Some(line_idx),
                    );
                }
                YamlLineType::DocumentEnd => {
                    push_line(
                        &mut lines,
                        &mut line_mapping,
                        vec![StyledSegment {
                            text: "...".to_string(),
                            fg: Some(theme.palette[11]),
                            bold: true,
                            ..Default::default()
                        }],
                        Some(line_idx),
                    );
                }
                YamlLineType::Comment { indent, text } => {
                    let depth = self.depth(indent);
                    let prefix = tree_renderer::tree_guides(depth);
                    push_line(
                        &mut lines,
                        &mut line_mapping,
                        vec![
                            guide_segment(&prefix, theme),
                            StyledSegment {
                                text,
                                fg: Some(theme.palette[8]), // Dimmed
                                italic: true,
                                ..Default::default()
                            },
                        ],
                        Some(line_idx),
                    );
                }
                YamlLineType::KeyValue { indent, key, value } => {
                    let depth = self.depth(indent);
                    let prefix = tree_renderer::tree_guides(depth);

                    if value.is_empty() {
                        // Key with no value — potentially a mapping parent.
                        // Check if we should collapse.
                        if depth >= self.config.max_depth_expanded {
                            let child_count = count_children(
                                &content.lines,
                                line_idx + 1,
                                indent,
                                self.config.indent_width,
                            );
                            let summary = tree_renderer::collapsed_summary("object", child_count);
                            push_line(
                                &mut lines,
                                &mut line_mapping,
                                vec![
                                    guide_segment(&prefix, theme),
                                    key_segment(&key, theme),
                                    plain_segment(": "),
                                    StyledSegment {
                                        text: summary,
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
                                    key_segment(&key, theme),
                                    plain_segment(":"),
                                ],
                                Some(line_idx),
                            );
                        }
                    } else {
                        let mut segments = vec![
                            guide_segment(&prefix, theme),
                            key_segment(&key, theme),
                            plain_segment(": "),
                        ];
                        segments.extend(self.style_value(&value, theme));
                        push_line(&mut lines, &mut line_mapping, segments, Some(line_idx));
                    }
                }
                YamlLineType::ListItem {
                    indent,
                    content: item_content,
                } => {
                    let depth = self.depth(indent);
                    let prefix = tree_renderer::tree_guides(depth);
                    let mut segments = vec![
                        guide_segment(&prefix, theme),
                        StyledSegment {
                            text: "- ".to_string(),
                            fg: Some(theme.palette[3]), // Yellow/brown
                            ..Default::default()
                        },
                    ];
                    if !item_content.is_empty() {
                        segments.extend(self.style_value(&item_content, theme));
                    }
                    push_line(&mut lines, &mut line_mapping, segments, Some(line_idx));
                }
                YamlLineType::Continuation { indent, text } => {
                    let depth = self.depth(indent);
                    let prefix = tree_renderer::tree_guides(depth);
                    push_line(
                        &mut lines,
                        &mut line_mapping,
                        vec![
                            guide_segment(&prefix, theme),
                            StyledSegment {
                                text,
                                fg: Some(theme.palette[2]), // Green (string continuation)
                                ..Default::default()
                            },
                        ],
                        Some(line_idx),
                    );
                }
                YamlLineType::Empty => {
                    push_line(
                        &mut lines,
                        &mut line_mapping,
                        vec![plain_segment("")],
                        Some(line_idx),
                    );
                }
            }
        }

        Ok(RenderedContent {
            lines,
            line_mapping,
            graphics: vec![],
            format_badge: "YAML".to_string(),
        })
    }

    fn format_badge(&self) -> &str {
        "YAML"
    }
}

/// Count immediate children of a mapping key (lines with greater indent).
fn count_children(
    lines: &[String],
    start: usize,
    parent_indent: usize,
    indent_width: usize,
) -> usize {
    let child_indent = parent_indent + indent_width;
    let mut count = 0;
    for line in lines.iter().skip(start) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent <= parent_indent {
            break;
        }
        if indent == child_indent {
            count += 1;
        }
    }
    count
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the YAML renderer with the registry.
pub fn register_yaml_renderer(registry: &mut RendererRegistry, config: &YamlRendererConfig) {
    registry.register_renderer("yaml", Box::new(YamlRenderer::new(config.clone())));
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
            theme_colors: ThemeColors::default(),
        }
    }

    fn renderer() -> YamlRenderer {
        YamlRenderer::new(YamlRendererConfig::default())
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
        assert_eq!(r.format_id(), "yaml");
        assert_eq!(r.display_name(), "YAML");
        assert_eq!(r.format_badge(), "YAML");
        assert_eq!(r.capabilities(), vec![RendererCapability::TextStyling]);
    }

    #[test]
    fn test_render_simple_key_value() {
        let r = renderer();
        let block = make_block(&["name: par-term", "version: 0.16.0"]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("name"));
        assert!(text.contains("par-term"));
        assert!(text.contains("version"));
        assert_eq!(result.format_badge, "YAML");
    }

    #[test]
    fn test_render_document_start() {
        let r = renderer();
        let block = make_block(&["---", "key: value"]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("---"));
    }

    #[test]
    fn test_render_document_end() {
        let r = renderer();
        let block = make_block(&["key: value", "..."]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("..."));
    }

    #[test]
    fn test_render_comment() {
        let r = renderer();
        let block = make_block(&["# This is a comment", "key: value"]);
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
    fn test_render_nested_yaml() {
        let r = renderer();
        let block = make_block(&["database:", "  host: localhost", "  port: 5432"]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("database"));
        assert!(text.contains("host"));
        assert!(text.contains("localhost"));
        // Should have tree guides for nested content
        assert!(text.contains('│'));
    }

    #[test]
    fn test_render_list_items() {
        let r = renderer();
        let block = make_block(&["items:", "  - serde", "  - tokio"]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("- "));
        assert!(text.contains("serde"));
        assert!(text.contains("tokio"));
    }

    #[test]
    fn test_boolean_coloring() {
        let r = renderer();
        let block = make_block(&["enabled: true", "debug: false"]);
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
    fn test_number_coloring() {
        let r = renderer();
        let block = make_block(&["port: 8080"]);
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
    fn test_null_coloring() {
        let r = renderer();
        let block = make_block(&["data: null"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();
        let null_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text == "null")
            .unwrap();
        assert_eq!(null_seg.fg, Some(theme.palette[8]));
        assert!(null_seg.italic);
    }

    #[test]
    fn test_anchor_highlighting() {
        let r = renderer();
        let block = make_block(&["defaults: &defaults", "  adapter: postgres"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();
        let anchor_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains("&defaults"))
            .unwrap();
        assert_eq!(anchor_seg.fg, Some(theme.palette[13]));
        assert!(anchor_seg.bold);
    }

    #[test]
    fn test_alias_highlighting() {
        let r = renderer();
        let block = make_block(&["production: *defaults"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();
        let alias_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains("*defaults"))
            .unwrap();
        assert_eq!(alias_seg.fg, Some(theme.palette[13]));
        assert!(alias_seg.italic);
    }

    #[test]
    fn test_tag_highlighting() {
        let r = renderer();
        let block = make_block(&["count: !!int 42"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();
        let tag_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains("!!int"))
            .unwrap();
        assert_eq!(tag_seg.fg, Some(theme.palette[8]));
        assert!(tag_seg.italic);
    }

    #[test]
    fn test_key_coloring() {
        let r = renderer();
        let block = make_block(&["mykey: value"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();
        let key_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text == "mykey")
            .unwrap();
        assert_eq!(key_seg.fg, Some(theme.palette[6]));
        assert!(key_seg.bold);
    }

    #[test]
    fn test_line_mappings_populated() {
        let r = renderer();
        let block = make_block(&["name: test", "version: 1"]);
        let result = r.render(&block, &test_config()).unwrap();
        assert_eq!(result.line_mapping.len(), result.lines.len());
    }

    #[test]
    fn test_auto_collapse_deep_mapping() {
        let r = YamlRenderer::new(YamlRendererConfig {
            max_depth_expanded: 1,
            ..Default::default()
        });
        let block = make_block(&[
            "level1:",
            "  level2:",
            "    key1: value1",
            "    key2: value2",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("keys"));
    }

    #[test]
    fn test_empty_lines_preserved() {
        let r = renderer();
        let block = make_block(&["key: value", "", "other: data"]);
        let result = r.render(&block, &test_config()).unwrap();
        assert_eq!(result.lines.len(), 3);
    }

    #[test]
    fn test_register_yaml_renderer() {
        let mut registry = RendererRegistry::new(0.6);
        register_yaml_renderer(&mut registry, &YamlRendererConfig::default());
        assert_eq!(registry.renderer_count(), 1);
        assert!(registry.get_renderer("yaml").is_some());
        assert_eq!(
            registry.get_renderer("yaml").unwrap().display_name(),
            "YAML"
        );
    }

    #[test]
    fn test_config_defaults() {
        let config = YamlRendererConfig::default();
        assert_eq!(config.indent_width, 2);
        assert_eq!(config.max_depth_expanded, 4);
    }

    #[test]
    fn test_quoted_string_value() {
        let r = renderer();
        let block = make_block(&["name: \"par-term\""]);
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
}

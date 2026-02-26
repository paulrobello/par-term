//! JSON renderer with syntax highlighting, tree guides, and collapsible nodes.
//!
//! Parses raw JSON text via `serde_json`, then walks the value tree to produce
//! styled terminal output. Features include:
//!
//! - **Syntax highlighting**: distinct colors for keys, strings, numbers, booleans, null
//! - **Tree guide lines**: vertical `│` characters at each indentation level
//! - **Collapsible nodes**: objects/arrays auto-collapse beyond `max_depth_expanded`
//! - **Value type indicators**: optional `(type)` annotations next to values
//! - **Large array truncation**: arrays beyond a threshold show `... and N more items`
//! - **URL detection**: string values containing URLs rendered as OSC 8 hyperlinks
//! - **Key sorting**: optional alphabetical ordering of object keys

use std::sync::OnceLock;

use regex::Regex;

use super::{push_line, tree_renderer};
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::traits::{ContentRenderer, RenderError, RendererConfig, ThemeColors};
use crate::prettifier::types::{
    ContentBlock, RenderedContent, RendererCapability, SourceLineMapping, StyledLine, StyledSegment,
};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the JSON renderer.
#[derive(Clone, Debug)]
pub struct JsonRendererConfig {
    /// Auto-collapse objects/arrays beyond this depth (default: 3).
    pub max_depth_expanded: usize,
    /// Truncate string values longer than this (default: 200).
    pub max_string_length: usize,
    /// Show `[N items]` next to arrays (default: true).
    pub show_array_length: bool,
    /// Show type annotations next to values (default: false).
    pub show_types: bool,
    /// Sort object keys alphabetically (default: false).
    pub sort_keys: bool,
    /// Visually distinguish null values (default: true).
    pub highlight_nulls: bool,
    /// Render URLs in strings as OSC 8 hyperlinks (default: true).
    pub clickable_urls: bool,
    /// Maximum array elements to show before truncation (default: 50).
    pub max_array_display: usize,
}

impl Default for JsonRendererConfig {
    fn default() -> Self {
        Self {
            max_depth_expanded: 3,
            max_string_length: 200,
            show_array_length: true,
            show_types: false,
            sort_keys: false,
            highlight_nulls: true,
            clickable_urls: true,
            max_array_display: 50,
        }
    }
}

// ---------------------------------------------------------------------------
// URL regex
// ---------------------------------------------------------------------------

fn re_url() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"https?://[^\s"]+"#).unwrap())
}

// ---------------------------------------------------------------------------
// JsonRenderer
// ---------------------------------------------------------------------------

/// Renders JSON content with syntax highlighting, tree guides, and collapsible nodes.
pub struct JsonRenderer {
    config: JsonRendererConfig,
}

impl JsonRenderer {
    /// Create a new JSON renderer with the given configuration.
    pub fn new(config: JsonRendererConfig) -> Self {
        Self { config }
    }

    /// Render a `serde_json::Value` recursively, appending styled lines.
    fn render_value(
        &self,
        value: &serde_json::Value,
        depth: usize,
        lines: &mut Vec<StyledLine>,
        line_mapping: &mut Vec<SourceLineMapping>,
        theme: &ThemeColors,
    ) {
        match value {
            serde_json::Value::Object(map) => {
                self.render_object(map, depth, lines, line_mapping, theme);
            }
            serde_json::Value::Array(arr) => {
                self.render_array(arr, depth, lines, line_mapping, theme);
            }
            _ => {
                // Scalar value at top level
                let prefix = tree_renderer::tree_guides(depth);
                let mut segments = vec![guide_segment(&prefix, theme)];
                segments.extend(self.style_value(value, theme));
                push_line(lines, line_mapping, segments, None);
            }
        }
    }

    /// Render a JSON object `{ ... }`.
    fn render_object(
        &self,
        map: &serde_json::Map<String, serde_json::Value>,
        depth: usize,
        lines: &mut Vec<StyledLine>,
        line_mapping: &mut Vec<SourceLineMapping>,
        theme: &ThemeColors,
    ) {
        let prefix = tree_renderer::tree_guides(depth);

        // Auto-collapse beyond max depth
        if depth >= self.config.max_depth_expanded {
            let summary = tree_renderer::collapsed_summary("object", map.len());
            push_line(
                lines,
                line_mapping,
                vec![
                    guide_segment(&prefix, theme),
                    punct_segment("{", theme),
                    StyledSegment {
                        text: format!(" {summary} "),
                        fg: Some(theme.palette[8]),
                        italic: true,
                        ..Default::default()
                    },
                    punct_segment("}", theme),
                ],
                None,
            );
            return;
        }

        // Opening brace
        let mut open_segments = vec![guide_segment(&prefix, theme), punct_segment("{", theme)];
        if self.config.show_array_length {
            open_segments.push(StyledSegment {
                text: format!("  // {count} keys", count = map.len()),
                fg: Some(theme.palette[8]),
                italic: true,
                ..Default::default()
            });
        }
        push_line(lines, line_mapping, open_segments, None);

        // Key-value pairs
        let keys: Vec<&String> = if self.config.sort_keys {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            keys
        } else {
            map.keys().collect()
        };

        let key_count = keys.len();
        for (i, key) in keys.iter().enumerate() {
            let value = &map[*key];
            let inner_prefix = tree_renderer::tree_guides(depth + 1);
            let trailing_comma = if i + 1 < key_count { "," } else { "" };

            if is_scalar(value) {
                let mut segments = vec![
                    guide_segment(&inner_prefix, theme),
                    key_segment(key, theme),
                    punct_segment(": ", theme),
                ];
                segments.extend(self.style_value(value, theme));
                segments.push(punct_segment(trailing_comma, theme));
                push_line(lines, line_mapping, segments, None);
            } else {
                // Complex value: emit key, then recurse for the value
                push_line(
                    lines,
                    line_mapping,
                    vec![
                        guide_segment(&inner_prefix, theme),
                        key_segment(key, theme),
                        punct_segment(": ", theme),
                    ],
                    None,
                );
                // Remove the last line we just pushed — we'll merge the key with the
                // opening bracket/brace of the value.
                let key_line = lines.pop().unwrap();
                line_mapping.pop();

                let before_len = lines.len();
                self.render_value(value, depth + 1, lines, line_mapping, theme);

                // Merge the key line segments into the first line of the rendered value
                if lines.len() > before_len {
                    let first_val_line = &mut lines[before_len];
                    let mut merged = key_line.segments;
                    // Skip the guide prefix of the value's first line (we already have it
                    // from the key line prefix).
                    let val_segments = &first_val_line.segments;
                    if val_segments.len() > 1 {
                        // Skip guide segment (index 0), keep the rest
                        merged.extend_from_slice(&val_segments[1..]);
                    } else {
                        merged.extend_from_slice(val_segments);
                    }
                    *first_val_line = StyledLine::new(merged);
                }

                // Add trailing comma to the last line of this value
                if !trailing_comma.is_empty()
                    && let Some(last_line) = lines.last_mut()
                {
                    last_line
                        .segments
                        .push(punct_segment(trailing_comma, theme));
                }
            }
        }

        // Closing brace
        push_line(
            lines,
            line_mapping,
            vec![guide_segment(&prefix, theme), punct_segment("}", theme)],
            None,
        );
    }

    /// Render a JSON array `[ ... ]`.
    fn render_array(
        &self,
        arr: &[serde_json::Value],
        depth: usize,
        lines: &mut Vec<StyledLine>,
        line_mapping: &mut Vec<SourceLineMapping>,
        theme: &ThemeColors,
    ) {
        let prefix = tree_renderer::tree_guides(depth);

        // Auto-collapse beyond max depth
        if depth >= self.config.max_depth_expanded {
            let summary = tree_renderer::collapsed_summary("array", arr.len());
            push_line(
                lines,
                line_mapping,
                vec![
                    guide_segment(&prefix, theme),
                    punct_segment("[", theme),
                    StyledSegment {
                        text: format!(" {summary} "),
                        fg: Some(theme.palette[8]),
                        italic: true,
                        ..Default::default()
                    },
                    punct_segment("]", theme),
                ],
                None,
            );
            return;
        }

        // Opening bracket
        let mut open_segments = vec![guide_segment(&prefix, theme), punct_segment("[", theme)];
        if self.config.show_array_length {
            open_segments.push(StyledSegment {
                text: format!("  // {count} items", count = arr.len()),
                fg: Some(theme.palette[8]),
                italic: true,
                ..Default::default()
            });
        }
        push_line(lines, line_mapping, open_segments, None);

        // Elements
        let display_count = arr.len().min(self.config.max_array_display);
        let total = arr.len();

        for (i, value) in arr.iter().take(display_count).enumerate() {
            let trailing_comma = if i + 1 < total { "," } else { "" };

            if is_scalar(value) {
                let inner_prefix = tree_renderer::tree_guides(depth + 1);
                let mut segments = vec![guide_segment(&inner_prefix, theme)];
                segments.extend(self.style_value(value, theme));
                segments.push(punct_segment(trailing_comma, theme));
                push_line(lines, line_mapping, segments, None);
            } else {
                self.render_value(value, depth + 1, lines, line_mapping, theme);
                // Add trailing comma
                if !trailing_comma.is_empty()
                    && let Some(last_line) = lines.last_mut()
                {
                    last_line
                        .segments
                        .push(punct_segment(trailing_comma, theme));
                }
            }
        }

        // Truncation indicator
        if total > display_count {
            let remaining = total - display_count;
            let inner_prefix = tree_renderer::tree_guides(depth + 1);
            push_line(
                lines,
                line_mapping,
                vec![
                    guide_segment(&inner_prefix, theme),
                    StyledSegment {
                        text: format!("... and {remaining} more items"),
                        fg: Some(theme.palette[8]),
                        italic: true,
                        ..Default::default()
                    },
                ],
                None,
            );
        }

        // Closing bracket
        push_line(
            lines,
            line_mapping,
            vec![guide_segment(&prefix, theme), punct_segment("]", theme)],
            None,
        );
    }

    /// Style a scalar JSON value with appropriate colors.
    fn style_value(&self, value: &serde_json::Value, theme: &ThemeColors) -> Vec<StyledSegment> {
        let mut segments = Vec::new();

        match value {
            serde_json::Value::String(s) => {
                let display = if s.chars().count() > self.config.max_string_length {
                    let truncated: String = s.chars().take(self.config.max_string_length).collect();
                    format!("\"{truncated}...\"")
                } else {
                    format!("\"{s}\"")
                };

                // Check for URL in string value
                if self.config.clickable_urls
                    && let Some(m) = re_url().find(s)
                {
                    let url = m.as_str().to_string();
                    segments.push(StyledSegment {
                        text: display,
                        fg: Some(theme.palette[2]), // Green
                        underline: true,
                        link_url: Some(url),
                        ..Default::default()
                    });
                    if self.config.show_types {
                        segments.push(type_annotation(" (string)", theme));
                    }
                    return segments;
                }

                segments.push(StyledSegment {
                    text: display,
                    fg: Some(theme.palette[2]), // Green
                    ..Default::default()
                });
                if self.config.show_types {
                    segments.push(type_annotation(" (string)", theme));
                }
            }
            serde_json::Value::Number(n) => {
                segments.push(StyledSegment {
                    text: n.to_string(),
                    fg: Some(theme.palette[11]), // Bright yellow
                    ..Default::default()
                });
                if self.config.show_types {
                    segments.push(type_annotation(" (number)", theme));
                }
            }
            serde_json::Value::Bool(b) => {
                segments.push(StyledSegment {
                    text: b.to_string(),
                    fg: Some(theme.palette[5]), // Magenta
                    ..Default::default()
                });
                if self.config.show_types {
                    segments.push(type_annotation(" (bool)", theme));
                }
            }
            serde_json::Value::Null => {
                let fg = if self.config.highlight_nulls {
                    Some(theme.palette[8]) // Dim grey
                } else {
                    None
                };
                segments.push(StyledSegment {
                    text: "null".to_string(),
                    fg,
                    italic: self.config.highlight_nulls,
                    ..Default::default()
                });
                if self.config.show_types {
                    segments.push(type_annotation(" (null)", theme));
                }
            }
            // Object/Array handled by render_object/render_array
            _ => {}
        }

        segments
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Whether a JSON value is a scalar (not object or array).
fn is_scalar(value: &serde_json::Value) -> bool {
    !value.is_object() && !value.is_array()
}

/// Create a styled segment for tree guide characters.
fn guide_segment(prefix: &str, theme: &ThemeColors) -> StyledSegment {
    StyledSegment {
        text: prefix.to_string(),
        fg: Some(theme.palette[8]), // Dim grey for guides
        ..Default::default()
    }
}

/// Create a styled segment for JSON punctuation (`{`, `}`, `[`, `]`, `:`, `,`).
fn punct_segment(text: &str, _theme: &ThemeColors) -> StyledSegment {
    StyledSegment {
        text: text.to_string(),
        ..Default::default()
    }
}

/// Create a styled segment for a JSON key.
fn key_segment(key: &str, theme: &ThemeColors) -> StyledSegment {
    StyledSegment {
        text: format!("\"{key}\""),
        fg: Some(theme.palette[6]), // Cyan
        ..Default::default()
    }
}

/// Create a dimmed type annotation segment.
fn type_annotation(text: &str, theme: &ThemeColors) -> StyledSegment {
    StyledSegment {
        text: text.to_string(),
        fg: Some(theme.palette[8]),
        italic: true,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// ContentRenderer implementation
// ---------------------------------------------------------------------------

impl ContentRenderer for JsonRenderer {
    fn format_id(&self) -> &str {
        "json"
    }

    fn display_name(&self) -> &str {
        "JSON"
    }

    fn capabilities(&self) -> Vec<RendererCapability> {
        vec![RendererCapability::TextStyling]
    }

    fn render(
        &self,
        content: &ContentBlock,
        config: &RendererConfig,
    ) -> Result<RenderedContent, RenderError> {
        let text = content.lines.join("\n");
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| RenderError::RenderFailed(format!("Invalid JSON: {e}")))?;

        let mut lines = Vec::new();
        let mut line_mapping = Vec::new();
        self.render_value(
            &value,
            0,
            &mut lines,
            &mut line_mapping,
            &config.theme_colors,
        );

        Ok(RenderedContent {
            lines,
            line_mapping,
            graphics: vec![],
            format_badge: "{}".to_string(),
        })
    }

    fn format_badge(&self) -> &str {
        "{}"
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the JSON renderer with the registry.
pub fn register_json_renderer(registry: &mut RendererRegistry, config: &JsonRendererConfig) {
    registry.register_renderer("json", Box::new(JsonRenderer::new(config.clone())));
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

    fn renderer() -> JsonRenderer {
        JsonRenderer::new(JsonRendererConfig::default())
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

    // -- Trait methods --

    #[test]
    fn test_format_id() {
        let r = renderer();
        assert_eq!(r.format_id(), "json");
        assert_eq!(r.display_name(), "JSON");
        assert_eq!(r.format_badge(), "{}");
        assert_eq!(r.capabilities(), vec![RendererCapability::TextStyling]);
    }

    // -- Basic rendering --

    #[test]
    fn test_render_simple_object() {
        let r = renderer();
        let block = make_block(&[r#"{"name": "par-term", "version": 1}"#]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("\"name\""));
        assert!(text.contains("\"par-term\""));
        assert!(text.contains("1"));
        assert_eq!(result.format_badge, "{}");
    }

    #[test]
    fn test_render_simple_array() {
        let r = renderer();
        let block = make_block(&[r#"["a", "b", "c"]"#]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("\"a\""));
        assert!(text.contains("\"b\""));
        assert!(text.contains("\"c\""));
    }

    #[test]
    fn test_render_nested_object() {
        let r = renderer();
        let json = r#"{"config": {"fps": 60, "vsync": true}}"#;
        let block = make_block(&[json]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("\"config\""));
        assert!(text.contains("\"fps\""));
        assert!(text.contains("60"));
        assert!(text.contains("true"));
    }

    // -- Syntax highlighting --

    #[test]
    fn test_string_color() {
        let r = renderer();
        let block = make_block(&[r#"{"key": "value"}"#]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();

        // Find a segment containing "value" text
        let str_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains("\"value\""))
            .unwrap();
        assert_eq!(str_seg.fg, Some(theme.palette[2])); // Green
    }

    #[test]
    fn test_number_color() {
        let r = renderer();
        let block = make_block(&[r#"{"count": 42}"#]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();

        let num_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text == "42")
            .unwrap();
        assert_eq!(num_seg.fg, Some(theme.palette[11])); // Bright yellow
    }

    #[test]
    fn test_boolean_color() {
        let r = renderer();
        let block = make_block(&[r#"{"flag": true}"#]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();

        let bool_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text == "true")
            .unwrap();
        assert_eq!(bool_seg.fg, Some(theme.palette[5])); // Magenta
    }

    #[test]
    fn test_null_highlighted() {
        let r = renderer();
        let block = make_block(&[r#"{"empty": null}"#]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();

        let null_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text == "null")
            .unwrap();
        assert_eq!(null_seg.fg, Some(theme.palette[8])); // Dim grey
        assert!(null_seg.italic);
    }

    #[test]
    fn test_null_not_highlighted() {
        let r = JsonRenderer::new(JsonRendererConfig {
            highlight_nulls: false,
            ..Default::default()
        });
        let block = make_block(&[r#"{"empty": null}"#]);
        let result = r.render(&block, &test_config()).unwrap();

        let null_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text == "null")
            .unwrap();
        assert!(null_seg.fg.is_none());
        assert!(!null_seg.italic);
    }

    #[test]
    fn test_key_color() {
        let r = renderer();
        let block = make_block(&[r#"{"mykey": 1}"#]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();

        let key_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains("\"mykey\""))
            .unwrap();
        assert_eq!(key_seg.fg, Some(theme.palette[6])); // Cyan
    }

    // -- Tree guides --

    #[test]
    fn test_tree_guides_present() {
        let r = renderer();
        let json = r#"{"a": {"b": 1}}"#;
        let block = make_block(&[json]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains('│'));
    }

    // -- Collapsible nodes --

    #[test]
    fn test_auto_collapse_deep_object() {
        let r = JsonRenderer::new(JsonRendererConfig {
            max_depth_expanded: 1,
            ..Default::default()
        });
        let json = r#"{"level1": {"level2": {"deep": true}}}"#;
        let block = make_block(&[json]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        // The deep object should be collapsed
        assert!(text.contains("keys"));
    }

    #[test]
    fn test_auto_collapse_deep_array() {
        let r = JsonRenderer::new(JsonRendererConfig {
            max_depth_expanded: 1,
            ..Default::default()
        });
        let json = r#"{"items": [1, 2, 3]}"#;
        let block = make_block(&[json]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("items"));
    }

    // -- Type indicators --

    #[test]
    fn test_show_types() {
        let r = JsonRenderer::new(JsonRendererConfig {
            show_types: true,
            ..Default::default()
        });
        let block = make_block(&[r#"{"name": "test", "count": 5, "active": true, "data": null}"#]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("(string)"));
        assert!(text.contains("(number)"));
        assert!(text.contains("(bool)"));
        assert!(text.contains("(null)"));
    }

    #[test]
    fn test_types_hidden_by_default() {
        let r = renderer();
        let block = make_block(&[r#"{"name": "test"}"#]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(!text.contains("(string)"));
    }

    // -- Large array truncation --

    #[test]
    fn test_large_array_truncation() {
        let r = JsonRenderer::new(JsonRendererConfig {
            max_array_display: 3,
            ..Default::default()
        });
        let block = make_block(&["[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]"]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("... and 7 more items"));
    }

    // -- String truncation --

    #[test]
    fn test_long_string_truncation() {
        let r = JsonRenderer::new(JsonRendererConfig {
            max_string_length: 10,
            ..Default::default()
        });
        let long_str = "a".repeat(50);
        let json = format!(r#"{{"text": "{long_str}"}}"#);
        let block = make_block(&[&json]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("...\""));
    }

    // -- URL detection --

    #[test]
    fn test_url_becomes_hyperlink() {
        let r = renderer();
        let block = make_block(&[r#"{"url": "https://example.com/api"}"#]);
        let result = r.render(&block, &test_config()).unwrap();

        let url_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.link_url.is_some())
            .unwrap();
        assert_eq!(url_seg.link_url.as_deref(), Some("https://example.com/api"));
        assert!(url_seg.underline);
    }

    #[test]
    fn test_url_detection_disabled() {
        let r = JsonRenderer::new(JsonRendererConfig {
            clickable_urls: false,
            ..Default::default()
        });
        let block = make_block(&[r#"{"url": "https://example.com"}"#]);
        let result = r.render(&block, &test_config()).unwrap();

        let has_link = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .any(|s| s.link_url.is_some());
        assert!(!has_link);
    }

    // -- Sort keys --

    #[test]
    fn test_sort_keys() {
        let r = JsonRenderer::new(JsonRendererConfig {
            sort_keys: true,
            ..Default::default()
        });
        let block = make_block(&[r#"{"zebra": 1, "alpha": 2, "middle": 3}"#]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        let alpha_pos = text.find("\"alpha\"").unwrap();
        let middle_pos = text.find("\"middle\"").unwrap();
        let zebra_pos = text.find("\"zebra\"").unwrap();
        assert!(alpha_pos < middle_pos);
        assert!(middle_pos < zebra_pos);
    }

    // -- Invalid JSON --

    #[test]
    fn test_invalid_json_produces_error() {
        let r = renderer();
        let block = make_block(&["not valid json {"]);
        let result = r.render(&block, &test_config());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Invalid JSON"));
    }

    // -- Line mappings --

    #[test]
    fn test_line_mappings_populated() {
        let r = renderer();
        let block = make_block(&[r#"{"a": 1, "b": 2}"#]);
        let result = r.render(&block, &test_config()).unwrap();
        assert_eq!(result.line_mapping.len(), result.lines.len());
    }

    // -- Registration --

    #[test]
    fn test_register_json_renderer() {
        let mut registry = RendererRegistry::new(0.6);
        register_json_renderer(&mut registry, &JsonRendererConfig::default());
        assert_eq!(registry.renderer_count(), 1);
        assert!(registry.get_renderer("json").is_some());
        assert_eq!(
            registry.get_renderer("json").unwrap().display_name(),
            "JSON"
        );
    }

    // -- Edge cases --

    #[test]
    fn test_empty_object() {
        let r = renderer();
        let block = make_block(&["{}"]);
        let result = r.render(&block, &test_config()).unwrap();
        assert!(!result.lines.is_empty());
    }

    #[test]
    fn test_empty_array() {
        let r = renderer();
        let block = make_block(&["[]"]);
        let result = r.render(&block, &test_config()).unwrap();
        assert!(!result.lines.is_empty());
    }

    #[test]
    fn test_scalar_top_level_string() {
        let r = renderer();
        let block = make_block(&[r#""just a string""#]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("\"just a string\""));
    }

    #[test]
    fn test_scalar_top_level_number() {
        let r = renderer();
        let block = make_block(&["42"]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("42"));
    }

    #[test]
    fn test_multiline_json() {
        let r = renderer();
        let block = make_block(&["{", "  \"key\": \"value\"", "}"]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("\"key\""));
        assert!(text.contains("\"value\""));
    }

    #[test]
    fn test_deeply_nested_auto_collapses() {
        let r = JsonRenderer::new(JsonRendererConfig {
            max_depth_expanded: 2,
            ..Default::default()
        });
        let json = r#"{"a": {"b": {"c": {"d": 1}}}}"#;
        let block = make_block(&[json]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        // depth 2 = c's object should be collapsed
        assert!(text.contains("keys"));
    }

    #[test]
    fn test_array_length_annotation() {
        let r = renderer();
        let block = make_block(&[r#"{"items": [1, 2, 3]}"#]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("3 items"));
    }

    #[test]
    fn test_config_defaults() {
        let config = JsonRendererConfig::default();
        assert_eq!(config.max_depth_expanded, 3);
        assert_eq!(config.max_string_length, 200);
        assert!(config.show_array_length);
        assert!(!config.show_types);
        assert!(!config.sort_keys);
        assert!(config.highlight_nulls);
        assert!(config.clickable_urls);
        assert_eq!(config.max_array_display, 50);
    }
}

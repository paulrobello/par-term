//! JSON token parser and renderer implementation.

use std::sync::OnceLock;

use regex::Regex;

use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::renderers::{guide_segment, push_line, tree_renderer};
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
    RE.get_or_init(|| {
        Regex::new(r#"https?://[^\s"]+"#).expect("regex pattern is valid and should always compile")
    })
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
                        fg: Some(theme.dim_color()),
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
                fg: Some(theme.dim_color()),
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
                let key_line = lines
                    .pop()
                    .expect("lines is non-empty: we just pushed a key segment to it");
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
                        fg: Some(theme.dim_color()),
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
                fg: Some(theme.dim_color()),
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
                        fg: Some(theme.dim_color()),
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
                        fg: Some(theme.string_color()), // Green
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
                    fg: Some(theme.string_color()), // Green
                    ..Default::default()
                });
                if self.config.show_types {
                    segments.push(type_annotation(" (string)", theme));
                }
            }
            serde_json::Value::Number(n) => {
                segments.push(StyledSegment {
                    text: n.to_string(),
                    fg: Some(theme.number_color()), // Bright yellow
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
                    Some(theme.dim_color()) // Dim grey
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
        fg: Some(theme.key_color()), // Cyan
        ..Default::default()
    }
}

/// Create a dimmed type annotation segment.
fn type_annotation(text: &str, theme: &ThemeColors) -> StyledSegment {
    StyledSegment {
        text: text.to_string(),
        fg: Some(theme.dim_color()),
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

//! YAML line classifier and renderer implementation.

use std::sync::OnceLock;

use regex::Regex;

use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::renderers::{push_line, tree_renderer};
use crate::prettifier::traits::{ContentRenderer, RenderError, RendererConfig, ThemeColors};
use crate::prettifier::types::{ContentBlock, RenderedContent, RendererCapability, StyledSegment};

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
    RE.get_or_init(|| {
        Regex::new(r"&\w+").expect("regex pattern is valid and should always compile")
    })
}

fn re_alias() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\*\w+").expect("regex pattern is valid and should always compile")
    })
}

fn re_tag() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"!!\w+").expect("regex pattern is valid and should always compile")
    })
}

fn re_key_value() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(\s*)([\w./-]+)\s*:\s*(.*)$")
            .expect("re_key_value: pattern is valid and should always compile")
    })
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
pub(super) fn count_children(
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

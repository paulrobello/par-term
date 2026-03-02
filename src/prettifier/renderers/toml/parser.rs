//! TOML line classifier and renderer implementation.

use std::sync::OnceLock;

use regex::Regex;

use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::renderers::{guide_segment, plain_segment, push_line, tree_renderer};
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
    RE.get_or_init(|| {
        Regex::new(r"^\[([\w.-]+)\]\s*$")
            .expect("re_section_header: pattern is valid and should always compile")
    })
}

fn re_array_table() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\[\[([\w.-]+)\]\]\s*$")
            .expect("re_array_table: pattern is valid and should always compile")
    })
}

pub(super) fn re_key_value() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^([\w.-]+)\s*=\s*(.*)$")
            .expect("re_key_value: pattern is valid and should always compile")
    })
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
                fg: Some(theme.string_color()), // Green
                ..Default::default()
            }];
        }

        // String (single-quoted / literal)
        if trimmed.starts_with('\'') && trimmed.ends_with('\'') {
            return vec![StyledSegment {
                text: trimmed.to_string(),
                fg: Some(theme.string_color()), // Green
                ..Default::default()
            }];
        }

        // Multi-line string start
        if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
            return vec![StyledSegment {
                text: trimmed.to_string(),
                fg: Some(theme.string_color()),
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
                fg: Some(theme.number_color()), // Bright yellow
                ..Default::default()
            }];
        }

        // Fallback — unquoted value
        vec![StyledSegment {
            text: trimmed.to_string(),
            fg: Some(theme.string_color()),
            ..Default::default()
        }]
    }
}

/// Check if a value looks like a TOML datetime.
pub(super) fn is_toml_datetime(s: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"^\d{4}-\d{2}-\d{2}([T ]\d{2}:\d{2}(:\d{2})?)?")
            .expect("is_toml_datetime: pattern is valid and should always compile")
    });
    re.is_match(s)
}

/// Check if a value looks like a TOML number.
pub(super) fn is_toml_number(s: &str) -> bool {
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
                                    fg: Some(theme.dim_color()),
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
                                fg: Some(theme.dim_color()),
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
                            fg: Some(theme.key_color()), // Cyan
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

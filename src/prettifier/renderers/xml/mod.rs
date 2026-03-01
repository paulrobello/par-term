//! XML/HTML renderer with syntax highlighting, tree guides, and collapsible elements.
//!
//! Uses a line-by-line parser to produce styled terminal output. Features include:
//!
//! - **Tag hierarchy with indentation and guide lines**: Uses shared tree renderer
//! - **Attribute highlighting**: Tag names, attribute names, and attribute values
//!   in distinct colors
//! - **Collapsible elements**: Deep elements auto-collapse beyond `max_depth_expanded`
//! - **Namespace coloring**: XML namespace prefixes in a distinct color
//! - **CDATA/comment distinction**: CDATA sections and comments styled differently

use std::sync::OnceLock;

use regex::Regex;

use super::{dim_segment, guide_segment, plain_segment, push_line, tree_renderer};
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::traits::{ContentRenderer, RenderError, RendererConfig, ThemeColors};
use crate::prettifier::types::{ContentBlock, RenderedContent, RendererCapability, StyledSegment};

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the XML renderer.
#[derive(Clone, Debug)]
pub struct XmlRendererConfig {
    /// Auto-collapse elements beyond this depth (default: 4).
    pub max_depth_expanded: usize,
}

impl Default for XmlRendererConfig {
    fn default() -> Self {
        Self {
            max_depth_expanded: 4,
        }
    }
}

// ---------------------------------------------------------------------------
// Regex helpers
// ---------------------------------------------------------------------------

fn re_xml_declaration() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^<\?xml\s+.*\?>").expect("regex pattern is valid and should always compile")
    })
}

fn re_comment_start() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"<!--").expect("regex pattern is valid and should always compile")
    })
}

fn re_cdata_start() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"<!\[CDATA\[").expect("regex pattern is valid and should always compile")
    })
}

fn re_opening_tag() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\s*<([a-zA-Z][\w:.-]*)(\s+[^>]*)?>")
            .expect("re_opening_tag: pattern is valid and should always compile")
    })
}

fn re_closing_tag() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\s*</([a-zA-Z][\w:.-]*)>")
            .expect("re_closing_tag: pattern is valid and should always compile")
    })
}

fn re_self_closing_tag() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\s*<([a-zA-Z][\w:.-]*)(\s+[^>]*)?\s*/>")
            .expect("re_self_closing_tag: pattern is valid and should always compile")
    })
}

fn re_attribute() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"([\w:.-]+)\s*=\s*("[^"]*"|'[^']*')"#)
            .expect("re_attribute: pattern is valid and should always compile")
    })
}

fn re_doctype() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^<!DOCTYPE\s+").expect("regex pattern is valid and should always compile")
    })
}

// ---------------------------------------------------------------------------
// XmlRenderer
// ---------------------------------------------------------------------------

/// Renders XML/HTML content with syntax highlighting and tree guides.
pub struct XmlRenderer {
    config: XmlRendererConfig,
}

impl XmlRenderer {
    /// Create a new XML renderer with the given configuration.
    pub fn new(config: XmlRendererConfig) -> Self {
        Self { config }
    }

    /// Style an XML tag name, handling namespace prefixes.
    fn style_tag_name(&self, name: &str, theme: &ThemeColors) -> Vec<StyledSegment> {
        if let Some(colon_pos) = name.find(':') {
            let ns = &name[..colon_pos];
            let local = &name[colon_pos + 1..];
            vec![
                StyledSegment {
                    text: ns.to_string(),
                    fg: Some(theme.palette[5]), // Magenta for namespace
                    ..Default::default()
                },
                StyledSegment {
                    text: ":".to_string(),
                    fg: Some(theme.dim_color()), // Dim
                    ..Default::default()
                },
                StyledSegment {
                    text: local.to_string(),
                    fg: Some(theme.palette[4]), // Blue for tag name
                    bold: true,
                    ..Default::default()
                },
            ]
        } else {
            vec![StyledSegment {
                text: name.to_string(),
                fg: Some(theme.palette[4]), // Blue for tag name
                bold: true,
                ..Default::default()
            }]
        }
    }

    /// Style attributes from an attribute string.
    fn style_attributes(&self, attr_str: &str, theme: &ThemeColors) -> Vec<StyledSegment> {
        let mut segments = Vec::new();
        let mut last_end = 0;

        for caps in re_attribute().captures_iter(attr_str) {
            let full_match = caps
                .get(0)
                .expect("re_attribute capture group 0 (full match) must be present after a match");
            let attr_name = caps
                .get(1)
                .expect(
                    "re_attribute capture group 1 (attribute name) must be present after a match",
                )
                .as_str();
            let attr_value = caps
                .get(2)
                .expect(
                    "re_attribute capture group 2 (attribute value) must be present after a match",
                )
                .as_str();

            // Any text between matches (whitespace)
            if full_match.start() > last_end {
                let between = &attr_str[last_end..full_match.start()];
                if !between.is_empty() {
                    segments.push(plain_segment(between));
                }
            }

            // Handle namespace prefix in attribute name
            if let Some(colon_pos) = attr_name.find(':') {
                let ns = &attr_name[..colon_pos];
                let local = &attr_name[colon_pos + 1..];
                segments.push(StyledSegment {
                    text: ns.to_string(),
                    fg: Some(theme.palette[5]), // Magenta for namespace
                    ..Default::default()
                });
                segments.push(StyledSegment {
                    text: ":".to_string(),
                    fg: Some(theme.dim_color()),
                    ..Default::default()
                });
                segments.push(StyledSegment {
                    text: local.to_string(),
                    fg: Some(theme.key_color()), // Cyan for attribute name
                    ..Default::default()
                });
            } else {
                segments.push(StyledSegment {
                    text: attr_name.to_string(),
                    fg: Some(theme.key_color()), // Cyan for attribute name
                    ..Default::default()
                });
            }

            segments.push(dim_segment("=", theme));

            segments.push(StyledSegment {
                text: attr_value.to_string(),
                fg: Some(theme.string_color()), // Green for attribute value
                ..Default::default()
            });

            last_end = full_match.end();
        }

        // Remaining text after last attribute
        if last_end < attr_str.len() {
            let remaining = &attr_str[last_end..];
            if !remaining.trim().is_empty() {
                segments.push(plain_segment(remaining));
            }
        }

        segments
    }
}

// ---------------------------------------------------------------------------
// ContentRenderer implementation
// ---------------------------------------------------------------------------

impl ContentRenderer for XmlRenderer {
    fn format_id(&self) -> &str {
        "xml"
    }

    fn display_name(&self) -> &str {
        "XML/HTML"
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
        let mut lines = Vec::new();
        let mut line_mapping = Vec::new();
        let mut depth: usize = 0;
        // When collapsing a deep element, skip all children until we return
        // to this depth. `None` means we're not skipping.
        let mut skip_until_depth: Option<usize> = None;

        for (line_idx, line) in content.lines.iter().enumerate() {
            let trimmed = line.trim();

            // While skipping collapsed children, track depth changes but don't emit output.
            if let Some(target_depth) = skip_until_depth {
                if let Some(caps) = re_opening_tag().captures(trimmed) {
                    // Only count non-self-closing opening tags.
                    if !re_self_closing_tag().is_match(trimmed) {
                        let _ = caps; // suppress unused warning
                        depth += 1;
                    }
                } else if re_closing_tag().is_match(trimmed) {
                    depth = depth.saturating_sub(1);
                    if depth <= target_depth {
                        skip_until_depth = None;
                    }
                }
                continue;
            }

            if trimmed.is_empty() {
                push_line(
                    &mut lines,
                    &mut line_mapping,
                    vec![plain_segment("")],
                    Some(line_idx),
                );
                continue;
            }

            // XML declaration
            if re_xml_declaration().is_match(trimmed) {
                let prefix = tree_renderer::tree_guides(depth);
                push_line(
                    &mut lines,
                    &mut line_mapping,
                    vec![
                        guide_segment(&prefix, theme),
                        StyledSegment {
                            text: trimmed.to_string(),
                            fg: Some(theme.dim_color()), // Dim for processing instructions
                            italic: true,
                            ..Default::default()
                        },
                    ],
                    Some(line_idx),
                );
                continue;
            }

            // DOCTYPE
            if re_doctype().is_match(trimmed) {
                let prefix = tree_renderer::tree_guides(depth);
                push_line(
                    &mut lines,
                    &mut line_mapping,
                    vec![
                        guide_segment(&prefix, theme),
                        StyledSegment {
                            text: trimmed.to_string(),
                            fg: Some(theme.dim_color()),
                            italic: true,
                            ..Default::default()
                        },
                    ],
                    Some(line_idx),
                );
                continue;
            }

            // Comment
            if re_comment_start().is_match(trimmed) {
                let prefix = tree_renderer::tree_guides(depth);
                push_line(
                    &mut lines,
                    &mut line_mapping,
                    vec![
                        guide_segment(&prefix, theme),
                        StyledSegment {
                            text: trimmed.to_string(),
                            fg: Some(theme.dim_color()), // Dim for comments
                            italic: true,
                            ..Default::default()
                        },
                    ],
                    Some(line_idx),
                );
                continue;
            }

            // CDATA
            if re_cdata_start().is_match(trimmed) {
                let prefix = tree_renderer::tree_guides(depth);
                push_line(
                    &mut lines,
                    &mut line_mapping,
                    vec![
                        guide_segment(&prefix, theme),
                        StyledSegment {
                            text: trimmed.to_string(),
                            fg: Some(theme.palette[3]), // Yellow for CDATA
                            ..Default::default()
                        },
                    ],
                    Some(line_idx),
                );
                continue;
            }

            // Self-closing tag
            if let Some(caps) = re_self_closing_tag().captures(trimmed) {
                let tag_name = caps.get(1).map_or("", |m| m.as_str());
                let attrs = caps.get(2).map_or("", |m| m.as_str());
                let prefix = tree_renderer::tree_guides(depth);

                let mut segments = vec![guide_segment(&prefix, theme), dim_segment("<", theme)];
                segments.extend(self.style_tag_name(tag_name, theme));
                if !attrs.is_empty() {
                    segments.extend(self.style_attributes(attrs, theme));
                }
                segments.push(dim_segment(" />", theme));

                push_line(&mut lines, &mut line_mapping, segments, Some(line_idx));
                continue;
            }

            // Closing tag (must check before opening tag)
            if let Some(caps) = re_closing_tag().captures(trimmed) {
                let tag_name = caps.get(1).map_or("", |m| m.as_str());
                depth = depth.saturating_sub(1);
                let prefix = tree_renderer::tree_guides(depth);

                let mut segments = vec![guide_segment(&prefix, theme), dim_segment("</", theme)];
                segments.extend(self.style_tag_name(tag_name, theme));
                segments.push(dim_segment(">", theme));

                push_line(&mut lines, &mut line_mapping, segments, Some(line_idx));
                continue;
            }

            // Opening tag
            if let Some(caps) = re_opening_tag().captures(trimmed) {
                let tag_name = caps.get(1).map_or("", |m| m.as_str());
                let attrs = caps.get(2).map_or("", |m| m.as_str());
                let prefix = tree_renderer::tree_guides(depth);

                // Collapse deep elements — skip all children until matching close tag.
                if depth >= self.config.max_depth_expanded {
                    let summary = format!("<{tag_name}>...</{tag_name}>");
                    push_line(
                        &mut lines,
                        &mut line_mapping,
                        vec![
                            guide_segment(&prefix, theme),
                            StyledSegment {
                                text: summary,
                                fg: Some(theme.dim_color()),
                                italic: true,
                                ..Default::default()
                            },
                        ],
                        Some(line_idx),
                    );
                    // Enter skip mode: track nested depth until we return to
                    // current depth (the matching closing tag brings us back).
                    skip_until_depth = Some(depth);
                    depth += 1; // account for this opening tag
                    continue;
                }

                let mut segments = vec![guide_segment(&prefix, theme), dim_segment("<", theme)];
                segments.extend(self.style_tag_name(tag_name, theme));
                if !attrs.is_empty() {
                    segments.extend(self.style_attributes(attrs, theme));
                }
                segments.push(dim_segment(">", theme));

                push_line(&mut lines, &mut line_mapping, segments, Some(line_idx));
                depth += 1;
                continue;
            }

            // Text content or unrecognized line
            let prefix = tree_renderer::tree_guides(depth);
            push_line(
                &mut lines,
                &mut line_mapping,
                vec![
                    guide_segment(&prefix, theme),
                    StyledSegment {
                        text: trimmed.to_string(),
                        fg: Some(theme.palette[7]), // White for text content
                        ..Default::default()
                    },
                ],
                Some(line_idx),
            );
        }

        Ok(RenderedContent {
            lines,
            line_mapping,
            graphics: vec![],
            format_badge: "XML".to_string(),
        })
    }

    fn format_badge(&self) -> &str {
        "XML"
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the XML renderer with the registry.
pub fn register_xml_renderer(registry: &mut RendererRegistry, config: &XmlRendererConfig) {
    registry.register_renderer("xml", Box::new(XmlRenderer::new(config.clone())));
}

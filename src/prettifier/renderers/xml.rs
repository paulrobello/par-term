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

use super::{push_line, tree_renderer};
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::traits::{ContentRenderer, RenderError, RendererConfig, ThemeColors};
use crate::prettifier::types::{ContentBlock, RenderedContent, RendererCapability, StyledSegment};

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
                    fg: Some(theme.palette[8]), // Dim
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
                    fg: Some(theme.palette[8]),
                    ..Default::default()
                });
                segments.push(StyledSegment {
                    text: local.to_string(),
                    fg: Some(theme.palette[6]), // Cyan for attribute name
                    ..Default::default()
                });
            } else {
                segments.push(StyledSegment {
                    text: attr_name.to_string(),
                    fg: Some(theme.palette[6]), // Cyan for attribute name
                    ..Default::default()
                });
            }

            segments.push(dim_segment("=", theme));

            segments.push(StyledSegment {
                text: attr_value.to_string(),
                fg: Some(theme.palette[2]), // Green for attribute value
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
// Helper functions
// ---------------------------------------------------------------------------

fn guide_segment(prefix: &str, theme: &ThemeColors) -> StyledSegment {
    StyledSegment {
        text: prefix.to_string(),
        fg: Some(theme.palette[8]),
        ..Default::default()
    }
}

fn dim_segment(text: &str, theme: &ThemeColors) -> StyledSegment {
    StyledSegment {
        text: text.to_string(),
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
                            fg: Some(theme.palette[8]), // Dim for processing instructions
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
                            fg: Some(theme.palette[8]),
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
                            fg: Some(theme.palette[8]), // Dim for comments
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
                                fg: Some(theme.palette[8]),
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::testing::test_renderer_config;
    use crate::prettifier::traits::RendererConfig;
    use crate::prettifier::types::{ContentBlock, StyledLine};
    use std::time::SystemTime;

    fn test_config() -> RendererConfig {
        test_renderer_config()
    }

    fn renderer() -> XmlRenderer {
        XmlRenderer::new(XmlRendererConfig::default())
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
        assert_eq!(r.format_id(), "xml");
        assert_eq!(r.display_name(), "XML/HTML");
        assert_eq!(r.format_badge(), "XML");
        assert_eq!(r.capabilities(), vec![RendererCapability::TextStyling]);
    }

    #[test]
    fn test_render_simple_xml() {
        let r = renderer();
        let block = make_block(&[
            "<root>",
            "  <child>",
            "    content",
            "  </child>",
            "</root>",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("root"));
        assert!(text.contains("child"));
        assert!(text.contains("content"));
        assert_eq!(result.format_badge, "XML");
    }

    #[test]
    fn test_render_xml_declaration() {
        let r = renderer();
        let block = make_block(&["<?xml version=\"1.0\" encoding=\"UTF-8\"?>"]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("<?xml"));
    }

    #[test]
    fn test_render_attributes() {
        let r = renderer();
        let block = make_block(&["<item name=\"test\" id=\"1\">value</item>"]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("name"));
        assert!(text.contains("\"test\""));
        assert!(text.contains("id"));
    }

    #[test]
    fn test_attribute_highlighting() {
        let r = renderer();
        let block = make_block(&["<tag attr=\"value\">"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();

        // Find the attribute name segment
        let attr_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text == "attr")
            .unwrap();
        assert_eq!(attr_seg.fg, Some(theme.palette[6])); // Cyan

        // Find the attribute value segment
        let val_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text == "\"value\"")
            .unwrap();
        assert_eq!(val_seg.fg, Some(theme.palette[2])); // Green
    }

    #[test]
    fn test_namespace_coloring() {
        let r = renderer();
        let block = make_block(&["<ns:tag>content</ns:tag>"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();

        // Namespace prefix in magenta
        let ns_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text == "ns")
            .unwrap();
        assert_eq!(ns_seg.fg, Some(theme.palette[5])); // Magenta

        // Tag name in blue
        let tag_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text == "tag" && s.bold)
            .unwrap();
        assert_eq!(tag_seg.fg, Some(theme.palette[4])); // Blue
    }

    #[test]
    fn test_self_closing_tag() {
        let r = renderer();
        let block = make_block(&["<root>", "  <br />", "</root>"]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("br"));
        assert!(text.contains("/>"));
    }

    #[test]
    fn test_comment_styling() {
        let r = renderer();
        let block = make_block(&["<!-- This is a comment -->"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();

        let comment_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains("<!-- This is a comment -->"))
            .unwrap();
        assert_eq!(comment_seg.fg, Some(theme.palette[8]));
        assert!(comment_seg.italic);
    }

    #[test]
    fn test_cdata_styling() {
        let r = renderer();
        let block = make_block(&["<![CDATA[some data]]>"]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();

        let cdata_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains("CDATA"))
            .unwrap();
        assert_eq!(cdata_seg.fg, Some(theme.palette[3])); // Yellow
    }

    #[test]
    fn test_tree_guides_present() {
        let r = renderer();
        let block = make_block(&["<root>", "  <child>text</child>", "</root>"]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains('│'));
    }

    #[test]
    fn test_auto_collapse_deep_elements() {
        let r = XmlRenderer::new(XmlRendererConfig {
            max_depth_expanded: 1,
        });
        let block = make_block(&[
            "<root>",
            "  <level1>",
            "    <deep>content</deep>",
            "  </level1>",
            "</root>",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("..."));
    }

    #[test]
    fn test_line_mappings_populated() {
        let r = renderer();
        let block = make_block(&["<root>", "  <child />", "</root>"]);
        let result = r.render(&block, &test_config()).unwrap();
        assert_eq!(result.line_mapping.len(), result.lines.len());
    }

    #[test]
    fn test_register_xml_renderer() {
        let mut registry = RendererRegistry::new(0.6);
        register_xml_renderer(&mut registry, &XmlRendererConfig::default());
        assert_eq!(registry.renderer_count(), 1);
        assert!(registry.get_renderer("xml").is_some());
        assert_eq!(
            registry.get_renderer("xml").unwrap().display_name(),
            "XML/HTML"
        );
    }

    #[test]
    fn test_config_defaults() {
        let config = XmlRendererConfig::default();
        assert_eq!(config.max_depth_expanded, 4);
    }

    #[test]
    fn test_empty_content() {
        let r = renderer();
        let block = make_block(&[]);
        let result = r.render(&block, &test_config()).unwrap();
        assert!(result.lines.is_empty());
    }

    #[test]
    fn test_doctype_rendering() {
        let r = renderer();
        let block = make_block(&["<!DOCTYPE html>", "<html>", "</html>"]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("DOCTYPE"));
    }
}

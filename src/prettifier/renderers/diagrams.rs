//! Diagram renderer for fenced code blocks tagged with diagram language identifiers.
//!
//! Converts fenced code blocks tagged with diagram language identifiers
//! (Mermaid, PlantUML, GraphViz, D2, etc.) into styled fallback output.
//! When rendering backends (local CLI tools, Kroki API) are unavailable,
//! falls back to syntax-highlighted source display with a format badge.

use std::collections::HashMap;

use crate::config::prettifier::DiagramRendererConfig;
use crate::prettifier::traits::{ContentRenderer, RenderError, RendererConfig};
use crate::prettifier::types::{
    ContentBlock, RenderedContent, RendererCapability, SourceLineMapping, StyledLine, StyledSegment,
};

/// A supported diagram language with rendering metadata.
#[derive(Debug, Clone)]
pub struct DiagramLanguage {
    /// The fenced code block tag (e.g., "mermaid", "plantuml", "dot").
    pub tag: String,
    /// Display name (e.g., "Mermaid", "PlantUML").
    pub display_name: String,
    /// Kroki API type identifier (if supported by Kroki).
    pub kroki_type: Option<String>,
    /// Local CLI command to render this language.
    pub local_command: Option<String>,
    /// Arguments for local CLI command.
    pub local_args: Vec<String>,
}

/// Renders fenced code blocks with diagram language tags.
///
/// Currently provides syntax-highlighted fallback rendering. The infrastructure
/// for local CLI and Kroki API backends is defined but requires async support
/// in the rendering pipeline to be fully operational.
pub struct DiagramRenderer {
    config: DiagramRendererConfig,
    /// Registry of supported diagram languages, keyed by tag.
    languages: HashMap<String, DiagramLanguage>,
}

impl DiagramRenderer {
    /// Create a new diagram renderer with the given config.
    pub fn new(config: DiagramRendererConfig) -> Self {
        let mut languages = HashMap::new();
        for lang in default_diagram_languages() {
            languages.insert(lang.tag.clone(), lang);
        }
        Self { config, languages }
    }

    /// Check if a language tag is a known diagram language.
    pub fn is_diagram_language(&self, tag: &str) -> bool {
        self.languages.contains_key(tag)
    }

    /// Get the language config for a tag.
    pub fn get_language(&self, tag: &str) -> Option<&DiagramLanguage> {
        self.languages.get(tag)
    }

    /// Add a custom diagram language to the registry.
    pub fn add_language(&mut self, lang: DiagramLanguage) {
        self.languages.insert(lang.tag.clone(), lang);
    }

    /// Get the configured rendering backend.
    pub fn backend(&self) -> &str {
        self.config.engine.as_deref().unwrap_or("auto")
    }

    /// Parse a content block into diagram sections.
    ///
    /// Extracts diagram fenced code blocks and surrounding text,
    /// returning a list of sections for rendering.
    fn parse_diagram_blocks<'a>(&self, lines: &'a [String]) -> Vec<DiagramSection<'a>> {
        let mut sections = Vec::new();
        let mut i = 0;

        while i < lines.len() {
            let line = &lines[i];

            // Check for opening fence with diagram tag.
            if let Some(tag) = self.extract_diagram_tag(line) {
                let start = i;
                i += 1;
                let mut source_lines = Vec::new();

                // Collect lines until closing fence.
                while i < lines.len() && !lines[i].starts_with("```") {
                    source_lines.push(lines[i].as_str());
                    i += 1;
                }

                // Skip closing fence.
                if i < lines.len() {
                    i += 1;
                }

                sections.push(DiagramSection::Diagram {
                    tag,
                    source_lines,
                    start_line: start,
                });
            } else {
                // Plain text line.
                sections.push(DiagramSection::Text {
                    line: &lines[i],
                    source_line: i,
                });
                i += 1;
            }
        }

        sections
    }

    /// Extract a diagram language tag from a fenced code block opening line.
    fn extract_diagram_tag<'a>(&self, line: &'a str) -> Option<&'a str> {
        let trimmed = line.trim();
        let rest = trimmed.strip_prefix("```")?;
        let tag = rest.trim();
        if !tag.is_empty() && self.is_diagram_language(tag) {
            Some(tag)
        } else {
            None
        }
    }

    /// Render a diagram section as styled fallback text.
    fn render_diagram_fallback(
        &self,
        tag: &str,
        source_lines: &[&str],
        start_line: usize,
        theme: &RendererConfig,
    ) -> (Vec<StyledLine>, Vec<SourceLineMapping>) {
        let mut styled = Vec::new();
        let mut mappings = Vec::new();
        let lang = self.get_language(tag);
        let display_name = lang.map_or(tag, |l| l.display_name.as_str());

        // Header line with diagram type badge.
        let header_line = StyledLine::new(vec![
            StyledSegment {
                text: format!(" {display_name} "),
                fg: Some(theme.theme_colors.bg),
                bg: Some(theme.theme_colors.palette[4]), // Blue background
                bold: true,
                ..Default::default()
            },
            StyledSegment {
                text: " (source)".to_string(),
                fg: Some(theme.theme_colors.palette[8]), // Dark grey
                ..Default::default()
            },
        ]);
        mappings.push(SourceLineMapping {
            rendered_line: styled.len(),
            source_line: Some(start_line),
        });
        styled.push(header_line);

        // Source lines with syntax-like coloring.
        let comment_color = theme.theme_colors.palette[8]; // Dark grey
        let keyword_color = theme.theme_colors.palette[6]; // Cyan
        let string_color = theme.theme_colors.palette[2]; // Green

        for (idx, source_line) in source_lines.iter().enumerate() {
            let source_idx = start_line + 1 + idx; // +1 to skip opening fence
            let line =
                render_diagram_source_line(source_line, comment_color, keyword_color, string_color);
            mappings.push(SourceLineMapping {
                rendered_line: styled.len(),
                source_line: Some(source_idx),
            });
            styled.push(line);
        }

        (styled, mappings)
    }
}

/// A parsed section of content — either a diagram block or plain text.
enum DiagramSection<'a> {
    /// A diagram fenced code block.
    Diagram {
        tag: &'a str,
        source_lines: Vec<&'a str>,
        start_line: usize,
    },
    /// A regular text line.
    Text { line: &'a str, source_line: usize },
}

impl ContentRenderer for DiagramRenderer {
    fn format_id(&self) -> &str {
        "diagrams"
    }

    fn display_name(&self) -> &str {
        "Diagrams"
    }

    fn capabilities(&self) -> Vec<RendererCapability> {
        vec![
            RendererCapability::TextStyling,
            RendererCapability::InlineGraphics,
            RendererCapability::ExternalCommand,
            RendererCapability::NetworkAccess,
        ]
    }

    fn render(
        &self,
        content: &ContentBlock,
        config: &RendererConfig,
    ) -> Result<RenderedContent, RenderError> {
        let sections = self.parse_diagram_blocks(&content.lines);

        let mut all_lines = Vec::new();
        let mut all_mappings = Vec::new();

        for section in sections {
            match section {
                DiagramSection::Diagram {
                    tag,
                    source_lines,
                    start_line,
                } => {
                    let (lines, mappings) =
                        self.render_diagram_fallback(tag, &source_lines, start_line, config);
                    // Adjust mapping indices for the current offset.
                    for mut mapping in mappings {
                        mapping.rendered_line += all_lines.len();
                        all_mappings.push(mapping);
                    }
                    all_lines.extend(lines);
                }
                DiagramSection::Text { line, source_line } => {
                    all_mappings.push(SourceLineMapping {
                        rendered_line: all_lines.len(),
                        source_line: Some(source_line),
                    });
                    all_lines.push(StyledLine::plain(line));
                }
            }
        }

        Ok(RenderedContent {
            lines: all_lines,
            line_mapping: all_mappings,
            graphics: vec![],
            format_badge: "DG".to_string(),
        })
    }

    fn format_badge(&self) -> &str {
        "DG"
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the diagram renderer with the registry.
pub fn register_diagram_renderer(
    registry: &mut crate::prettifier::registry::RendererRegistry,
    config: &DiagramRendererConfig,
) {
    registry.register_renderer("diagrams", Box::new(DiagramRenderer::new(config.clone())));
}

/// Render a single diagram source line with basic syntax highlighting.
fn render_diagram_source_line(
    line: &str,
    comment_color: [u8; 3],
    keyword_color: [u8; 3],
    string_color: [u8; 3],
) -> StyledLine {
    let trimmed = line.trim();

    // Simple heuristic coloring for common diagram patterns.
    if trimmed.starts_with("%%") || trimmed.starts_with("//") || trimmed.starts_with('#') {
        // Comment line.
        StyledLine::new(vec![StyledSegment {
            text: format!("  {line}"),
            fg: Some(comment_color),
            italic: true,
            ..Default::default()
        }])
    } else if trimmed.starts_with('@')
        || trimmed.starts_with("graph ")
        || trimmed.starts_with("digraph ")
        || trimmed.starts_with("subgraph ")
        || trimmed.starts_with("sequenceDiagram")
        || trimmed.starts_with("classDiagram")
        || trimmed.starts_with("flowchart ")
        || trimmed.starts_with("erDiagram")
        || trimmed.starts_with("gantt")
        || trimmed.starts_with("pie ")
        || trimmed.starts_with("stateDiagram")
    {
        // Keyword/directive line.
        StyledLine::new(vec![StyledSegment {
            text: format!("  {line}"),
            fg: Some(keyword_color),
            bold: true,
            ..Default::default()
        }])
    } else if trimmed.contains('"') {
        // Line with strings — highlight the whole line in string color.
        StyledLine::new(vec![StyledSegment {
            text: format!("  {line}"),
            fg: Some(string_color),
            ..Default::default()
        }])
    } else {
        // Default styling.
        StyledLine::new(vec![StyledSegment {
            text: format!("  {line}"),
            fg: None,
            ..Default::default()
        }])
    }
}

/// Return the default set of diagram languages.
pub fn default_diagram_languages() -> Vec<DiagramLanguage> {
    vec![
        DiagramLanguage {
            tag: "mermaid".into(),
            display_name: "Mermaid".into(),
            kroki_type: Some("mermaid".into()),
            local_command: Some("mmdc".into()),
            local_args: vec![
                "-i".into(),
                "/dev/stdin".into(),
                "-o".into(),
                "/dev/stdout".into(),
                "-e".into(),
                "png".into(),
            ],
        },
        DiagramLanguage {
            tag: "plantuml".into(),
            display_name: "PlantUML".into(),
            kroki_type: Some("plantuml".into()),
            local_command: Some("plantuml".into()),
            local_args: vec!["-tpng".into(), "-pipe".into()],
        },
        DiagramLanguage {
            tag: "graphviz".into(),
            display_name: "GraphViz".into(),
            kroki_type: Some("graphviz".into()),
            local_command: Some("dot".into()),
            local_args: vec!["-Tpng".into()],
        },
        DiagramLanguage {
            tag: "dot".into(),
            display_name: "GraphViz".into(),
            kroki_type: Some("graphviz".into()),
            local_command: Some("dot".into()),
            local_args: vec!["-Tpng".into()],
        },
        DiagramLanguage {
            tag: "d2".into(),
            display_name: "D2".into(),
            kroki_type: Some("d2".into()),
            local_command: Some("d2".into()),
            local_args: vec!["-".into(), "-".into()],
        },
        DiagramLanguage {
            tag: "ditaa".into(),
            display_name: "Ditaa".into(),
            kroki_type: Some("ditaa".into()),
            local_command: None,
            local_args: vec![],
        },
        DiagramLanguage {
            tag: "svgbob".into(),
            display_name: "SvgBob".into(),
            kroki_type: Some("svgbob".into()),
            local_command: Some("svgbob".into()),
            local_args: vec![],
        },
        DiagramLanguage {
            tag: "erd".into(),
            display_name: "Erd".into(),
            kroki_type: Some("erd".into()),
            local_command: Some("erd".into()),
            local_args: vec![],
        },
        DiagramLanguage {
            tag: "vegalite".into(),
            display_name: "Vega-Lite".into(),
            kroki_type: Some("vegalite".into()),
            local_command: None,
            local_args: vec![],
        },
        DiagramLanguage {
            tag: "wavedrom".into(),
            display_name: "WaveDrom".into(),
            kroki_type: Some("wavedrom".into()),
            local_command: None,
            local_args: vec![],
        },
        DiagramLanguage {
            tag: "excalidraw".into(),
            display_name: "Excalidraw".into(),
            kroki_type: Some("excalidraw".into()),
            local_command: None,
            local_args: vec![],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::traits::RendererConfig;
    use std::time::SystemTime;

    fn test_config() -> DiagramRendererConfig {
        DiagramRendererConfig::default()
    }

    fn test_renderer() -> DiagramRenderer {
        DiagramRenderer::new(test_config())
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

    #[test]
    fn test_is_diagram_language() {
        let renderer = test_renderer();
        assert!(renderer.is_diagram_language("mermaid"));
        assert!(renderer.is_diagram_language("plantuml"));
        assert!(renderer.is_diagram_language("graphviz"));
        assert!(renderer.is_diagram_language("dot"));
        assert!(renderer.is_diagram_language("d2"));
        assert!(renderer.is_diagram_language("ditaa"));
        assert!(renderer.is_diagram_language("svgbob"));
        assert!(renderer.is_diagram_language("erd"));
        assert!(renderer.is_diagram_language("vegalite"));
        assert!(renderer.is_diagram_language("wavedrom"));
        assert!(renderer.is_diagram_language("excalidraw"));
        assert!(!renderer.is_diagram_language("rust"));
        assert!(!renderer.is_diagram_language("python"));
    }

    #[test]
    fn test_all_ten_languages_registered() {
        let renderer = test_renderer();
        // 11 tags covering 10 unique languages (graphviz + dot both map to GraphViz).
        assert_eq!(renderer.languages.len(), 11);
    }

    #[test]
    fn test_format_id() {
        let renderer = test_renderer();
        assert_eq!(ContentRenderer::format_id(&renderer), "diagrams");
    }

    #[test]
    fn test_display_name() {
        let renderer = test_renderer();
        assert_eq!(ContentRenderer::display_name(&renderer), "Diagrams");
    }

    #[test]
    fn test_capabilities() {
        let renderer = test_renderer();
        let caps = renderer.capabilities();
        assert!(caps.contains(&RendererCapability::InlineGraphics));
        assert!(caps.contains(&RendererCapability::NetworkAccess));
        assert!(caps.contains(&RendererCapability::ExternalCommand));
        assert!(caps.contains(&RendererCapability::TextStyling));
    }

    #[test]
    fn test_format_badge() {
        let renderer = test_renderer();
        assert_eq!(ContentRenderer::format_badge(&renderer), "DG");
    }

    #[test]
    fn test_render_mermaid_fallback() {
        let renderer = test_renderer();
        let block = make_block(&["```mermaid", "graph TD", "  A-->B", "```"]);
        let config = RendererConfig::default();

        let result = renderer.render(&block, &config).unwrap();
        assert_eq!(result.format_badge, "DG");
        // Should have header + 2 source lines = 3 rendered lines.
        assert_eq!(result.lines.len(), 3);
        // First line should mention "Mermaid".
        let first_text: String = result.lines[0]
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(first_text.contains("Mermaid"));
    }

    #[test]
    fn test_render_plantuml_fallback() {
        let renderer = test_renderer();
        let block = make_block(&[
            "```plantuml",
            "@startuml",
            "Alice -> Bob: Hello",
            "@enduml",
            "```",
        ]);
        let config = RendererConfig::default();

        let result = renderer.render(&block, &config).unwrap();
        assert_eq!(result.lines.len(), 4); // header + 3 source lines
        let first_text: String = result.lines[0]
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(first_text.contains("PlantUML"));
    }

    #[test]
    fn test_render_with_surrounding_text() {
        let renderer = test_renderer();
        let block = make_block(&[
            "Here is a diagram:",
            "```mermaid",
            "graph LR",
            "  A-->B",
            "```",
            "End of content.",
        ]);
        let config = RendererConfig::default();

        let result = renderer.render(&block, &config).unwrap();
        // Line 0: "Here is a diagram:" (plain text)
        // Line 1: header (Mermaid badge)
        // Line 2: "graph LR" (source)
        // Line 3: "  A-->B" (source)
        // Line 4: "End of content." (plain text)
        assert_eq!(result.lines.len(), 5);
        assert_eq!(result.lines[0].segments[0].text, "Here is a diagram:");
        assert_eq!(
            result.lines[result.lines.len() - 1].segments[0].text,
            "End of content."
        );
    }

    #[test]
    fn test_render_empty_diagram() {
        let renderer = test_renderer();
        let block = make_block(&["```mermaid", "```"]);
        let config = RendererConfig::default();

        let result = renderer.render(&block, &config).unwrap();
        // Just the header, no source lines.
        assert_eq!(result.lines.len(), 1);
    }

    #[test]
    fn test_render_non_diagram_content() {
        let renderer = test_renderer();
        let block = make_block(&["Just some plain text", "Nothing special"]);
        let config = RendererConfig::default();

        let result = renderer.render(&block, &config).unwrap();
        // All lines are plain text pass-through.
        assert_eq!(result.lines.len(), 2);
        assert_eq!(result.lines[0].segments[0].text, "Just some plain text");
    }

    #[test]
    fn test_line_mappings() {
        let renderer = test_renderer();
        let block = make_block(&["```mermaid", "graph TD", "  A-->B", "```"]);
        let config = RendererConfig::default();

        let result = renderer.render(&block, &config).unwrap();
        assert_eq!(result.line_mapping.len(), 3);
        // Header maps to source line 0 (the opening fence).
        assert_eq!(result.line_mapping[0].source_line, Some(0));
        // First source line maps to source line 1.
        assert_eq!(result.line_mapping[1].source_line, Some(1));
        // Second source line maps to source line 2.
        assert_eq!(result.line_mapping[2].source_line, Some(2));
    }

    #[test]
    fn test_syntax_highlight_comments() {
        let renderer = test_renderer();
        let block = make_block(&["```mermaid", "%% This is a comment", "graph TD", "```"]);
        let config = RendererConfig::default();

        let result = renderer.render(&block, &config).unwrap();
        // Comment line (index 1) should be italic.
        assert!(result.lines[1].segments[0].italic);
    }

    #[test]
    fn test_syntax_highlight_keywords() {
        let renderer = test_renderer();
        let block = make_block(&["```mermaid", "graph TD", "  A-->B", "```"]);
        let config = RendererConfig::default();

        let result = renderer.render(&block, &config).unwrap();
        // Keyword line (index 1 = "graph TD") should be bold.
        assert!(result.lines[1].segments[0].bold);
    }

    #[test]
    fn test_backend_default() {
        let renderer = test_renderer();
        assert_eq!(renderer.backend(), "auto");
    }

    #[test]
    fn test_backend_custom() {
        let config = DiagramRendererConfig {
            engine: Some("kroki".into()),
            ..DiagramRendererConfig::default()
        };
        let renderer = DiagramRenderer::new(config);
        assert_eq!(renderer.backend(), "kroki");
    }

    #[test]
    fn test_get_language() {
        let renderer = test_renderer();
        let lang = renderer.get_language("mermaid").unwrap();
        assert_eq!(lang.display_name, "Mermaid");
        assert_eq!(lang.kroki_type.as_deref(), Some("mermaid"));
        assert_eq!(lang.local_command.as_deref(), Some("mmdc"));

        assert!(renderer.get_language("nonexistent").is_none());
    }

    #[test]
    fn test_graphviz_and_dot_share_config() {
        let renderer = test_renderer();
        let gv = renderer.get_language("graphviz").unwrap();
        let dot = renderer.get_language("dot").unwrap();
        assert_eq!(gv.display_name, "GraphViz");
        assert_eq!(dot.display_name, "GraphViz");
        assert_eq!(gv.local_command, dot.local_command);
    }

    #[test]
    fn test_default_languages_have_kroki_type() {
        for lang in default_diagram_languages() {
            assert!(
                lang.kroki_type.is_some(),
                "Language {} should have a kroki_type",
                lang.tag
            );
        }
    }

    #[test]
    fn test_multiple_diagram_blocks() {
        let renderer = test_renderer();
        let block = make_block(&[
            "```mermaid",
            "graph TD",
            "```",
            "Some text between",
            "```d2",
            "x -> y",
            "```",
        ]);
        let config = RendererConfig::default();

        let result = renderer.render(&block, &config).unwrap();
        // Mermaid: header + 1 source = 2
        // Text: 1
        // D2: header + 1 source = 2
        // Total: 5
        assert_eq!(result.lines.len(), 5);

        // Verify both diagram headers are present.
        let all_text: String = result
            .lines
            .iter()
            .flat_map(|l| l.segments.iter())
            .map(|s| s.text.as_str())
            .collect();
        assert!(all_text.contains("Mermaid"));
        assert!(all_text.contains("D2"));
    }
}

//! Tests for the diagram renderer.

#[cfg(test)]
mod tests {
    use crate::config::prettifier::DiagramRendererConfig;
    use crate::prettifier::traits::{ContentRenderer, RendererConfig, ThemeColors};
    use crate::prettifier::types::{ContentBlock, RendererCapability};
    use std::time::SystemTime;

    use super::super::languages::default_diagram_languages;
    use super::super::renderer::DiagramRenderer;
    use super::super::svg_utils::svg_to_png_bytes;

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
    fn test_render_mermaid_native() {
        // With "auto" backend, mermaid now renders natively.
        let renderer = test_renderer();
        let block = make_block(&["```mermaid", "graph TD", "  A-->B", "```"]);
        let config = RendererConfig::default();

        let result = renderer.render(&block, &config).unwrap();
        assert_eq!(result.format_badge, "DG");
        // Should have header + placeholder rows for the rendered image.
        assert!(
            result.lines.len() >= 2,
            "Expected at least header + placeholder rows"
        );
        // First line should mention "Mermaid" and "(rendered)".
        let first_text: String = result.lines[0]
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(first_text.contains("Mermaid"));
        assert!(first_text.contains("rendered"));
        // Should have an InlineGraphic with pre-decoded RGBA data.
        assert_eq!(result.graphics.len(), 1);
        assert!(!result.graphics[0].data.is_empty());
        assert!(result.graphics[0].is_rgba);
        assert!(result.graphics[0].pixel_width > 0);
        assert!(result.graphics[0].pixel_height > 0);
    }

    #[test]
    fn test_render_mermaid_text_fallback() {
        // With "text_fallback" engine, mermaid should produce source-style output.
        let config = DiagramRendererConfig {
            engine: Some("text_fallback".into()),
            ..DiagramRendererConfig::default()
        };
        let renderer = DiagramRenderer::new(config);
        let block = make_block(&["```mermaid", "graph TD", "  A-->B", "```"]);
        let rc = RendererConfig::default();

        let result = renderer.render(&block, &rc).unwrap();
        assert_eq!(result.format_badge, "DG");
        assert_eq!(result.lines.len(), 3);
        let first_text: String = result.lines[0]
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(first_text.contains("Mermaid"));
        assert!(first_text.contains("source"));
        assert!(result.graphics.is_empty());
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
        assert!(
            result.lines.len() >= 3,
            "Expected at least text + header + text"
        );
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
        assert!(!result.lines.is_empty());
    }

    #[test]
    fn test_render_non_diagram_content() {
        let renderer = test_renderer();
        let block = make_block(&["Just some plain text", "Nothing special"]);
        let config = RendererConfig::default();

        let result = renderer.render(&block, &config).unwrap();
        assert_eq!(result.lines.len(), 2);
        assert_eq!(result.lines[0].segments[0].text, "Just some plain text");
    }

    #[test]
    fn test_line_mappings() {
        let renderer = test_renderer();
        let block = make_block(&["```mermaid", "graph TD", "  A-->B", "```"]);
        let config = RendererConfig::default();

        let result = renderer.render(&block, &config).unwrap();
        assert!(!result.line_mapping.is_empty());
        assert_eq!(result.line_mapping[0].source_line, Some(0));
        assert_eq!(result.line_mapping.len(), result.lines.len());
    }

    #[test]
    fn test_syntax_highlight_comments() {
        let config = DiagramRendererConfig {
            engine: Some("text_fallback".into()),
            ..DiagramRendererConfig::default()
        };
        let renderer = DiagramRenderer::new(config);
        let block = make_block(&["```mermaid", "%% This is a comment", "graph TD", "```"]);
        let rc = RendererConfig::default();

        let result = renderer.render(&block, &rc).unwrap();
        assert!(result.lines[1].segments[0].italic);
    }

    #[test]
    fn test_syntax_highlight_keywords() {
        let config = DiagramRendererConfig {
            engine: Some("text_fallback".into()),
            ..DiagramRendererConfig::default()
        };
        let renderer = DiagramRenderer::new(config);
        let block = make_block(&["```mermaid", "graph TD", "  A-->B", "```"]);
        let rc = RendererConfig::default();

        let result = renderer.render(&block, &rc).unwrap();
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
    fn test_text_fallback_engine() {
        let config = DiagramRendererConfig {
            engine: Some("text_fallback".into()),
            ..DiagramRendererConfig::default()
        };
        let renderer = DiagramRenderer::new(config);
        let block = make_block(&["```mermaid", "graph TD", "```"]);
        let result = renderer.render(&block, &RendererConfig::default()).unwrap();
        let first_text: String = result.lines[0]
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(first_text.contains("source"));
        assert!(result.graphics.is_empty());
    }

    #[test]
    fn test_auto_backend_falls_back_to_text() {
        let config = DiagramRendererConfig {
            engine: None,
            ..DiagramRendererConfig::default()
        };
        let renderer = DiagramRenderer::new(config);
        let block = make_block(&["```ditaa", "+---+", "| A |", "+---+", "```"]);
        let result = renderer.render(&block, &RendererConfig::default()).unwrap();
        assert!(!result.lines.is_empty());
    }

    #[test]
    fn test_try_local_cli_missing_command() {
        use super::super::languages::DiagramLanguage;
        let renderer = test_renderer();
        let lang = DiagramLanguage {
            tag: "test".into(),
            display_name: "Test".into(),
            kroki_type: None,
            local_command: Some("nonexistent_command_12345".into()),
            local_args: vec![],
        };
        assert!(renderer.try_local_cli(&lang, "test").is_none());
    }

    #[test]
    fn test_try_local_cli_no_command() {
        use super::super::languages::DiagramLanguage;
        let renderer = test_renderer();
        let lang = DiagramLanguage {
            tag: "test".into(),
            display_name: "Test".into(),
            kroki_type: None,
            local_command: None,
            local_args: vec![],
        };
        assert!(renderer.try_local_cli(&lang, "test").is_none());
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
        assert_eq!(result.lines.len(), 5);

        let all_text: String = result
            .lines
            .iter()
            .flat_map(|l| l.segments.iter())
            .map(|s| s.text.as_str())
            .collect();
        assert!(all_text.contains("Mermaid"));
        assert!(all_text.contains("D2"));
        assert_eq!(result.graphics.len(), 1);
    }

    // ===================================================================
    // Native mermaid rendering tests
    // ===================================================================

    #[test]
    fn test_native_mermaid_basic_flowchart() {
        let renderer = test_renderer();
        let colors = ThemeColors::default();
        let source = "graph TD\n  A-->B\n  B-->C";
        let result = renderer.try_native_mermaid("mermaid", source, &colors);
        assert!(
            result.is_some(),
            "Native mermaid should render a basic flowchart"
        );
        let png = result.unwrap();
        assert!(png.len() > 8);
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn test_native_mermaid_handles_garbage_gracefully() {
        let renderer = test_renderer();
        let colors = ThemeColors::default();
        let _ = renderer.try_native_mermaid("mermaid", "", &colors);
        let _ = renderer.try_native_mermaid("mermaid", "this is not valid mermaid %%!@#$", &colors);
        let _ = renderer.try_native_mermaid("mermaid", "\x00\x01\x02", &colors);
    }

    #[test]
    fn test_native_mermaid_sequence_diagram() {
        let renderer = test_renderer();
        let colors = ThemeColors::default();
        let source = "sequenceDiagram\n  Alice->>Bob: Hello\n  Bob-->>Alice: Hi";
        let result = renderer.try_native_mermaid("mermaid", source, &colors);
        assert!(
            result.is_some(),
            "Native mermaid should render a sequence diagram"
        );
        let png = result.unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn test_native_mermaid_non_mermaid_tag() {
        let renderer = test_renderer();
        let colors = ThemeColors::default();
        assert!(
            renderer
                .try_native_mermaid("plantuml", "anything", &colors)
                .is_none()
        );
        assert!(
            renderer
                .try_native_mermaid("dot", "anything", &colors)
                .is_none()
        );
    }

    #[test]
    fn test_svg_to_png_bytes_basic() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
            <rect width="100" height="100" fill="red"/>
        </svg>"#;
        let result = svg_to_png_bytes(svg, None);
        assert!(result.is_some(), "Simple SVG should convert to PNG");
        let png = result.unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn test_svg_to_png_bytes_invalid() {
        let result = svg_to_png_bytes("this is not svg at all", None);
        assert!(result.is_none(), "Invalid SVG should return None");
    }

    #[test]
    fn test_native_engine_explicit() {
        let config = DiagramRendererConfig {
            engine: Some("native".into()),
            ..DiagramRendererConfig::default()
        };
        let renderer = DiagramRenderer::new(config);
        let block = make_block(&["```mermaid", "graph TD", "  A-->B", "```"]);
        let result = renderer.render(&block, &RendererConfig::default()).unwrap();
        let first_text: String = result.lines[0]
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(first_text.contains("rendered"));
        assert!(!result.graphics.is_empty());
    }

    #[test]
    fn test_native_engine_non_mermaid_falls_back() {
        let config = DiagramRendererConfig {
            engine: Some("native".into()),
            ..DiagramRendererConfig::default()
        };
        let renderer = DiagramRenderer::new(config);
        let block = make_block(&["```plantuml", "@startuml", "Alice -> Bob", "@enduml", "```"]);
        let result = renderer.render(&block, &RendererConfig::default()).unwrap();
        let first_text: String = result.lines[0]
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(first_text.contains("source"));
        assert!(result.graphics.is_empty());
    }
}

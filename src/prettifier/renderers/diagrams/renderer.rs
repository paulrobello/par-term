//! DiagramRenderer: core rendering logic for diagram fenced code blocks.
//!
//! Handles parsing, backend dispatch (native/local/kroki/fallback),
//! and rendering of diagram sections within content blocks.

use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::Arc;

use crate::config::prettifier::DiagramRendererConfig;
use crate::prettifier::traits::{ContentRenderer, RenderError, RendererConfig, ThemeColors};
use crate::prettifier::types::{
    ContentBlock, InlineGraphic, RenderedContent, RendererCapability, SourceLineMapping,
    StyledLine, StyledSegment,
};

use super::languages::{DiagramLanguage, default_diagram_languages};
use super::svg_utils::{dark_mermaid_theme, svg_to_png_bytes};

/// Default Kroki server URL when none is configured.
const DEFAULT_KROKI_SERVER: &str = "https://kroki.io";

/// Renders fenced code blocks with diagram language tags.
///
/// Supports three rendering backends: local CLI tools, Kroki API, and text
/// fallback. Backend selection is controlled by the `engine` config field.
pub struct DiagramRenderer {
    pub(super) config: DiagramRendererConfig,
    /// Registry of supported diagram languages, keyed by tag.
    pub(super) languages: HashMap<String, DiagramLanguage>,
}

/// A parsed section of content — either a diagram block or plain text.
pub(super) enum DiagramSection<'a> {
    /// A diagram fenced code block.
    Diagram {
        tag: &'a str,
        source_lines: Vec<&'a str>,
        start_line: usize,
    },
    /// A regular text line.
    Text { line: &'a str, source_line: usize },
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
    pub(super) fn parse_diagram_blocks<'a>(&self, lines: &'a [String]) -> Vec<DiagramSection<'a>> {
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

    /// Try to render a diagram using the configured backend.
    ///
    /// Returns `Some((png_bytes, display_name))` on success, `None` if no backend
    /// succeeded (caller should fall back to text rendering).
    fn try_render_backend(
        &self,
        tag: &str,
        source: &str,
        colors: &ThemeColors,
    ) -> Option<(Vec<u8>, String)> {
        let lang = self.get_language(tag)?;
        let display_name = lang.display_name.clone();
        let backend = self.backend();

        match backend {
            "text_fallback" => None,
            "native" => self
                .try_native_mermaid(tag, source, colors)
                .map(|d| (d, display_name)),
            "local" => self.try_local_cli(lang, source).map(|d| (d, display_name)),
            "kroki" => self.try_kroki(lang, source).map(|d| (d, display_name)),
            // "auto" or unrecognized: try native (mermaid only) → local CLI → Kroki.
            _ => self
                .try_native_mermaid(tag, source, colors)
                .or_else(|| self.try_local_cli(lang, source))
                .or_else(|| self.try_kroki(lang, source))
                .map(|d| (d, display_name)),
        }
    }

    /// Try to render a mermaid diagram natively using `mermaid-rs-renderer`.
    ///
    /// Only works for mermaid diagrams; returns `None` for other diagram types.
    /// Renders mermaid source → SVG → PNG bytes.
    pub(super) fn try_native_mermaid(
        &self,
        tag: &str,
        source: &str,
        colors: &ThemeColors,
    ) -> Option<Vec<u8>> {
        if tag != "mermaid" {
            return None;
        }

        let theme = dark_mermaid_theme(colors);
        let opts = mermaid_rs_renderer::RenderOptions {
            theme,
            layout: mermaid_rs_renderer::LayoutConfig::default(),
        };

        // Render mermaid source to SVG using the dark theme.
        let svg = match mermaid_rs_renderer::render_with_options(source, opts) {
            Ok(svg_str) => {
                crate::debug_info!(
                    "PRETTIFIER",
                    "Native Mermaid SVG generated ({} bytes)",
                    svg_str.len()
                );
                svg_str
            }
            Err(e) => {
                crate::debug_info!("PRETTIFIER", "Native Mermaid render failed: {e}");
                return None;
            }
        };

        // Convert SVG to PNG with terminal background.
        svg_to_png_bytes(&svg, Some(colors.bg))
    }

    /// Render a diagram via a local CLI command.
    ///
    /// Some tools (like `mmdc`) don't support stdout piping and require file
    /// output, so we use a temp file strategy: write source to a temp input
    /// file, invoke the CLI with temp output path, then read the result.
    pub(super) fn try_local_cli(&self, lang: &DiagramLanguage, source: &str) -> Option<Vec<u8>> {
        let cmd = lang.local_command.as_deref()?;

        let tmp_dir = std::env::temp_dir();
        let input_path = tmp_dir.join("par_term_diagram_input.txt");
        let output_path = tmp_dir.join("par_term_diagram_output.png");

        // Write source to temp input file.
        std::fs::write(&input_path, source).ok()?;

        // Remove stale output so we can detect fresh generation.
        let _ = std::fs::remove_file(&output_path);

        // Build args, substituting placeholder paths.
        let args: Vec<String> = lang
            .local_args
            .iter()
            .map(|a| match a.as_str() {
                "/dev/stdin" => input_path.to_string_lossy().into_owned(),
                "/dev/stdout" => output_path.to_string_lossy().into_owned(),
                other => other.to_string(),
            })
            .collect();

        let status = Command::new(cmd)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .ok()?;

        // Clean up input file.
        let _ = std::fs::remove_file(&input_path);

        if status.success() {
            let data = std::fs::read(&output_path).ok()?;
            let _ = std::fs::remove_file(&output_path);
            if data.is_empty() { None } else { Some(data) }
        } else {
            let _ = std::fs::remove_file(&output_path);
            None
        }
    }

    /// Render a diagram via the Kroki API.
    ///
    /// Sends a POST request with the diagram source and receives PNG back.
    /// Gracefully returns `None` if the HTTP request fails or TLS is unavailable.
    fn try_kroki(&self, lang: &DiagramLanguage, source: &str) -> Option<Vec<u8>> {
        let kroki_type = lang.kroki_type.as_deref()?;
        let server = self
            .config
            .kroki_server
            .as_deref()
            .unwrap_or(DEFAULT_KROKI_SERVER);
        let url = format!("{server}/{kroki_type}/png");
        let source = source.to_string();

        // ureq may panic if TLS provider isn't available at runtime;
        // catch_unwind ensures we fall back gracefully instead of crashing.
        std::panic::catch_unwind(|| {
            let response = ureq::post(&url)
                .header("Content-Type", "text/plain")
                .send(source.as_bytes())
                .ok()?;

            let bytes = response.into_body().read_to_vec().ok()?;
            if bytes.is_empty() { None } else { Some(bytes) }
        })
        .ok()
        .flatten()
    }

    /// Render a diagram section with backend rendering, producing an InlineGraphic
    /// and source text with a "(rendered)" badge, or falling back to text rendering.
    ///
    /// When a backend succeeds, the PNG is decoded to RGBA pixels and stored in
    /// an `InlineGraphic`. Blank placeholder rows are emitted sized to the image
    /// so the GPU renderer can overlay the graphic at the correct position.
    pub fn render_diagram_section(
        &self,
        tag: &str,
        source_lines: &[&str],
        start_line: usize,
        theme: &RendererConfig,
    ) -> (Vec<StyledLine>, Vec<SourceLineMapping>, Vec<InlineGraphic>) {
        let source = source_lines.join("\n");

        // Try backend rendering.
        if let Some((png_data, display_name)) =
            self.try_render_backend(tag, &source, &theme.theme_colors)
        {
            // Decode PNG → RGBA so the GPU renderer can texture it directly.
            let decoded = image::load_from_memory(&png_data).ok();
            let (rgba_data, pixel_width, pixel_height) = match decoded {
                Some(img) => {
                    let rgba = img.to_rgba8();
                    let w = rgba.width();
                    let h = rgba.height();
                    (rgba.into_raw(), w, h)
                }
                None => {
                    // PNG decode failed — fall back to text rendering.
                    let (styled, mappings) =
                        self.render_diagram_fallback(tag, source_lines, start_line, theme);
                    return (styled, mappings, vec![]);
                }
            };

            let mut styled = Vec::new();
            let mut mappings = Vec::new();

            let width_cells = theme.terminal_width.min(80);

            // Compute image height in terminal rows from pixel dimensions.
            let cell_h = theme.cell_height_px.unwrap_or(16.0);
            let image_rows = ((pixel_height as f32) / cell_h).ceil() as usize;
            let height_cells = image_rows + 1; // +1 for header line

            // Header line with green "(rendered)" badge.
            let header = StyledLine::new(vec![
                StyledSegment {
                    text: format!(" {display_name} "),
                    fg: Some(theme.theme_colors.bg),
                    bg: Some(theme.theme_colors.palette[2]), // Green background = rendered
                    bold: true,
                    ..Default::default()
                },
                StyledSegment {
                    text: " (rendered)".to_string(),
                    fg: Some(theme.theme_colors.palette[10]), // Bright green
                    ..Default::default()
                },
            ]);
            mappings.push(SourceLineMapping {
                rendered_line: 0,
                source_line: Some(start_line),
            });
            styled.push(header);

            // Emit blank placeholder rows — the GPU renderer will overlay the
            // graphic image on top of these rows.
            for i in 0..image_rows {
                let source_idx = if i < source_lines.len() {
                    Some(start_line + 1 + i)
                } else {
                    None
                };
                mappings.push(SourceLineMapping {
                    rendered_line: styled.len(),
                    source_line: source_idx,
                });
                styled.push(StyledLine::plain(""));
            }

            let graphic = InlineGraphic {
                data: Arc::new(rgba_data),
                row: 1, // First row after header
                col: 0,
                width_cells,
                height_cells,
                pixel_width,
                pixel_height,
                is_rgba: true,
            };

            (styled, mappings, vec![graphic])
        } else {
            // Fall back to text rendering.
            let (styled, mappings) =
                self.render_diagram_fallback(tag, source_lines, start_line, theme);
            (styled, mappings, vec![])
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
        let mut all_graphics = Vec::new();

        for section in sections {
            match section {
                DiagramSection::Diagram {
                    tag,
                    source_lines,
                    start_line,
                } => {
                    let (lines, mappings, graphics) =
                        self.render_diagram_section(tag, &source_lines, start_line, config);
                    let offset = all_lines.len();
                    // Adjust mapping and graphic indices for the current offset.
                    for mut mapping in mappings {
                        mapping.rendered_line += offset;
                        all_mappings.push(mapping);
                    }
                    for mut graphic in graphics {
                        graphic.row += offset;
                        all_graphics.push(graphic);
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
            graphics: all_graphics,
            format_badge: "DG".to_string(),
        })
    }

    fn format_badge(&self) -> &str {
        "DG"
    }
}

/// Register the diagram renderer with the registry.
pub fn register_diagram_renderer(
    registry: &mut crate::prettifier::registry::RendererRegistry,
    config: &DiagramRendererConfig,
) {
    registry.register_renderer("diagrams", Box::new(DiagramRenderer::new(config.clone())));
}

/// Render a single diagram source line with basic syntax highlighting.
pub(super) fn render_diagram_source_line(
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

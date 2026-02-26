//! Diagram renderer for fenced code blocks tagged with diagram language identifiers.
//!
//! Converts fenced code blocks tagged with diagram language identifiers
//! (Mermaid, PlantUML, GraphViz, D2, etc.) into rendered output using one of
//! four backends:
//!
//! - **Native**: pure-Rust mermaid rendering via `mermaid-rs-renderer` (mermaid only,
//!   500–1400× faster than mmdc, zero external dependencies).
//! - **Local CLI**: pipes diagram source to a local tool (e.g., `dot`, `mmdc`),
//!   captures PNG output, and stores it as an `InlineGraphic`.
//! - **Kroki API**: sends diagram source via HTTP POST to a Kroki server,
//!   receives PNG output, and stores it as an `InlineGraphic`.
//! - **Text fallback**: syntax-highlighted source display with a format badge.
//!
//! Backend selection follows the `engine` config: `"auto"` tries native (mermaid
//! only) then local CLI then Kroki, `"native"` uses only the native renderer,
//! `"local"` uses only local CLI, `"kroki"` uses only the API, and
//! `"text_fallback"` skips all backends.

use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::Arc;

use crate::config::prettifier::DiagramRendererConfig;
use crate::prettifier::traits::{ContentRenderer, RenderError, RendererConfig, ThemeColors};
use crate::prettifier::types::{
    ContentBlock, InlineGraphic, RenderedContent, RendererCapability, SourceLineMapping,
    StyledLine, StyledSegment,
};

/// Lazily-loaded system font database for SVG text rendering.
///
/// Loading system fonts is expensive (~50ms), so we do it once and share
/// the database across all `svg_to_png_bytes` calls.
static FONTDB: std::sync::LazyLock<Arc<fontdb::Database>> = std::sync::LazyLock::new(|| {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    crate::debug_info!("PRETTIFIER", "Loaded {} font faces from system", db.len());
    Arc::new(db)
});

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

/// Default Kroki server URL when none is configured.
const DEFAULT_KROKI_SERVER: &str = "https://kroki.io";

/// Renders fenced code blocks with diagram language tags.
///
/// Supports three rendering backends: local CLI tools, Kroki API, and text
/// fallback. Backend selection is controlled by the `engine` config field.
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
    fn try_native_mermaid(
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
    fn try_local_cli(&self, lang: &DiagramLanguage, source: &str) -> Option<Vec<u8>> {
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
    /// Render a diagram section using a backend engine, producing an inline graphic.
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

/// Fix malformed SVG font-family attributes that contain unescaped inner quotes.
///
/// Some renderers emit SVG like:
///   `font-family="Inter, "Segoe UI", sans-serif"`
/// which is invalid XML. We replace inner `"` within attribute values with `'`.
fn sanitize_svg_font_family(svg: &str) -> String {
    let mut result = String::with_capacity(svg.len());
    let mut chars = svg.char_indices().peekable();

    while let Some((i, c)) = chars.next() {
        result.push(c);

        // Look for font-family="
        if svg[i..].starts_with("font-family=\"") {
            // Push the rest of "font-family=\"" (skip the 'f' already pushed)
            let attr_start = "font-family=\"";
            for ch in attr_start[1..].chars() {
                chars.next();
                result.push(ch);
            }
            // Now inside the attribute value. Find the closing quote.
            // Inner quotes are replaced with single quotes.
            while let Some(&(_, next_c)) = chars.peek() {
                chars.next();
                if next_c == '"' {
                    // Is this the closing quote? Check if next char ends the attribute.
                    if let Some(&(_, after)) = chars.peek() {
                        if after == ' ' || after == '/' || after == '>' {
                            result.push('"');
                            break;
                        }
                        // Inner quote — replace with single quote.
                        result.push('\'');
                    } else {
                        // End of string — closing quote.
                        result.push('"');
                        break;
                    }
                } else {
                    result.push(next_c);
                }
            }
        }
    }
    result
}

/// Format an `[r, g, b]` triple as a `#RRGGBB` hex string.
fn rgb_to_hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// Build a dark `mermaid_rs_renderer::Theme` derived from terminal colors.
fn dark_mermaid_theme(colors: &ThemeColors) -> mermaid_rs_renderer::Theme {
    let fg = rgb_to_hex(colors.fg);
    let bg = rgb_to_hex(colors.bg);
    let blue = rgb_to_hex(colors.palette[4]);
    let mauve = rgb_to_hex(colors.palette[5]);
    let teal = rgb_to_hex(colors.palette[6]);
    let surface0 = rgb_to_hex(colors.palette[0]);
    let overlay0 = rgb_to_hex(colors.palette[8]);
    let subtext0 = rgb_to_hex(colors.palette[7]);

    let pie_colors = [
        blue.clone(),
        mauve.clone(),
        teal.clone(),
        rgb_to_hex(colors.palette[1]),  // red
        rgb_to_hex(colors.palette[2]),  // green
        rgb_to_hex(colors.palette[3]),  // yellow
        rgb_to_hex(colors.palette[9]),  // bright red
        rgb_to_hex(colors.palette[10]), // bright green
        rgb_to_hex(colors.palette[11]), // bright yellow
        rgb_to_hex(colors.palette[12]), // bright blue
        rgb_to_hex(colors.palette[13]), // bright magenta
        rgb_to_hex(colors.palette[14]), // bright cyan
    ];

    mermaid_rs_renderer::Theme {
        font_family: "sans-serif".to_string(),
        font_size: 14.0,
        primary_color: blue.clone(),
        primary_text_color: "#FFFFFF".to_string(),
        primary_border_color: overlay0.clone(),
        line_color: subtext0.clone(),
        secondary_color: mauve,
        tertiary_color: teal,
        edge_label_background: surface0.clone(),
        cluster_background: surface0,
        cluster_border: overlay0.clone(),
        background: bg,
        sequence_actor_fill: blue,
        sequence_actor_border: overlay0.clone(),
        sequence_actor_line: subtext0.clone(),
        sequence_note_fill: rgb_to_hex(colors.palette[3]),
        sequence_note_border: rgb_to_hex(colors.palette[11]),
        sequence_activation_fill: overlay0.clone(),
        sequence_activation_border: subtext0.clone(),
        text_color: fg.clone(),
        git_colors: mermaid_rs_renderer::Theme::modern().git_colors,
        git_inv_colors: mermaid_rs_renderer::Theme::modern().git_inv_colors,
        git_branch_label_colors: mermaid_rs_renderer::Theme::modern().git_branch_label_colors,
        git_commit_label_color: fg.clone(),
        git_commit_label_background: overlay0.clone(),
        git_tag_label_color: fg.clone(),
        git_tag_label_background: overlay0,
        git_tag_label_border: subtext0,
        pie_colors,
        pie_title_text_size: 25.0,
        pie_title_text_color: fg.clone(),
        pie_section_text_size: 17.0,
        pie_section_text_color: fg.clone(),
        pie_legend_text_size: 17.0,
        pie_legend_text_color: fg,
        pie_stroke_color: rgb_to_hex(colors.palette[7]),
        pie_stroke_width: 1.6,
        pie_outer_stroke_width: 1.6,
        pie_outer_stroke_color: rgb_to_hex(colors.palette[8]),
        pie_opacity: 0.85,
    }
}

/// Convert an SVG string to PNG bytes using resvg.
///
/// `bg` sets the pixmap background color; defaults to white when `None`.
/// System fonts are loaded lazily via [`FONTDB`] so that `<text>` elements
/// render correctly.
///
/// Returns `None` if parsing fails, dimensions are invalid (zero or > 4096),
/// or rasterization/encoding fails.
pub fn svg_to_png_bytes(svg: &str, bg: Option<[u8; 3]>) -> Option<Vec<u8>> {
    use image::codecs::png::PngEncoder;
    use image::ImageEncoder;

    // Some SVG generators produce malformed font-family attributes with
    // unescaped inner quotes (e.g. font-family="..., "Segoe UI", ...").
    // Fix these so the XML parser doesn't choke.
    let svg = sanitize_svg_font_family(svg);

    let opts = resvg::usvg::Options {
        fontdb: FONTDB.clone(),
        ..Default::default()
    };
    let tree = match resvg::usvg::Tree::from_str(&svg, &opts) {
        Ok(t) => t,
        Err(e) => {
            crate::debug_info!("PRETTIFIER", "SVG parse failed: {e}");
            return None;
        }
    };
    let size = tree.size();
    let width = size.width().ceil() as u32;
    let height = size.height().ceil() as u32;

    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        crate::debug_info!(
            "PRETTIFIER",
            "SVG dimensions out of range: {width}x{height}"
        );
        return None;
    }

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    let [r, g, b] = bg.unwrap_or([255, 255, 255]);
    pixmap.fill(resvg::tiny_skia::Color::from_rgba8(r, g, b, 255));

    resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pixmap.as_mut());

    let mut png_buf = Vec::new();
    let encoder = PngEncoder::new(&mut png_buf);
    encoder
        .write_image(pixmap.data(), width, height, image::ExtendedColorType::Rgba8)
        .ok()?;

    crate::debug_info!(
        "PRETTIFIER",
        "SVG->PNG conversion succeeded: {width}x{height}, {} bytes",
        png_buf.len()
    );
    Some(png_buf)
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
    fn test_render_mermaid_native() {
        // With "auto" backend, mermaid now renders natively.
        let renderer = test_renderer();
        let block = make_block(&["```mermaid", "graph TD", "  A-->B", "```"]);
        let config = RendererConfig::default();

        let result = renderer.render(&block, &config).unwrap();
        assert_eq!(result.format_badge, "DG");
        // Should have header + placeholder rows for the rendered image.
        assert!(result.lines.len() >= 2, "Expected at least header + placeholder rows");
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
        // Line 0: "Here is a diagram:" (plain text)
        // Line 1: header (Mermaid badge with "(rendered)" — native backend succeeds)
        // Lines 2..N: blank placeholder rows for the rendered image
        // Last line: "End of content." (plain text)
        assert!(result.lines.len() >= 3, "Expected at least text + header + text");
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
        // At least the header line; if native render fails for empty source,
        // fallback produces just the header. If it succeeds, header + placeholders.
        assert!(!result.lines.is_empty());
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
        // With native rendering: header + placeholder rows, all mapped.
        assert!(!result.line_mapping.is_empty());
        // Header maps to source line 0 (the opening fence).
        assert_eq!(result.line_mapping[0].source_line, Some(0));
        // Mapping count matches line count.
        assert_eq!(result.line_mapping.len(), result.lines.len());
    }

    #[test]
    fn test_syntax_highlight_comments() {
        // Use text_fallback to test source-line styling without native rendering.
        let config = DiagramRendererConfig {
            engine: Some("text_fallback".into()),
            ..DiagramRendererConfig::default()
        };
        let renderer = DiagramRenderer::new(config);
        let block = make_block(&["```mermaid", "%% This is a comment", "graph TD", "```"]);
        let rc = RendererConfig::default();

        let result = renderer.render(&block, &rc).unwrap();
        // Comment line (index 1) should be italic.
        assert!(result.lines[1].segments[0].italic);
    }

    #[test]
    fn test_syntax_highlight_keywords() {
        // Use text_fallback to test source-line styling without native rendering.
        let config = DiagramRendererConfig {
            engine: Some("text_fallback".into()),
            ..DiagramRendererConfig::default()
        };
        let renderer = DiagramRenderer::new(config);
        let block = make_block(&["```mermaid", "graph TD", "  A-->B", "```"]);
        let rc = RendererConfig::default();

        let result = renderer.render(&block, &rc).unwrap();
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
    fn test_text_fallback_engine() {
        let config = DiagramRendererConfig {
            engine: Some("text_fallback".into()),
            ..DiagramRendererConfig::default()
        };
        let renderer = DiagramRenderer::new(config);
        let block = make_block(&["```mermaid", "graph TD", "```"]);
        let result = renderer.render(&block, &RendererConfig::default()).unwrap();
        // text_fallback should always produce source-style output.
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
        // With "auto" backend and no local tools installed, should fall back to text.
        let config = DiagramRendererConfig {
            engine: None, // "auto"
            ..DiagramRendererConfig::default()
        };
        let renderer = DiagramRenderer::new(config);
        // Use a language with no local command to ensure fallback.
        let block = make_block(&["```ditaa", "+---+", "| A |", "+---+", "```"]);
        let result = renderer.render(&block, &RendererConfig::default()).unwrap();
        // Should still produce output (text fallback).
        assert!(!result.lines.is_empty());
    }

    #[test]
    fn test_try_local_cli_missing_command() {
        let renderer = test_renderer();
        let lang = DiagramLanguage {
            tag: "test".into(),
            display_name: "Test".into(),
            kroki_type: None,
            local_command: Some("nonexistent_command_12345".into()),
            local_args: vec![],
        };
        // Should return None for missing command.
        assert!(renderer.try_local_cli(&lang, "test").is_none());
    }

    #[test]
    fn test_try_local_cli_no_command() {
        let renderer = test_renderer();
        let lang = DiagramLanguage {
            tag: "test".into(),
            display_name: "Test".into(),
            kroki_type: None,
            local_command: None,
            local_args: vec![],
        };
        // Should return None when no command is configured.
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
        // Mermaid (native rendered): header + 1 source = 2
        // Text: 1
        // D2 (fallback): header + 1 source = 2
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
        // Mermaid should have a graphic, D2 should not.
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
        assert!(result.is_some(), "Native mermaid should render a basic flowchart");
        let png = result.unwrap();
        // PNG signature: first 8 bytes.
        assert!(png.len() > 8);
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn test_native_mermaid_handles_garbage_gracefully() {
        let renderer = test_renderer();
        let colors = ThemeColors::default();
        // The mermaid-rs-renderer is tolerant of unusual input.
        // This test verifies it doesn't panic on garbage — the result can be
        // either Some (rendered as best-effort) or None (parse error).
        let _ = renderer.try_native_mermaid("mermaid", "", &colors);
        let _ = renderer.try_native_mermaid("mermaid", "this is not valid mermaid %%!@#$", &colors);
        let _ = renderer.try_native_mermaid("mermaid", "\x00\x01\x02", &colors);
        // If we get here without panicking, the test passes.
    }

    #[test]
    fn test_native_mermaid_sequence_diagram() {
        let renderer = test_renderer();
        let colors = ThemeColors::default();
        let source = "sequenceDiagram\n  Alice->>Bob: Hello\n  Bob-->>Alice: Hi";
        let result = renderer.try_native_mermaid("mermaid", source, &colors);
        assert!(result.is_some(), "Native mermaid should render a sequence diagram");
        let png = result.unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn test_native_mermaid_non_mermaid_tag() {
        let renderer = test_renderer();
        let colors = ThemeColors::default();
        // Native renderer should return None for non-mermaid tags.
        assert!(renderer.try_native_mermaid("plantuml", "anything", &colors).is_none());
        assert!(renderer.try_native_mermaid("dot", "anything", &colors).is_none());
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
        // Test "native" engine setting.
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
        // "native" engine should fall back for non-mermaid diagrams.
        let config = DiagramRendererConfig {
            engine: Some("native".into()),
            ..DiagramRendererConfig::default()
        };
        let renderer = DiagramRenderer::new(config);
        let block = make_block(&["```plantuml", "@startuml", "Alice -> Bob", "@enduml", "```"]);
        let result = renderer.render(&block, &RendererConfig::default()).unwrap();
        // Native can't handle plantuml, so should fall back to text.
        let first_text: String = result.lines[0]
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(first_text.contains("source"));
        assert!(result.graphics.is_empty());
    }
}

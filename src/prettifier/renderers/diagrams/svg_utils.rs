//! SVG rendering utilities: SVG→PNG conversion, font sanitization, and color helpers.
//!
//! Used by the native Mermaid backend and any other renderer that produces SVG output.

use crate::prettifier::traits::ThemeColors;
use std::sync::Arc;

/// Lazily-loaded system font database for SVG text rendering.
///
/// Loading system fonts is expensive (~50ms), so we do it once and share
/// the database across all `svg_to_png_bytes` calls.
pub(super) static FONTDB: std::sync::LazyLock<Arc<fontdb::Database>> =
    std::sync::LazyLock::new(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        crate::debug_info!("PRETTIFIER", "Loaded {} font faces from system", db.len());
        Arc::new(db)
    });

/// Fix malformed SVG font-family attributes that contain unescaped inner quotes.
///
/// Some renderers emit SVG like:
///   `font-family="Inter, "Segoe UI", sans-serif"`
/// which is invalid XML. We replace inner `"` within attribute values with `'`.
pub(super) fn sanitize_svg_font_family(svg: &str) -> String {
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
pub(super) fn rgb_to_hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// Build a dark `mermaid_rs_renderer::Theme` derived from terminal colors.
pub(super) fn dark_mermaid_theme(colors: &ThemeColors) -> mermaid_rs_renderer::Theme {
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
    use image::ImageEncoder;
    use image::codecs::png::PngEncoder;

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

    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );

    let mut png_buf = Vec::new();
    let encoder = PngEncoder::new(&mut png_buf);
    encoder
        .write_image(
            pixmap.data(),
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;

    crate::debug_info!(
        "PRETTIFIER",
        "SVG->PNG conversion succeeded: {width}x{height}, {} bytes",
        png_buf.len()
    );
    Some(png_buf)
}

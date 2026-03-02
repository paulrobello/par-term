//! Diagram rendering backends: native Mermaid, local CLI, and Kroki API.
//!
//! Each backend receives diagram source text and returns raw PNG bytes on
//! success. Callers fall back to text rendering when all backends return `None`.

use crate::config::prettifier::DiagramRendererConfig;
use crate::traits::ThemeColors;

use super::languages::DiagramLanguage;
use super::svg_utils::{dark_mermaid_theme, svg_to_png_bytes};

/// Default Kroki server URL when none is configured.
pub(super) const DEFAULT_KROKI_SERVER: &str = "https://kroki.io";

/// Try to render a mermaid diagram natively using `mermaid-rs-renderer`.
///
/// Only works for mermaid diagrams; returns `None` for other diagram types.
/// Renders mermaid source → SVG → PNG bytes.
pub(super) fn try_native_mermaid(tag: &str, source: &str, colors: &ThemeColors) -> Option<Vec<u8>> {
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
pub(super) fn try_local_cli(lang: &DiagramLanguage, source: &str) -> Option<Vec<u8>> {
    use std::process::{Command, Stdio};

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
pub(super) fn try_kroki(
    config: &DiagramRendererConfig,
    lang: &DiagramLanguage,
    source: &str,
) -> Option<Vec<u8>> {
    let kroki_type = lang.kroki_type.as_deref()?;
    let server = config
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

/// Try to render a diagram using the configured backend.
///
/// Returns `Some((png_bytes, display_name))` on success, `None` if no backend
/// succeeded (caller should fall back to text rendering).
pub(super) fn try_render_backend(
    config: &DiagramRendererConfig,
    tag: &str,
    lang: &DiagramLanguage,
    source: &str,
    colors: &ThemeColors,
) -> Option<(Vec<u8>, String)> {
    let display_name = lang.display_name.clone();
    let backend = config.engine.as_deref().unwrap_or("auto");

    match backend {
        "text_fallback" => None,
        "native" => try_native_mermaid(tag, source, colors).map(|d| (d, display_name)),
        "local" => try_local_cli(lang, source).map(|d| (d, display_name)),
        "kroki" => try_kroki(config, lang, source).map(|d| (d, display_name)),
        // "auto" or unrecognized: try native (mermaid only) → local CLI → Kroki.
        _ => try_native_mermaid(tag, source, colors)
            .or_else(|| try_local_cli(lang, source))
            .or_else(|| try_kroki(config, lang, source))
            .map(|d| (d, display_name)),
    }
}

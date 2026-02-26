//! CPU-only font metrics calculation for window sizing.
//!
//! This module provides font metrics calculation that works without a window or GPU.
//! It enables calculating the exact window size needed for a given terminal grid
//! (cols × rows) BEFORE creating the window, eliminating visible resize on startup.

use anyhow::Result;
use fontdb::{Database, Family, Query};
use swash::FontRef;

use crate::config::Config;

/// Font metrics calculated from font data.
#[derive(Debug, Clone)]
pub struct FontMetrics {
    /// Width of a single cell in pixels
    pub cell_width: f32,
    /// Height of a single cell in pixels
    pub cell_height: f32,
    /// Font ascent (distance from baseline to top)
    pub ascent: f32,
    /// Font descent (distance from baseline to bottom, typically negative)
    pub descent: f32,
    /// Font leading (extra line spacing)
    pub leading: f32,
    /// Character advance width
    pub char_advance: f32,
    /// Font size in pixels (after DPI scaling)
    pub font_size_pixels: f32,
}

/// Embedded DejaVu Sans Mono font (TTF format) - same as font_manager/loader.rs
const EMBEDDED_FONT: &[u8] = include_bytes!("../fonts/DejaVuSansMono.ttf");

/// Calculate font metrics without requiring a window or GPU.
///
/// This performs the same calculation as `CellRenderer::new()` but uses only
/// CPU-based font operations (fontdb + swash).
///
/// # Arguments
/// * `font_family` - Primary font family name (None uses embedded DejaVu Sans Mono)
/// * `font_size` - Font size in points
/// * `line_spacing` - Line height multiplier (1.0 = tight, 1.2 = default)
/// * `char_spacing` - Character width multiplier (1.0 = normal)
/// * `scale_factor` - Display scale factor (1.0 for standard DPI, 2.0 for Retina)
pub fn calculate_font_metrics(
    font_family: Option<&str>,
    font_size: f32,
    line_spacing: f32,
    char_spacing: f32,
    scale_factor: f32,
) -> Result<FontMetrics> {
    // Load font database
    let mut font_db = Database::new();
    font_db.load_system_fonts();

    // Get font data - either from system or embedded
    let font_data = if let Some(family_name) = font_family {
        // Try to load from system
        let query = Query {
            families: &[Family::Name(family_name)],
            weight: fontdb::Weight::NORMAL,
            style: fontdb::Style::Normal,
            ..Query::default()
        };

        if let Some(id) = font_db.query(&query) {
            // SAFETY: make_shared_face_data is safe when called with a valid ID from query()
            if let Some((data, _)) = unsafe { font_db.make_shared_face_data(id) } {
                data.as_ref().as_ref().to_vec()
            } else {
                log::warn!(
                    "Font '{}' found but failed to load data, using embedded font",
                    family_name
                );
                EMBEDDED_FONT.to_vec()
            }
        } else {
            log::warn!(
                "Font '{}' not found, using embedded DejaVu Sans Mono",
                family_name
            );
            EMBEDDED_FONT.to_vec()
        }
    } else {
        EMBEDDED_FONT.to_vec()
    };

    // Create FontRef for metrics calculation
    let font_ref = FontRef::from_index(&font_data, 0)
        .ok_or_else(|| anyhow::anyhow!("Failed to create FontRef from font data"))?;

    // Calculate font size in pixels (matching CellRenderer::new logic)
    let platform_dpi = if cfg!(target_os = "macos") {
        72.0
    } else {
        96.0
    };
    let base_font_pixels = font_size * platform_dpi / 72.0;
    let font_size_pixels = (base_font_pixels * scale_factor).max(1.0);

    // Extract font metrics
    let metrics = font_ref.metrics(&[]);
    let scale = font_size_pixels / metrics.units_per_em as f32;

    let ascent = metrics.ascent * scale;
    let descent = metrics.descent * scale;
    let leading = metrics.leading * scale;

    // Get advance width for 'm' character (standard monospace reference)
    let glyph_id = font_ref.charmap().map('m');
    let char_advance = font_ref.glyph_metrics(&[]).advance_width(glyph_id) * scale;

    // Calculate cell dimensions (matching CellRenderer::new logic)
    let natural_line_height = ascent + descent + leading;
    let cell_height = (natural_line_height * line_spacing).max(1.0);
    let cell_width = (char_advance * char_spacing).max(1.0);

    Ok(FontMetrics {
        cell_width,
        cell_height,
        ascent,
        descent,
        leading,
        char_advance,
        font_size_pixels,
    })
}

/// Calculate the window size needed for a given terminal grid.
///
/// # Arguments
/// * `cols` - Number of columns
/// * `rows` - Number of rows
/// * `cell_width` - Width of each cell in pixels
/// * `cell_height` - Height of each cell in pixels
/// * `padding` - Window padding in pixels
/// * `tab_bar_height` - Height of tab bar (0 if hidden)
///
/// # Returns
/// `(width, height)` in logical pixels
pub fn calculate_window_size(
    cols: usize,
    rows: usize,
    cell_width: f32,
    cell_height: f32,
    padding: f32,
    tab_bar_height: f32,
) -> (u32, u32) {
    let content_width = cols as f32 * cell_width;
    let content_height = rows as f32 * cell_height;

    // Add padding on all sides, plus tab bar height
    let width = (content_width + padding * 2.0).ceil() as u32;
    let height = (content_height + padding * 2.0 + tab_bar_height).ceil() as u32;

    (width.max(100), height.max(100)) // Minimum window size
}

/// Calculate window size directly from configuration.
///
/// This is a convenience function that combines font metrics calculation
/// with window size calculation, using values from the Config.
///
/// # Arguments
/// * `config` - Terminal configuration
/// * `scale_factor` - Display scale factor (defaults to 1.0 if not known yet)
///
/// # Returns
/// `(width, height)` in logical pixels, or error if font loading fails
pub fn window_size_from_config(config: &Config, scale_factor: f32) -> Result<(u32, u32)> {
    let metrics = calculate_font_metrics(
        Some(&config.font_family),
        config.font_size,
        config.line_spacing,
        config.char_spacing,
        scale_factor,
    )?;

    // Determine tab bar height based on mode
    let tab_bar_height = match config.tab_bar_mode {
        crate::config::TabBarMode::Always => config.tab_bar_height,
        crate::config::TabBarMode::WhenMultiple | crate::config::TabBarMode::Never => 0.0,
    };

    let (width, height) = calculate_window_size(
        config.cols,
        config.rows,
        metrics.cell_width,
        metrics.cell_height,
        config.window_padding,
        tab_bar_height,
    );

    log::info!(
        "Calculated window size: {}x{} for {}x{} grid (cell: {:.1}x{:.1}, padding: {:.1}, tab_bar: {:.1})",
        width,
        height,
        config.cols,
        config.rows,
        metrics.cell_width,
        metrics.cell_height,
        config.window_padding,
        tab_bar_height
    );

    Ok((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_font_metrics_embedded() {
        // Test with embedded font (no font family specified)
        let metrics = calculate_font_metrics(None, 13.0, 1.0, 1.0, 1.0).unwrap();

        assert!(metrics.cell_width > 0.0);
        assert!(metrics.cell_height > 0.0);
        assert!(metrics.font_size_pixels > 0.0);
    }

    #[test]
    fn test_calculate_window_size() {
        let (width, height) = calculate_window_size(80, 24, 8.0, 16.0, 10.0, 0.0);

        // 80 * 8 + 10 * 2 = 660
        assert_eq!(width, 660);
        // 24 * 16 + 10 * 2 = 404
        assert_eq!(height, 404);
    }

    #[test]
    fn test_calculate_window_size_with_tab_bar() {
        let (width, height) = calculate_window_size(80, 24, 8.0, 16.0, 10.0, 28.0);

        assert_eq!(width, 660);
        // 24 * 16 + 10 * 2 + 28 = 432
        assert_eq!(height, 432);
    }

    #[test]
    fn test_minimum_window_size() {
        let (width, height) = calculate_window_size(1, 1, 1.0, 1.0, 0.0, 0.0);

        // Minimum is 100x100
        assert_eq!(width, 100);
        assert_eq!(height, 100);
    }

    #[test]
    fn test_line_spacing_affects_cell_height() {
        let metrics_tight = calculate_font_metrics(None, 13.0, 1.0, 1.0, 1.0).unwrap();
        let metrics_spacious = calculate_font_metrics(None, 13.0, 1.5, 1.0, 1.0).unwrap();

        // Cell height should be 50% larger with 1.5 line spacing
        let ratio = metrics_spacious.cell_height / metrics_tight.cell_height;
        assert!((ratio - 1.5).abs() < 0.01);
    }

    #[test]
    fn test_char_spacing_affects_cell_width() {
        let metrics_normal = calculate_font_metrics(None, 13.0, 1.0, 1.0, 1.0).unwrap();
        let metrics_wide = calculate_font_metrics(None, 13.0, 1.0, 1.5, 1.0).unwrap();

        // Cell width should be 50% larger with 1.5 char spacing
        let ratio = metrics_wide.cell_width / metrics_normal.cell_width;
        assert!((ratio - 1.5).abs() < 0.01);
    }

    #[test]
    fn test_scale_factor_affects_metrics() {
        let metrics_1x = calculate_font_metrics(None, 13.0, 1.0, 1.0, 1.0).unwrap();
        let metrics_2x = calculate_font_metrics(None, 13.0, 1.0, 1.0, 2.0).unwrap();

        // At 2x scale, metrics should be doubled
        let width_ratio = metrics_2x.cell_width / metrics_1x.cell_width;
        let height_ratio = metrics_2x.cell_height / metrics_1x.cell_height;

        assert!((width_ratio - 2.0).abs() < 0.01);
        assert!((height_ratio - 2.0).abs() < 0.01);
    }
}

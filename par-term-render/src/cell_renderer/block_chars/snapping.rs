//! Glyph snapping utilities for block characters.
//!
//! Provides functions to align glyph bounds to cell boundaries,
//! preventing gaps between adjacent block characters (e.g. tmux pane borders).

/// Calculate snapped glyph bounds for block characters.
///
/// This function adjusts glyph position and size to align with cell boundaries,
/// preventing gaps between adjacent block characters.
///
/// # Arguments
/// * `glyph_left` - Original glyph left position in pixels
/// * `glyph_top` - Original glyph top position in pixels
/// * `render_w` - Original glyph render width in pixels
/// * `render_h` - Original glyph render height in pixels
/// * `cell_x0` - Cell left boundary in pixels
/// * `cell_y0` - Cell top boundary in pixels
/// * `cell_x1` - Cell right boundary in pixels
/// * `cell_y1` - Cell bottom boundary in pixels
/// * `snap_threshold` - Distance in pixels to consider "close enough" to snap
/// * `extension` - Amount to extend beyond boundaries to prevent gaps
///
/// # Returns
/// Tuple of (new_left, new_top, new_width, new_height)
#[allow(clippy::too_many_arguments)]
pub fn snap_glyph_to_cell(
    glyph_left: f32,
    glyph_top: f32,
    render_w: f32,
    render_h: f32,
    cell_x0: f32,
    cell_y0: f32,
    cell_x1: f32,
    cell_y1: f32,
    snap_threshold: f32,
    extension: f32,
) -> (f32, f32, f32, f32) {
    let mut new_left = glyph_left;
    let mut new_top = glyph_top;
    let mut new_w = render_w;
    let mut new_h = render_h;

    let glyph_right = glyph_left + render_w;
    let glyph_bottom = glyph_top + render_h;

    // Snap left edge
    if (glyph_left - cell_x0).abs() < snap_threshold {
        new_left = cell_x0 - extension;
        new_w = glyph_right - new_left;
    }

    // Snap right edge
    if (glyph_right - cell_x1).abs() < snap_threshold {
        new_w = cell_x1 + extension - new_left;
    }

    // Snap top edge
    if (glyph_top - cell_y0).abs() < snap_threshold {
        new_top = cell_y0 - extension;
        new_h = glyph_bottom - new_top;
    }

    // Snap bottom edge
    if (glyph_bottom - cell_y1).abs() < snap_threshold {
        new_h = cell_y1 + extension - new_top;
    }

    // Also snap to middle boundaries for half-block characters
    let cell_cx = (cell_x0 + cell_x1) / 2.0;
    let cell_cy = (cell_y0 + cell_y1) / 2.0;

    // Vertical middle snap
    if (glyph_bottom - cell_cy).abs() < snap_threshold {
        new_h = cell_cy - new_top;
    } else if (glyph_top - cell_cy).abs() < snap_threshold {
        let bottom = new_top + new_h;
        new_top = cell_cy;
        new_h = bottom - new_top;
    }

    // Horizontal middle snap
    if (glyph_right - cell_cx).abs() < snap_threshold {
        new_w = cell_cx - new_left;
    } else if (glyph_left - cell_cx).abs() < snap_threshold {
        let right = new_left + new_w;
        new_left = cell_cx;
        new_w = right - new_left;
    }

    (new_left, new_top, new_w, new_h)
}

//! Glyph snapping utilities for block characters.
//!
//! Provides functions to align glyph bounds to cell boundaries,
//! preventing gaps between adjacent block characters (e.g. tmux pane borders).

/// Parameters for [`snap_glyph_to_cell`].
pub struct SnapGlyphParams {
    /// Original glyph left position in pixels.
    pub glyph_left: f32,
    /// Original glyph top position in pixels.
    pub glyph_top: f32,
    /// Original glyph render width in pixels.
    pub render_w: f32,
    /// Original glyph render height in pixels.
    pub render_h: f32,
    /// Cell left boundary in pixels.
    pub cell_x0: f32,
    /// Cell top boundary in pixels.
    pub cell_y0: f32,
    /// Cell right boundary in pixels.
    pub cell_x1: f32,
    /// Cell bottom boundary in pixels.
    pub cell_y1: f32,
    /// Distance in pixels to consider "close enough" to snap.
    pub snap_threshold: f32,
    /// Amount to extend beyond boundaries to prevent gaps.
    pub extension: f32,
}

/// Calculate snapped glyph bounds for block characters.
///
/// Adjusts glyph position and size to align with cell boundaries,
/// preventing gaps between adjacent block characters.
///
/// Returns `(new_left, new_top, new_width, new_height)`.
pub fn snap_glyph_to_cell(p: SnapGlyphParams) -> (f32, f32, f32, f32) {
    let SnapGlyphParams {
        glyph_left,
        glyph_top,
        render_w,
        render_h,
        cell_x0,
        cell_y0,
        cell_x1,
        cell_y1,
        snap_threshold,
        extension,
    } = p;
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

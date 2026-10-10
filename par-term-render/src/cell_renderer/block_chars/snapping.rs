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

    // Pixel-align the snapped quad: fractional edges (fractional cell metrics,
    // the 0.5px extension) make the GPU resample the bitmap's flat edge across
    // two output columns, which reads as a thin line of the wrong color at
    // powerline cap junctions. Rounding the absolute edges — not position and
    // size independently — keeps shared cell boundaries consistent with the
    // geometric block path's per-cell rounding.
    let right = new_left + new_w;
    let bottom = new_top + new_h;
    new_left = new_left.round();
    new_top = new_top.round();
    new_w = right.round() - new_left;
    new_h = bottom.round() - new_top;

    (new_left, new_top, new_w, new_h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapped_quad_edges_are_pixel_aligned() {
        // Fractional cell metrics (e.g. JetBrains Mono 13pt on a 2x display)
        // put every snapped edge between pixels; the quad must not straddle.
        let (l, t, w, h) = snap_glyph_to_cell(SnapGlyphParams {
            glyph_left: 10.9,
            glyph_top: 31.1,
            render_w: 8.2,
            render_h: 15.9,
            cell_x0: 10.4,
            cell_y0: 30.6,
            cell_x1: 19.7,
            cell_y1: 47.5,
            snap_threshold: 3.0,
            extension: 0.5,
        });
        for (name, v) in [("left", l), ("top", t), ("right", l + w), ("bottom", t + h)] {
            assert_eq!(v, v.round(), "{name} edge must land on a pixel boundary");
        }
    }

    #[test]
    fn cap_flat_edge_reaches_cell_boundary() {
        // A left-cap powerline glyph (U+E0B6) has its flat right edge inside
        // the snap threshold of cell_x1; after pixel alignment the flat edge
        // must end at the cell's rounded boundary, never short of it, so the
        // solid pill fill takes over without a partial-coverage seam column.
        let (l, _t, w, _h) = snap_glyph_to_cell(SnapGlyphParams {
            glyph_left: 11.0,
            glyph_top: 31.0,
            render_w: 8.5,
            render_h: 16.4,
            cell_x0: 10.4,
            cell_y0: 30.6,
            cell_x1: 19.7,
            cell_y1: 47.5,
            snap_threshold: 3.0,
            extension: 0.5,
        });
        assert_eq!(l + w, 20.0); // round(19.7 + 0.5)
    }
}

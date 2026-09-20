//! Graphics placement and scaling calculations.
//!
//! Extracted from the graphics_renderer.rs root per ARC-009: pure geometry
//! with no renderer state, tested in isolation.

/// Window and pane geometry for a single [`GraphicsRenderer::render_for_pane`] call.
#[derive(Debug, Clone, Copy)]
pub struct PaneRenderGeometry {
    pub window_width: f32,
    pub window_height: f32,
    pub pane_origin_x: f32,
    pub pane_origin_y: f32,
}

/// Compute UV coordinates and display size for an inline graphic instance.
///
/// Maps the source crop rectangle to the destination cell extent, applying
/// scroll clipping in destination space (so a scrolled row removes the same
/// fraction of source and destination). Returns `(tex_coords, size)`.
#[allow(clippy::too_many_arguments)]
pub(super) fn compute_graphic_geometry(
    tex_w: f32,
    tex_h: f32,
    crop: [u32; 4],
    width_cells: usize,
    height_cells: usize,
    cell_w: f32,
    cell_h: f32,
    clip_px: f32,
    has_cols: bool,
    has_rows: bool,
    preserve_aspect: bool,
    is_virtual: bool,
    window_w: f32,
    window_h: f32,
) -> ([f32; 4], [f32; 2]) {
    let has_crop = crop != [0, 0, 0, 0];

    // Effective source rectangle (zero extents normalize to texture edges).
    let (sx, sy, sw, sh) = if has_crop && tex_w > 0.0 && tex_h > 0.0 {
        let x = (crop[0] as f32).min(tex_w);
        let y = (crop[1] as f32).min(tex_h);
        let w = if crop[2] > 0 {
            (crop[2] as f32).min(tex_w - x)
        } else {
            tex_w - x
        };
        let h = if crop[3] > 0 {
            (crop[3] as f32).min(tex_h - y)
        } else {
            tex_h - y
        };
        (x, y, w.max(0.0), h.max(0.0))
    } else {
        (0.0, 0.0, tex_w, tex_h)
    };

    // Un-clipped destination size in pixels, chosen per axis:
    // - Virtual / both c+r: exact cell rectangle.
    // - c-only: cell width × aspect-derived exact height (no cell rounding).
    // - r-only: cell height × aspect-derived exact width.
    // - neither: natural source crop size (or full texture when
    //   preserve_aspect, else cell-derived fallback).
    // Empty crop intersection (crop at the texture edge yields sw/sh=0)
    // produces nothing to draw; return zero size immediately.
    if sw <= 0.0 || sh <= 0.0 {
        return ([0.0, 0.0, 0.0, 0.0], [0.0, 0.0]);
    }
    let aspect = sw / sh;
    let (dest_w, dest_h) = if is_virtual || (has_cols && has_rows) {
        (width_cells as f32 * cell_w, height_cells as f32 * cell_h)
    } else if has_cols && !has_rows {
        let dw = width_cells as f32 * cell_w;
        (dw, dw / aspect)
    } else if has_rows && !has_cols {
        let dh = height_cells as f32 * cell_h;
        (dh * aspect, dh)
    } else if has_crop && sw > 0.0 && sh > 0.0 {
        (sw, sh)
    } else if preserve_aspect && tex_w > 0.0 && tex_h > 0.0 {
        (tex_w, tex_h)
    } else {
        (width_cells as f32 * cell_w, height_cells as f32 * cell_h)
    };

    // Destination clip fraction.
    let visible_frac = if dest_h > 0.0 {
        ((dest_h - clip_px) / dest_h).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let scrolled_frac = if dest_h > 0.0 {
        (clip_px / dest_h).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // UV: map scrolled/visible destination fractions onto the source rect.
    let uv = if sw > 0.0 && sh > 0.0 && tex_w > 0.0 && tex_h > 0.0 {
        [
            sx / tex_w,
            (sy + sh * scrolled_frac) / tex_h,
            sw / tex_w,
            (sh * visible_frac) / tex_h,
        ]
    } else {
        [0.0, 0.0, 1.0, 1.0]
    };

    let size = (dest_w / window_w, dest_h * visible_frac / window_h);

    (uv, size.into())
}

#[cfg(test)]
mod geometry_tests {
    use super::compute_graphic_geometry;

    const WW: f32 = 800.0;
    const WH: f32 = 600.0;
    const CW: f32 = 10.0;
    const CH: f32 = 20.0;

    /// 100px source in 40px dest (r=2), scrolled 1 row (20px):
    /// clip fraction 0.5, UV starts at pixel 50, 50px visible.
    #[test]
    fn no_crop_both_cells_uses_dest_fraction_for_uv() {
        let (uv, size) = compute_graphic_geometry(
            100.0,
            100.0,
            [0, 0, 0, 0],
            10,
            2,
            CW,
            CH,
            20.0, // clip_px
            true,
            true, // has_cols, has_rows
            false,
            false, // preserve_aspect, is_virtual
            WW,
            WH,
        );
        let expected_uv_y = 50.0 / 100.0;
        let expected_uv_h = 50.0 / 100.0;
        assert!((uv[1] - expected_uv_y).abs() < 1e-5);
        assert!((uv[3] - expected_uv_h).abs() < 1e-5);
        assert!((size[1] - 20.0 / WH).abs() < 1e-5);
    }

    /// 25px source crop, no c/r, scrolled 20px: dest_h=25, 5px visible.
    #[test]
    fn natural_crop_without_cells_uses_crop_height_for_dest() {
        let (uv, size) = compute_graphic_geometry(
            100.0,
            100.0,
            [0, 0, 0, 25],
            1,
            1,
            CW,
            CH,
            20.0,
            false,
            false,
            false,
            false,
            WW,
            WH,
        );
        let expected_uv_y = (0.0 + 25.0 * 0.8) / 100.0;
        let expected_uv_h = (25.0 * 0.2) / 100.0;
        assert!((uv[1] - expected_uv_y).abs() < 1e-5);
        assert!((uv[3] - expected_uv_h).abs() < 1e-5);
        assert!((size[1] - 5.0 / WH).abs() < 1e-5);
    }

    /// row=-1, Y=5: clip 15px, UV reflects 15/60 scrolled fraction.
    #[test]
    fn y_offset_produces_sub_row_clip() {
        let top_px = -1.0 * CH + 5.0;
        let clip_px = (-top_px).max(0.0);
        assert_eq!(clip_px, 15.0);

        let (uv, size) = compute_graphic_geometry(
            100.0,
            100.0,
            [0, 0, 0, 0],
            10,
            3,
            CW,
            CH,
            clip_px,
            true,
            true,
            false,
            false,
            WW,
            WH,
        );
        assert!((uv[1] - 25.0 / 100.0).abs() < 1e-5);
        assert!((uv[3] - 75.0 / 100.0).abs() < 1e-5);
        assert!((size[1] - 45.0 / WH).abs() < 1e-5);
    }

    /// 100×100 source, c=5 only, cell 10×20: dest_w=50, dest_h=50 (aspect 1:1).
    #[test]
    fn c_only_computes_exact_height_from_aspect() {
        let (_uv, size) = compute_graphic_geometry(
            100.0,
            100.0,
            [0, 0, 0, 0],
            5,
            3,
            CW,
            CH,
            0.0,
            true,
            false, // has_cols only
            false,
            false,
            WW,
            WH,
        );
        // dest_h = 50px / 600px (aspect-derived, not cell-rounded)
        assert!((size[0] - 50.0 / WW).abs() < 1e-5);
        assert!((size[1] - 50.0 / WH).abs() < 1e-5);
    }

    /// 100×100 source, r=2 only, cell 10×20: dest_h=40, dest_w=40 (aspect 1:1).
    #[test]
    fn r_only_computes_exact_width_from_aspect() {
        let (_uv, size) = compute_graphic_geometry(
            100.0,
            100.0,
            [0, 0, 0, 0],
            4,
            2,
            CW,
            CH,
            0.0,
            false,
            true, // has_rows only
            false,
            false,
            WW,
            WH,
        );
        assert!((size[0] - 40.0 / WW).abs() < 1e-5);
        assert!((size[1] - 40.0 / WH).abs() < 1e-5);
    }

    /// 100×50 source (2:1), c=5, cell 10×20: dest_w=50, dest_h=25.
    #[test]
    fn c_only_wide_source_computes_proportional_height() {
        let (_uv, size) = compute_graphic_geometry(
            100.0,
            50.0,
            [0, 0, 0, 0],
            5,
            1,
            CW,
            CH,
            0.0,
            true,
            false,
            false,
            false,
            WW,
            WH,
        );
        assert!((size[0] - 50.0 / WW).abs() < 1e-5);
        assert!((size[1] - 25.0 / WH).abs() < 1e-5);
    }

    /// Crop at the texture edge (source_x=100 on 100px image) yields sw=0.
    /// Zero-size intersection must return zero output, not a full-image
    /// fallback or NaN from aspect division.
    #[test]
    fn zero_size_crop_at_edge_returns_zero_output() {
        let (uv, size) = compute_graphic_geometry(
            100.0,
            100.0,
            [100, 0, 0, 0],
            5,
            3,
            CW,
            CH,
            0.0,
            true,
            false,
            false,
            false,
            WW,
            WH,
        );
        // Zero crop → zero UV and zero size
        assert_eq!(uv, [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(size, [0.0, 0.0]);
    }
}

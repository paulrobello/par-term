//! Center-out segment fitting for the status bar.
//!
//! The bar draws three widget groups (Left, Center, Right) in one row. When
//! the window is too narrow for every segment, hiding is decided here:
//! segments nearest the bar's horizontal center hide first, working outward,
//! so the leftmost and rightmost segments — the fixed anchors — survive
//! longest. If even a single segment cannot fit, the plan is "draw nothing
//! at all" rather than a clipped or overlapping row.

/// Visibility mask for the ordered segment list handed to
/// [`plan_visible_segments`]: `visible[i]` mirrors segment `i`.
pub type SegmentPlan = Vec<bool>;

/// Decide which status-bar segments are drawn this frame.
///
/// `widths` are the per-segment widths in visual left-to-right order;
/// `gap` is one separator width, counted between every pair of adjacent
/// visible segments; `available` is the width the row may occupy. The
/// returned mask hides the segment whose midpoint is nearest the bar's
/// horizontal center first, then the next nearest, until the visible total
/// fits; on a distance tie the right-side segment hides first. If no subset
/// fits, every segment hides.
///
/// Midpoints come from the all-visible strip laid out centered in
/// `available`, so the sweep is symmetric around the bar center and the
/// hiding order is a fixed center-out sweep that does not shift as segments
/// hide.
pub fn plan_visible_segments(widths: &[f32], gap: f32, available: f32) -> SegmentPlan {
    let mut visible = vec![true; widths.len()];
    if widths.is_empty() {
        return visible;
    }
    if available <= 0.0 {
        return vec![false; widths.len()];
    }

    // Total width of the all-visible strip; it is laid out centered in
    // `available`, spilling equally on both sides when it overflows so the
    // midpoint ranking stays symmetric around `available / 2`.
    let extra = widths.len().saturating_sub(1);
    let total = widths.iter().sum::<f32>() + gap * extra as f32;
    let start = (available - total) * 0.5;

    let center = available * 0.5;
    let mut x = start;
    let mut ranked: Vec<(f32, f32, usize)> = Vec::with_capacity(widths.len());
    for (i, &w) in widths.iter().enumerate() {
        let mid = x + w * 0.5;
        ranked.push(((mid - center).abs(), mid, i));
        x += w + gap;
    }

    // Nearest-to-center hides first; on a distance tie the right-side
    // segment goes first, so the plan is deterministic.
    ranked.sort_by(|a, b| a.0.total_cmp(&b.0).then(b.1.total_cmp(&a.1)));

    for &(_, _, index) in &ranked {
        if plan_total_width(widths, &visible, gap) <= available {
            break;
        }
        visible[index] = false;
    }

    visible
}

/// Total drawn width of the segments a plan keeps: their widths plus one
/// separator between each pair of adjacent visible segments.
pub fn plan_total_width(widths: &[f32], visible: &SegmentPlan, gap: f32) -> f32 {
    let mut total = 0.0;
    let mut seen = 0usize;
    for (i, &keep) in visible.iter().enumerate() {
        if keep {
            if seen > 0 {
                total += gap;
            }
            total += widths[i];
            seen += 1;
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask(plan: &[bool]) -> String {
        plan.iter().map(|&c| if c { '1' } else { '0' }).collect()
    }

    #[test]
    fn zero_segments_yields_empty_plan() {
        assert!(plan_visible_segments(&[], 8.0, 1000.0).is_empty());
    }

    #[test]
    fn single_segment_kept_when_it_fits_exactly() {
        assert_eq!(mask(&plan_visible_segments(&[100.0], 8.0, 100.0)), "1");
    }

    #[test]
    fn single_segment_hidden_when_one_pixel_over() {
        assert_eq!(mask(&plan_visible_segments(&[100.0], 8.0, 99.0)), "0");
    }

    #[test]
    fn fits_exactly_boundary_on_two_segment_layout() {
        // Total 10+8+10 = 28: everything fits at 28; one pixel less hides
        // a segment.
        assert_eq!(mask(&plan_visible_segments(&[10.0, 10.0], 8.0, 28.0)), "11");
        assert_eq!(mask(&plan_visible_segments(&[10.0, 10.0], 8.0, 27.0)), "10");
    }

    #[test]
    fn progressive_center_out_on_known_layout() {
        // Five equal segments, no gaps, total 50 into 30 available: the
        // center segment hides first, then the distance tie between its two
        // neighbors resolves right-first, stopping once the kept three (30)
        // fit exactly.
        assert_eq!(mask(&plan_visible_segments(&[10.0; 5], 0.0, 30.0)), "11001");
    }

    #[test]
    fn middle_hides_before_both_edges() {
        // Total 60 into 44 available: only the center segment needs to hide.
        assert_eq!(
            mask(&plan_visible_segments(&[20.0, 20.0, 20.0], 0.0, 44.0)),
            "101"
        );
    }

    #[test]
    fn tie_break_hides_right_before_left() {
        // Two equal segments straddling the bar center at identical
        // distance: the right one hides first.
        assert_eq!(mask(&plan_visible_segments(&[10.0, 10.0], 0.0, 19.0)), "10");
    }

    #[test]
    fn all_hidden_when_no_subset_fits() {
        // Each segment alone is wider than the bar: nothing can be drawn.
        assert_eq!(
            mask(&plan_visible_segments(&[120.0, 120.0], 8.0, 100.0)),
            "00"
        );
    }

    #[test]
    fn all_hidden_at_zero_available() {
        assert_eq!(mask(&plan_visible_segments(&[10.0, 10.0], 8.0, 0.0)), "00");
    }

    #[test]
    fn plan_total_never_exceeds_available() {
        // Sweep over layouts and available widths: whenever a plan keeps
        // anything visible, its total never exceeds the available width —
        // the invariant that makes overlap impossible.
        let layouts: [(&[f32], f32); 8] = [
            (&[10.0], 8.0),
            (&[10.0, 10.0], 8.0),
            (&[30.0, 12.0, 44.0, 9.0], 7.0),
            (&[5.0; 7], 3.0),
            (&[64.0, 30.0, 28.0, 90.0, 12.0], 6.0),
            (&[7.0; 3], 0.0),
            (&[7.0; 3], 2.0),
            (&[300.0, 60.0, 200.0], 6.0),
        ];
        for (widths, gap) in layouts {
            for avail in [0.0, 1.0, 5.0, 20.0, 37.0, 80.0, 120.0, 300.0, 1000.0] {
                let plan = plan_visible_segments(widths, gap, avail);
                let all_hidden = plan.iter().all(|&v| !v);
                if !all_hidden {
                    assert!(
                        plan_total_width(widths, &plan, gap) <= avail + 1e-3,
                        "plan {:?} exceeds available {} for widths {:?} gap {}",
                        plan,
                        avail,
                        widths,
                        gap
                    );
                }
            }
        }
    }
}

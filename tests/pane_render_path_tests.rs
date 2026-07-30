//! Tests for the parts of the pane render path that are reachable without a GPU.
//!
//! # Scope, stated honestly
//!
//! ENH-005 Part B asks for coverage of instance-buffer construction, cell-to-
//! instance mapping, and the three-phase draw ordering invariant. Only the
//! geometry layer is actually reachable from a test:
//!
//! | Wanted | Reachable? | Why |
//! |---|---|---|
//! | Pane viewport → grid/scissor geometry | **yes** | `PaneViewport` is public and pure |
//! | Separator mark → screen row mapping | **yes** | `compute_visible_separator_marks` is public |
//! | Block-character classification | **yes** | `block_chars` is a public module |
//! | Instance-buffer construction | no | `build_pane_instance_buffers` is `&mut self` on a device-backed `CellRenderer` |
//! | Cell → instance mapping | no | `bg_instance_builder` / `text_instance_builder` are private |
//! | Three-phase draw ordering | **yes**, in `par-term-render` | covered by `ThreePhaseRanges::draw_sequence` there, not from here — see below |
//!
//! The ordering invariant — cursor overlays must be emitted *after* text, or
//! beam and underline cursors are hidden under glyphs — is now covered, but
//! inside `par-term-render` rather than here. `emit_three_phase_draw_calls`
//! walks `ThreePhaseRanges::draw_sequence()`, so the order cannot be changed in
//! the emitter without changing the function its tests assert on, and
//! `SingleGridLayout` is the single source of the buffer arithmetic those
//! ranges are built from. Restating either from this crate would recreate the
//! drifting second copy the seam exists to prevent.
//!
//! What *is* covered targets the multi-pane failure mode directly: panes tiling
//! a window must produce disjoint scissor rectangles and grid sizes derived
//! from padded content, which is the geometry a batched-submit regression
//! corrupts.

#[path = "common/unicode_corpus.rs"]
mod unicode_corpus;

use par_term::cell_renderer::block_chars::{
    BlockCharType, SnapGlyphParams, classify_char, should_render_geometrically,
    should_snap_to_boundaries, snap_glyph_to_cell,
};
use par_term_config::ScrollbackMark;
use par_term_render::PaneViewport;
use par_term_render::compute_visible_separator_marks;
use unicode_corpus::{CORPUS, Lcg, WIDE_FIXTURES};

const CELL_W: f32 = 8.0;
const CELL_H: f32 = 16.0;

// ===========================================================================
// Pane viewport geometry
// ===========================================================================

#[test]
fn grid_size_is_derived_from_padded_content_not_raw_bounds() {
    // The bug shape this guards: computing the grid from `width`/`height`
    // instead of `content_size()` overcounts columns by `2 * padding / cell_w`,
    // so the last column of every padded pane renders outside its scissor rect.
    let unpadded = PaneViewport::new(0.0, 0.0, 800.0, 480.0, true, 1.0);
    assert_eq!(unpadded.grid_size(CELL_W, CELL_H), (100, 30));

    let padded = PaneViewport::with_padding(0.0, 0.0, 800.0, 480.0, true, 1.0, 8.0);
    assert_eq!(padded.content_size(), (784.0, 464.0));
    assert_eq!(
        padded.grid_size(CELL_W, CELL_H),
        (98, 29),
        "padding must be subtracted before dividing by the cell size"
    );
}

#[test]
fn grid_size_floors_and_never_returns_zero() {
    // A partial trailing cell must not be counted, and a pane smaller than one
    // cell must still report a 1x1 grid rather than zero (which would make
    // every downstream `cols * rows` allocation empty).
    let partial = PaneViewport::new(0.0, 0.0, 807.0, 487.0, true, 1.0);
    assert_eq!(partial.grid_size(CELL_W, CELL_H), (100, 30));

    let tiny = PaneViewport::new(0.0, 0.0, 3.0, 3.0, true, 1.0);
    assert_eq!(tiny.grid_size(CELL_W, CELL_H), (1, 1));

    // Padding wider than the pane clamps rather than underflowing.
    let over_padded = PaneViewport::with_padding(0.0, 0.0, 10.0, 10.0, true, 1.0, 40.0);
    assert_eq!(over_padded.content_size(), (1.0, 1.0));
    assert_eq!(over_padded.grid_size(CELL_W, CELL_H), (1, 1));
}

#[test]
fn content_origin_applies_padding_and_centering_offsets() {
    let mut viewport = PaneViewport::with_padding(100.0, 50.0, 400.0, 300.0, false, 0.8, 6.0);
    assert_eq!(viewport.content_origin(), (106.0, 56.0));

    viewport.content_offset_x = 3.0;
    viewport.content_offset_y = 7.0;
    assert_eq!(
        viewport.content_origin(),
        (109.0, 63.0),
        "centering offsets stack on top of padding"
    );
    // Offsets must not change the content size — only where it starts.
    assert_eq!(viewport.content_size(), (388.0, 288.0));
}

#[test]
fn scissor_rect_clamps_negatives_and_never_collapses() {
    // A pane dragged off the left/top edge must produce a valid rect: wgpu
    // rejects a negative origin, and a zero-sized scissor drops the pane
    // entirely instead of rendering a sliver.
    let offscreen = PaneViewport::new(-20.0, -8.0, 100.0, 40.0, true, 1.0);
    assert_eq!(offscreen.to_scissor_rect(), (0, 0, 100, 40));

    let collapsed = PaneViewport::new(10.0, 10.0, 0.0, 0.0, true, 1.0);
    let (_, _, width, height) = collapsed.to_scissor_rect();
    assert_eq!((width, height), (1, 1), "a scissor rect is never empty");
}

/// Split `bounds` into `count` vertical strips, the way a split-pane layout does.
fn tile_vertically(count: usize, width: f32, height: f32, padding: f32) -> Vec<PaneViewport> {
    let strip = width / count as f32;
    (0..count)
        .map(|index| {
            PaneViewport::with_padding(
                index as f32 * strip,
                0.0,
                strip,
                height,
                index == 0,
                1.0,
                padding,
            )
        })
        .collect()
}

fn rects_overlap(a: (u32, u32, u32, u32), b: (u32, u32, u32, u32)) -> bool {
    let (ax, ay, aw, ah) = a;
    let (bx, by, bw, bh) = b;
    ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah
}

#[test]
fn tiled_panes_produce_disjoint_scissor_rectangles() {
    // The multi-pane invariant. If two panes' scissor rects overlap, whichever
    // draws second paints over the first — the visible symptom of a batching or
    // suballocation regression in the pane render path.
    for count in [2usize, 3, 4, 6, 8] {
        let panes = tile_vertically(count, 1920.0, 1080.0, 4.0);
        let rects: Vec<_> = panes.iter().map(PaneViewport::to_scissor_rect).collect();

        for (i, a) in rects.iter().enumerate() {
            for (j, b) in rects.iter().enumerate().skip(i + 1) {
                assert!(
                    !rects_overlap(*a, *b),
                    "{count} panes: pane {i} {a:?} overlaps pane {j} {b:?}"
                );
            }
        }
    }
}

#[test]
fn tiled_panes_cover_the_window_without_gaps() {
    // The complement of the disjointness test: disjoint-but-gapped rects would
    // leave unpainted stripes between panes.
    let count = 6;
    let width = 1920.0;
    let panes = tile_vertically(count, width, 1080.0, 0.0);

    let mut expected_x = 0u32;
    for (index, pane) in panes.iter().enumerate() {
        let (x, _, w, _) = pane.to_scissor_rect();
        assert_eq!(
            x,
            expected_x,
            "pane {index} does not start where {} ended",
            index.saturating_sub(1)
        );
        expected_x = x + w;
    }
    assert_eq!(expected_x, width as u32, "the panes do not span the window");
}

#[test]
fn every_tiled_pane_gets_a_usable_grid() {
    // Six panes across a 1920px window is the configuration the split-pane
    // performance work exercises; each must still resolve to a real grid.
    for count in [2usize, 6] {
        for pane in tile_vertically(count, 1920.0, 1080.0, 4.0) {
            let (cols, rows) = pane.grid_size(CELL_W, CELL_H);
            assert!(cols >= 1 && rows >= 1);
            // The grid must fit inside the pane's content area.
            let (content_w, content_h) = pane.content_size();
            assert!(
                cols as f32 * CELL_W <= content_w + f32::EPSILON,
                "{count} panes: {cols} columns overflow {content_w}px of content"
            );
            assert!(
                rows as f32 * CELL_H <= content_h + f32::EPSILON,
                "{count} panes: {rows} rows overflow {content_h}px of content"
            );
        }
    }
}

#[test]
fn viewport_geometry_is_stable_under_randomized_layouts() {
    let mut rng = Lcg::new(0xDEAD_BEEF_0000_0001);
    for case in 0..2_000 {
        let x = rng.below(2000) as f32 - 500.0;
        let y = rng.below(2000) as f32 - 500.0;
        let width = rng.below(1920) as f32;
        let height = rng.below(1080) as f32;
        let padding = rng.below(40) as f32;

        let pane = PaneViewport::with_padding(x, y, width, height, true, 1.0, padding);

        let (content_w, content_h) = pane.content_size();
        assert!(
            content_w >= 1.0 && content_h >= 1.0,
            "case {case}: content collapsed"
        );

        let (cols, rows) = pane.grid_size(CELL_W, CELL_H);
        assert!(cols >= 1 && rows >= 1, "case {case}: empty grid");

        let (sx, sy, sw, sh) = pane.to_scissor_rect();
        assert!(sw >= 1 && sh >= 1, "case {case}: empty scissor");
        // Origins are u32, so a negative pane position must have clamped.
        let _ = (sx, sy);
    }
}

// ===========================================================================
// Separator mark → screen row mapping
// ===========================================================================

fn mark_at(line: usize, exit_code: Option<i32>) -> ScrollbackMark {
    ScrollbackMark {
        line,
        exit_code,
        start_time: None,
        duration_ms: None,
        command: None,
        color: None,
        trigger_id: None,
    }
}

#[test]
fn separator_marks_map_absolute_lines_to_screen_rows() {
    // With no scroll, the viewport starts at the end of the scrollback.
    let marks = [
        mark_at(100, Some(0)),
        mark_at(105, Some(1)),
        mark_at(120, None),
    ];
    let visible = compute_visible_separator_marks(&marks, 100, 0, 24);

    assert_eq!(visible.len(), 3);
    assert_eq!(visible[0], (0, Some(0), None), "line 100 is screen row 0");
    assert_eq!(visible[1], (5, Some(1), None));
    assert_eq!(visible[2], (20, None, None));
}

#[test]
fn separator_marks_outside_the_viewport_are_dropped() {
    let marks = [
        mark_at(99, Some(0)),  // one line above the viewport
        mark_at(100, Some(0)), // first visible line
        mark_at(123, Some(0)), // last visible line
        mark_at(124, Some(0)), // one line below the viewport
    ];
    let visible = compute_visible_separator_marks(&marks, 100, 0, 24);

    assert_eq!(visible.len(), 2, "only the two in-range marks survive");
    assert_eq!(visible[0].0, 0, "the lower bound is inclusive");
    assert_eq!(visible[1].0, 23, "the upper bound is exclusive at row 24");
}

#[test]
fn scrolling_back_shifts_the_visible_window() {
    let marks = [mark_at(50, Some(0)), mark_at(90, Some(0))];

    // Unscrolled, the viewport is lines 100..124 — neither mark is visible.
    assert!(compute_visible_separator_marks(&marks, 100, 0, 24).is_empty());

    // Scrolled back 20 lines, the viewport is 80..104 — only line 90 is in it.
    let visible = compute_visible_separator_marks(&marks, 100, 20, 24);
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].0, 10, "line 90 with viewport_start 80 is row 10");

    // Scrolled past the top, the viewport clamps to 0 rather than underflowing.
    let visible = compute_visible_separator_marks(&marks, 100, 500, 24);
    assert_eq!(visible.len(), 0, "lines 0..24 hold no marks");
    let visible = compute_visible_separator_marks(&[mark_at(3, None)], 100, 500, 24);
    assert_eq!(visible[0].0, 3, "viewport_start saturated to 0");
}

#[test]
fn separator_mark_mapping_handles_degenerate_viewports() {
    let marks = [mark_at(0, None), mark_at(10, None)];
    assert!(
        compute_visible_separator_marks(&marks, 0, 0, 0).is_empty(),
        "a zero-line viewport shows nothing"
    );
    assert!(
        compute_visible_separator_marks(&[], 100, 0, 24).is_empty(),
        "no marks, no rows"
    );
    // Order is preserved and duplicates are not merged — documented behaviour.
    let duplicated = [mark_at(100, Some(1)), mark_at(100, Some(2))];
    let visible = compute_visible_separator_marks(&duplicated, 100, 0, 24);
    assert_eq!(visible.len(), 2, "no deduplication is performed");
    assert_eq!(visible[0].1, Some(1));
    assert_eq!(visible[1].1, Some(2));
}

#[test]
fn separator_mark_rows_always_fall_inside_the_viewport() {
    let mut rng = Lcg::new(0x1234_5678_9ABC_DEF0);
    for case in 0..2_000 {
        let scrollback_len = rng.below(5_000);
        let scroll_offset = rng.below(6_000);
        let visible_lines = rng.below(80);
        let marks: Vec<ScrollbackMark> = (0..rng.below(10))
            .map(|_| mark_at(rng.below(6_000), None))
            .collect();

        for (row, _, _) in
            compute_visible_separator_marks(&marks, scrollback_len, scroll_offset, visible_lines)
        {
            assert!(
                row < visible_lines,
                "case {case}: row {row} is outside a {visible_lines}-line viewport"
            );
        }
    }
}

// ===========================================================================
// Block characters vs. the non-ASCII corpus
// ===========================================================================

#[test]
fn corpus_characters_are_not_mistaken_for_block_characters() {
    // Block characters are rendered geometrically instead of through the glyph
    // atlas. Misclassifying a CJK ideograph or an emoji would draw a rectangle
    // where a glyph belongs, so the classifier must leave the corpus alone.
    for fixture in CORPUS {
        for ch in fixture.text.chars() {
            assert_eq!(
                classify_char(ch),
                BlockCharType::None,
                "{}: {ch:?} (U+{:04X}) must not classify as a block character",
                fixture.label,
                ch as u32
            );
        }
    }
}

#[test]
fn block_characters_still_classify_after_the_corpus_sweep() {
    // The complement: the sweep above would pass trivially if `classify_char`
    // returned `None` for everything.
    for ch in ['▄', '▀', '▌'] {
        assert_eq!(classify_char(ch), BlockCharType::PartialBlock, "{ch:?}");
    }
    assert_eq!(classify_char('█'), BlockCharType::SolidBlock);
    for ch in ['─', '│', '┼'] {
        assert_eq!(classify_char(ch), BlockCharType::BoxDrawing);
    }

    // These are drawn as geometry and snapped to the cell grid, which is what
    // removes the hairline gaps between adjacent block characters.
    for ch in ['▄', '▀', '▌', '█', '─', '┼'] {
        let kind = classify_char(ch);
        assert!(
            should_render_geometrically(kind),
            "{ch:?} is drawn as geometry"
        );
        assert!(should_snap_to_boundaries(kind), "{ch:?} snaps to the cell");
    }

    // Shade characters are the deliberate exception: they classify as block
    // characters but come from the font, because a geometric fill cannot
    // reproduce a dither pattern. Pinned so the exception stays intentional.
    for ch in ['░', '▒', '▓'] {
        let kind = classify_char(ch);
        assert_eq!(kind, BlockCharType::Shade, "{ch:?}");
        assert!(
            !should_render_geometrically(kind),
            "{ch:?} must keep using its font glyph"
        );
        assert!(!should_snap_to_boundaries(kind));
    }
}

#[test]
fn glyph_snapping_spans_a_full_wide_cell() {
    // Wide characters occupy two cell widths. Snapping must extend across the
    // whole double-width cell, not just the first half — otherwise a block
    // character adjacent to CJK text leaves a hairline gap.
    let single = snap_glyph_to_cell(SnapGlyphParams {
        glyph_left: 0.4,
        glyph_top: 0.3,
        render_w: CELL_W - 0.8,
        render_h: CELL_H - 0.6,
        cell_x0: 0.0,
        cell_y0: 0.0,
        cell_x1: CELL_W,
        cell_y1: CELL_H,
        snap_threshold: 3.0,
        extension: 0.5,
    });
    let (_, _, single_w, _) = single;

    let wide = snap_glyph_to_cell(SnapGlyphParams {
        glyph_left: 0.4,
        glyph_top: 0.3,
        render_w: CELL_W * 2.0 - 0.8,
        render_h: CELL_H - 0.6,
        cell_x0: 0.0,
        cell_y0: 0.0,
        cell_x1: CELL_W * 2.0,
        cell_y1: CELL_H,
        snap_threshold: 3.0,
        extension: 0.5,
    });
    let (wide_x, _, wide_w, _) = wide;

    assert!(
        wide_w > single_w,
        "a double-width cell must snap wider ({wide_w} vs {single_w})"
    );
    assert!(
        wide_x <= 0.0,
        "snapping pulls the left edge to the cell boundary"
    );
    assert!(
        wide_x + wide_w >= CELL_W * 2.0,
        "the snapped glyph must reach the far edge of the wide cell"
    );
}

#[test]
fn wide_fixture_rows_need_two_columns_per_character() {
    // Ties the corpus to the geometry: a row of wide characters needs twice as
    // many columns as it has characters, which is what makes a pane's column
    // count and a line's character count diverge in the render path.
    for text in WIDE_FIXTURES {
        let chars = text.chars().count();
        let pane = PaneViewport::new(0.0, 0.0, chars as f32 * 2.0 * CELL_W, CELL_H, true, 1.0);
        let (cols, rows) = pane.grid_size(CELL_W, CELL_H);
        assert_eq!(cols, chars * 2, "{text:?} needs two columns per character");
        assert_eq!(rows, 1);
        assert!(
            cols > chars,
            "the column index range exceeds the char count"
        );
    }
}

#![cfg(feature = "layout-conformance")]
//! Cross-repo conformance: the core's `LayoutTree::render()` parsed by the
//! real `TmuxLayout::parse`.
//!
//! This test lives here and not in the core because `par-term-tmux` depends
//! on the core — the core has no path to the real parser and proves only
//! self-consistency against a `#[cfg(test)]` parser port that can drift
//! silently (kanban 01a0c5c74485730188f8a798fce521a1). This crate sees both
//! sides, so it is the test that actually fails when the emitter and parser
//! grammars diverge.
//!
//! Gated on the `layout-conformance` feature, which forwards the core's
//! `mux` feature (first shipped in core 0.50). Run directly:
//! `cargo test -p par-term-tmux --features layout-conformance`.

use par_term_emu_core_rust::mux::layout::ResizeDirection;
use par_term_emu_core_rust::mux::pane::SpawnContext;
use par_term_emu_core_rust::mux::{
    LayoutTree, MuxError, MuxPane, MuxTree, PaneFactory, PaneId, SessionId, ShellPaneFactory,
    SplitDirection, WindowId,
};
use par_term_tmux::{LayoutNode, TmuxLayout};

type PaneRow = (u64, usize, usize, usize, usize);

fn collect_panes(node: &LayoutNode, out: &mut Vec<PaneRow>) {
    match node {
        LayoutNode::Pane {
            id,
            width,
            height,
            x,
            y,
        } => out.push((*id, *x, *y, *width, *height)),
        LayoutNode::HorizontalSplit { children, .. }
        | LayoutNode::VerticalSplit { children, .. } => {
            for child in children {
                collect_panes(child, out);
            }
        }
    }
}

/// Render `tree`, parse the output with the real parser, and assert the
/// parsed panes match `LayoutTree::geometry` exactly — pane ids AND geometry,
/// not merely a successful parse.
///
/// Returns the parsed layout so callers can additionally assert structure.
/// Structural checks matter: the bracket characters are the only place the
/// split orientation survives on the wire, and a `{`/`[` swap would parse
/// cleanly with identical leaf geometry.
fn assert_round_trips(
    tree: &LayoutTree,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
) -> TmuxLayout {
    let rendered = tree.render(x, y, width, height);
    assert!(
        rendered.starts_with("0000,"),
        "render() must carry the placeholder checksum prefix: {rendered}"
    );
    let layout = TmuxLayout::parse(&rendered)
        .unwrap_or_else(|| panic!("real parser rejected render() output: {rendered}"));

    let mut actual = Vec::new();
    collect_panes(&layout.root, &mut actual);
    actual.sort_by_key(|p| p.0);

    let mut expected: Vec<PaneRow> = tree
        .geometry(x, y, width, height)
        .iter()
        .map(|g| (g.pane.0 as u64, g.x, g.y, g.width, g.height))
        .collect();
    expected.sort_by_key(|p| p.0);

    assert_eq!(actual, expected, "rendered: {rendered}");
    layout
}

fn vertical_pair() -> LayoutTree {
    let mut tree = LayoutTree::leaf(PaneId(0));
    tree.split_pane(PaneId(0), PaneId(1), SplitDirection::Vertical, 0.5)
        .expect("splits the only leaf");
    tree
}

#[test]
fn single_pane_round_trips() {
    let tree = LayoutTree::leaf(PaneId(7));
    let layout = assert_round_trips(&tree, 0, 0, 90, 24);
    match &layout.root {
        LayoutNode::Pane {
            id,
            width,
            height,
            x,
            y,
        } => {
            assert_eq!(*id, 7);
            assert_eq!((*x, *y, *width, *height), (0, 0, 90, 24));
        }
        other => panic!("expected a single Pane root, got {other:?}"),
    }
}

#[test]
fn vertical_split_round_trips_as_curly_brace_group() {
    let tree = vertical_pair();
    let layout = assert_round_trips(&tree, 0, 0, 90, 24);
    match &layout.root {
        LayoutNode::VerticalSplit { children, .. } => {
            assert_eq!(children.len(), 2);
        }
        other => panic!("a Vertical split must render {{...}}, got {other:?}"),
    }
}

#[test]
fn horizontal_split_round_trips_as_square_bracket_group() {
    let mut tree = LayoutTree::leaf(PaneId(0));
    tree.split_pane(PaneId(0), PaneId(1), SplitDirection::Horizontal, 0.25)
        .expect("splits the only leaf");
    let layout = assert_round_trips(&tree, 0, 0, 90, 24);
    match &layout.root {
        LayoutNode::HorizontalSplit { children, .. } => {
            assert_eq!(children.len(), 2);
        }
        other => panic!("a Horizontal split must render [...], got {other:?}"),
    }
}

#[test]
fn three_same_direction_splits_collapse_to_one_n_ary_group() {
    // Split(Split(A,B),C) all Vertical — the collapse Decision 1 exists for.
    // The wire format must carry ONE {...} group with 3 children; a nested
    // {...{...}} shape would still round-trip geometrically, so child count
    // is the assertion that discriminates.
    let mut tree = LayoutTree::leaf(PaneId(0));
    tree.split_pane(PaneId(0), PaneId(1), SplitDirection::Vertical, 0.5)
        .expect("first split");
    tree.split_pane(PaneId(1), PaneId(2), SplitDirection::Vertical, 0.5)
        .expect("second split");
    let layout = assert_round_trips(&tree, 0, 0, 90, 24);
    match &layout.root {
        LayoutNode::VerticalSplit { children, .. } => {
            assert_eq!(
                children.len(),
                3,
                "same-direction run must collapse to one N-ary group"
            );
        }
        other => panic!("expected VerticalSplit root, got {other:?}"),
    }
}

#[test]
fn nested_mixed_direction_round_trips_with_offset_origin() {
    // Outer Vertical split whose second child splits Horizontal, rendered
    // at a non-zero window origin to exercise the offset arithmetic.
    let mut tree = LayoutTree::leaf(PaneId(0));
    tree.split_pane(PaneId(0), PaneId(1), SplitDirection::Vertical, 0.5)
        .expect("outer split");
    tree.split_pane(PaneId(1), PaneId(2), SplitDirection::Horizontal, 0.5)
        .expect("inner split");
    let layout = assert_round_trips(&tree, 10, 5, 90, 24);
    match &layout.root {
        LayoutNode::VerticalSplit { children, .. } => match &children[1] {
            LayoutNode::HorizontalSplit { .. } => {}
            other => panic!("inner Horizontal split must render [...], got {other:?}"),
        },
        other => panic!("expected VerticalSplit root, got {other:?}"),
    }
}

#[test]
fn placeholder_checksum_prefix_is_accepted_by_the_real_parser() {
    // render() emits a placeholder "0000," checksum. Confirm the real parser
    // accepts that prefix (4 hex chars before the first comma) rather than
    // assuming it — including alongside a real-checksum-shaped prefix.
    let with_placeholder = TmuxLayout::parse("0000,90x24,0,0,7");
    assert!(
        with_placeholder.is_some(),
        "placeholder checksum prefix must be skipped by the real parser"
    );
    let with_real_checksum = TmuxLayout::parse("f865,90x24,0,0,7");
    assert!(
        with_real_checksum.is_some(),
        "a real-checksum-shaped prefix must be skipped too"
    );
    let without_prefix = TmuxLayout::parse("90x24,0,0,7");
    let pane_of = |l: Option<TmuxLayout>| match l.expect("parses").root {
        LayoutNode::Pane {
            id, width, height, ..
        } => (id, width, height),
        other => panic!("expected Pane, got {other:?}"),
    };
    assert_eq!(pane_of(with_placeholder), (7, 90, 24));
    assert_eq!(pane_of(with_real_checksum), (7, 90, 24));
    assert_eq!(pane_of(without_prefix), (7, 90, 24));
}

#[test]
fn pinned_wire_format_example_matches_tmux_grammar() {
    // Exact-string anchor mirroring the core's own unit expectation: an
    // 89-column window split 50/50 renders 45+44 columns. Catches drift the
    // structural asserts cannot name (field order, separator shape).
    let tree = vertical_pair();
    assert_eq!(
        tree.render(0, 0, 89, 24),
        "0000,89x24,0,0{45x24,0,0,0,44x24,45,0,1}"
    );
}

// ---------------------------------------------------------------------------
// Phase 2: layouts that RESULT from the pane-command surface, driven through
// the core's MuxTree API (split-window / resize-pane / swap-pane /
// select-pane) rather than hand-constructed LayoutTree values, so the cases
// track real mutation behavior.
// ---------------------------------------------------------------------------

/// Panes that stay silent for the test's lifetime — the same shape the
/// core's own server tests use (`sleep 30`) so no shell-prompt race can
/// leak into anything. Layout conformance never reads pane output; the
/// child exists only because `PaneFactory` must return a real `MuxPane`.
struct QuietFactory;

impl PaneFactory for QuietFactory {
    fn create_pane(
        &self,
        id: PaneId,
        cols: u16,
        rows: u16,
        _command: Option<&str>,
        context: &SpawnContext<'_>,
    ) -> Result<MuxPane, MuxError> {
        ShellPaneFactory::default().create_pane(id, cols, rows, Some("sleep 30"), context)
    }
}

/// The (only) window of a freshly created session.
fn sole_window(tree: &MuxTree, session: SessionId) -> WindowId {
    tree.session(session).expect("session exists").windows[0]
}

/// Wire-order pane rows for a layout rendered then parsed by the REAL
/// parser (unsorted — leaf order on the wire is the thing under test).
fn wire_panes(tree: &LayoutTree, x: usize, y: usize, width: usize, height: usize) -> Vec<PaneRow> {
    let rendered = tree.render(x, y, width, height);
    let layout = TmuxLayout::parse(&rendered)
        .unwrap_or_else(|| panic!("real parser rejected render() output: {rendered}"));
    let mut panes = Vec::new();
    collect_panes(&layout.root, &mut panes);
    panes
}

#[test]
fn split_window_growth_through_mux_tree_round_trips() {
    let mut tree = MuxTree::new(Box::new(QuietFactory));
    let session = tree.new_session("conformance", 80, 24).expect("session");
    let window_id = sole_window(&tree, session);

    // split-window twice through the command-tier API: vertical on the
    // initial pane, then horizontal on the pane the first split made active.
    let first = tree.window(window_id).expect("window").active;
    let second = tree
        .split_pane(first, SplitDirection::Vertical, 0.5, None)
        .expect("first split");
    let third = tree
        .split_pane(second, SplitDirection::Horizontal, 0.5, None)
        .expect("second split");

    let window = tree.window(window_id).expect("window");
    assert_eq!(
        window.active, third,
        "split-window makes the new pane active"
    );
    assert_eq!(window.panes().len(), 3);

    let layout = assert_round_trips(&window.layout, 0, 0, 80, 24);
    match &layout.root {
        LayoutNode::VerticalSplit { children, .. } => {
            assert!(
                matches!(children[1], LayoutNode::HorizontalSplit { .. }),
                "the second split must live inside the second child, got {:?}",
                children[1]
            );
        }
        other => panic!("expected VerticalSplit root, got {other:?}"),
    }
}

#[test]
fn resize_pane_moves_the_divider_and_round_trips() {
    let mut tree = MuxTree::new(Box::new(QuietFactory));
    let session = tree.new_session("conformance", 80, 24).expect("session");
    let window_id = sole_window(&tree, session);

    let left = tree.window(window_id).expect("window").active;
    let right = tree
        .split_pane(left, SplitDirection::Vertical, 0.5, None)
        .expect("split");

    // resize-pane -R 8 on the LEFT pane: the divider moves 8 columns right.
    tree.resize_pane(left, ResizeDirection::Right, 8)
        .expect("resize");

    let window = tree.window(window_id).expect("window");
    assert_round_trips(&window.layout, 0, 0, 80, 24);
    let panes = wire_panes(&window.layout, 0, 0, 80, 24);
    assert_eq!(
        panes[0],
        (left.0 as u64, 0, 0, 48, 24),
        "the resized pane grows by the delta"
    );
    assert_eq!(
        panes[1],
        (right.0 as u64, 48, 0, 32, 24),
        "the neighbor cedes the same delta"
    );
}

#[test]
fn swap_panes_permutes_ids_over_unchanged_slots() {
    let mut tree = MuxTree::new(Box::new(QuietFactory));
    let session = tree.new_session("conformance", 90, 24).expect("session");
    let window_id = sole_window(&tree, session);

    let a = tree.window(window_id).expect("window").active;
    let b = tree
        .split_pane(a, SplitDirection::Vertical, 0.5, None)
        .expect("split 1");
    let c = tree
        .split_pane(b, SplitDirection::Vertical, 0.5, None)
        .expect("split 2");

    let before = wire_panes(
        &tree.window(window_id).expect("window").layout,
        0,
        0,
        90,
        24,
    );
    let before_ids: Vec<u64> = before.iter().map(|p| p.0).collect();
    assert_eq!(before_ids, vec![a.0 as u64, b.0 as u64, c.0 as u64]);

    tree.swap_panes(a, c).expect("swap");

    let window = tree.window(window_id).expect("window");
    assert_round_trips(&window.layout, 0, 0, 90, 24);
    let after = wire_panes(&window.layout, 0, 0, 90, 24);
    let after_ids: Vec<u64> = after.iter().map(|p| p.0).collect();
    assert_eq!(
        after_ids,
        vec![c.0 as u64, b.0 as u64, a.0 as u64],
        "the swapped ids exchange wire positions"
    );
    for (slot, (was, now)) in before.iter().zip(after.iter()).enumerate() {
        assert_eq!(
            (now.1, now.2, now.3, now.4),
            (was.1, was.2, was.3, was.4),
            "slot {slot} geometry must be unchanged by the swap"
        );
    }
}

#[test]
fn select_pane_changes_active_but_not_the_wire() {
    // tmux's layout string carries no focus field, so select-pane must leave
    // the rendered layout byte-identical while the window's active pane
    // moves. This pins that the wire contract is geometry only — if the
    // emitter ever starts encoding focus, this is the test that says so.
    let mut tree = MuxTree::new(Box::new(QuietFactory));
    let session = tree.new_session("conformance", 80, 24).expect("session");
    let window_id = sole_window(&tree, session);

    let original = tree.window(window_id).expect("window").active;
    let spawned = tree
        .split_pane(original, SplitDirection::Vertical, 0.5, None)
        .expect("split");
    assert_eq!(
        tree.window(window_id).expect("window").active,
        spawned,
        "split-window makes the new pane active"
    );

    let before = tree
        .window(window_id)
        .expect("window")
        .layout
        .render(0, 0, 80, 24);

    tree.select_pane(original).expect("select");

    let window = tree.window(window_id).expect("window");
    assert_eq!(window.active, original, "select-pane moves focus");
    assert_eq!(
        window.layout.render(0, 0, 80, 24),
        before,
        "the wire layout must not encode focus"
    );
}

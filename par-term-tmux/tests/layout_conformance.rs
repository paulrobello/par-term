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
//! Gated on the `layout-conformance` feature (declared EMPTY in this
//! crate's Cargo.toml — see its comment): forwarding the core's `mux`
//! feature, first present in core >= 0.50 and not yet on crates.io, breaks
//! cargo resolution against crates.io 0.49 even with the feature disabled,
//! because cargo validates `[features]` dep-forwarding eagerly. Local run:
//! apply the vendoring recipe from the repo CLAUDE.md (patch + pin raise),
//! extend the feature locally to `["par-term-emu-core-rust/mux"]`, then
//! `cargo test -p par-term-tmux --features layout-conformance`. Once the
//! core publishes >= 0.50, the forwarding entry and pin can be committed
//! and this runs in CI.

use par_term_emu_core_rust::mux::{LayoutTree, PaneId, SplitDirection};
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

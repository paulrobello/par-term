//! Property tests for pane geometry (UX.md P3 criteria 2, 3, 5).
//!
//! No property-testing crate is a dependency, so trees and operation
//! sequences come from a fixed-seed xorshift generator: every run explores
//! the same few thousand cases and a failure message names the seed. Each
//! test drives the same `PaneManager` methods the keyboard, drag, split,
//! equalize, and preset paths call — never a parallel model.

use super::PaneManager;
use crate::pane::LayoutPreset;
use crate::pane::types::{NavigationDirection, Pane, PaneBounds, PaneId, PaneNode, SplitDirection};
use par_term_terminal::TerminalManager;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::RwLock;

/// Cell size and divider width of a 2x display (8x16 logical cells).
const CELL_W: f32 = 16.0;
const CELL_H: f32 = 32.0;
const DIVIDER: f32 = 4.0;
const MIN_CELLS: usize = 10;
/// Per-pane pixels holding no cells: 2x(1 + 1) px padding on each side
/// plus a 24 px scrollbar across; 2x(1 + 1) px padding plus a 40 px title
/// bar down — what `pane_cell_overhead` computes for a 2x display.
const OVER_W: f32 = 8.0 + 24.0;
const OVER_H: f32 = 8.0 + 40.0;

/// Cells a pane displays, by the render gather's formula.
fn displayed_cells(b: PaneBounds) -> (usize, usize) {
    (
        ((b.width - OVER_W) / CELL_W).floor().max(0.0) as usize,
        ((b.height - OVER_H) / CELL_H).floor().max(0.0) as usize,
    )
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn unit(&mut self) -> f32 {
        (self.next() % 10_000) as f32 / 10_000.0
    }
}

fn stub_pane(id: PaneId) -> Pane {
    let terminal = TerminalManager::new_with_scrollback(4, 2, 0).expect("stub terminal");
    Pane::new_wrapping_terminal(
        id,
        Arc::new(RwLock::new(terminal)),
        None,
        Arc::new(AtomicBool::new(false)),
    )
}

fn direction(rng: &mut Rng) -> SplitDirection {
    if rng.below(2) == 0 {
        SplitDirection::Vertical
    } else {
        SplitDirection::Horizontal
    }
}

/// A random tree of `leaves` panes with random ratios, built directly so
/// ratios outside 10–90% are reachable (as a preset makes them).
fn random_tree(rng: &mut Rng, leaves: usize, next_id: &mut PaneId) -> PaneNode {
    if leaves == 1 {
        let id = *next_id;
        *next_id += 1;
        return PaneNode::leaf(stub_pane(id));
    }
    let left = 1 + rng.below(leaves - 1);
    PaneNode::Split {
        direction: direction(rng),
        ratio: 0.05 + 0.9 * rng.unit(),
        first: Box::new(random_tree(rng, left, next_id)),
        second: Box::new(random_tree(rng, leaves - left, next_id)),
    }
}

fn manager(root: PaneNode, bounds: PaneBounds) -> PaneManager {
    let ids = root.all_pane_ids();
    let mut pm = PaneManager::new();
    pm.next_pane_id = ids.iter().max().copied().unwrap_or(0) + 1;
    pm.focused_pane_id = ids.first().copied();
    pm.root = Some(root);
    pm.divider_width = DIVIDER;
    pm.set_min_pane_size(MIN_CELLS, (CELL_W, CELL_H), (OVER_W, OVER_H));
    pm.set_bounds(bounds);
    pm
}

fn window(rng: &mut Rng) -> PaneBounds {
    PaneBounds::new(
        0.0,
        0.0,
        1200.0 + 2400.0 * rng.unit(),
        800.0 + 1400.0 * rng.unit(),
    )
}

fn random_direction(rng: &mut Rng) -> NavigationDirection {
    [
        NavigationDirection::Left,
        NavigationDirection::Right,
        NavigationDirection::Up,
        NavigationDirection::Down,
    ][rng.below(4)]
}

/// The pixel position of the border of `pane` that an arrow moves: its
/// right edge for Left/Right on a side-by-side split, else its bottom
/// edge — whichever divider `resize_toward` moved is measured by the pane
/// edge on that axis changing.
fn edges(pm: &PaneManager, pane: PaneId) -> (f32, f32, f32, f32) {
    let b = pm.get_pane(pane).expect("pane").bounds;
    (b.x, b.x + b.width, b.y, b.y + b.height)
}

// ---------------------------------------------------------------------------
// Criterion 3: an arrow resize moves the divider in the arrow's direction.
// ---------------------------------------------------------------------------

/// Every tree of 2..=8 panes, every pane, every arrow: when the divider
/// moves, the focused pane's border on that axis moved WITH the arrow
/// (Right/Down: an edge moved right/down; Left/Up: left/up), the other axis
/// did not change, and no other border moved against the arrow.
#[test]
fn an_arrow_resize_moves_the_divider_in_the_arrows_direction() {
    // The same seed rebuilds the same tree and window for every case.
    let build = |seed: u64| {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let leaves = 2 + rng.below(7);
        let mut next = 1;
        let tree = random_tree(&mut rng, leaves, &mut next);
        let bounds = window(&mut rng);
        manager(tree, bounds)
    };
    let mut cases = 0;
    let mut moved_per_dir = [0usize; 4];
    for seed in 1..=300u64 {
        let ids = build(seed).root.as_ref().unwrap().all_pane_ids();
        for pane in ids {
            for dir in [
                NavigationDirection::Left,
                NavigationDirection::Right,
                NavigationDirection::Up,
                NavigationDirection::Down,
            ] {
                let mut pm = build(seed);
                let (x0, x1, y0, y1) = edges(&pm, pane);
                let moved = pm.resize_toward(pane, dir, 0.05);
                if moved {
                    moved_per_dir[dir as usize] += 1;
                }
                let (nx0, nx1, ny0, ny1) = edges(&pm, pane);
                let dx0 = nx0 - x0;
                let dx1 = nx1 - x1;
                let dy0 = ny0 - y0;
                let dy1 = ny1 - y1;
                let ctx = format!("seed {seed} pane {pane} {dir:?}");
                match dir {
                    NavigationDirection::Left | NavigationDirection::Right => {
                        assert!(dy0.abs() < 0.5 && dy1.abs() < 0.5, "{ctx}: height changed");
                        let sign = if dir == NavigationDirection::Right {
                            1.0
                        } else {
                            -1.0
                        };
                        assert!(
                            dx0 * sign >= -0.5 && dx1 * sign >= -0.5,
                            "{ctx}: an edge moved against the arrow ({dx0}, {dx1})"
                        );
                        if moved {
                            assert!(
                                dx0 * sign > 0.5 || dx1 * sign > 0.5,
                                "{ctx}: reported moved but no edge moved with the arrow"
                            );
                        }
                    }
                    NavigationDirection::Up | NavigationDirection::Down => {
                        assert!(dx0.abs() < 0.5 && dx1.abs() < 0.5, "{ctx}: width changed");
                        let sign = if dir == NavigationDirection::Down {
                            1.0
                        } else {
                            -1.0
                        };
                        assert!(
                            dy0 * sign >= -0.5 && dy1 * sign >= -0.5,
                            "{ctx}: an edge moved against the arrow ({dy0}, {dy1})"
                        );
                        if moved {
                            assert!(
                                dy0 * sign > 0.5 || dy1 * sign > 0.5,
                                "{ctx}: reported moved but no edge moved with the arrow"
                            );
                        }
                    }
                }
                cases += 1;
            }
        }
    }
    assert!(cases > 4000, "explored {cases} cases");
    // The direction checks above only bite when the divider moves; make
    // sure every arrow moved it in many cases, not a vacuous few.
    for (dir, moved) in moved_per_dir.iter().enumerate() {
        assert!(
            *moved > 300,
            "direction {dir}: only {moved} resizes moved a divider"
        );
    }
}

/// tmux semantics: a pane with a divider on the arrow's side moves THAT
/// divider (Right on the left pane of `A | B` grows A; Right on B, which
/// has no divider to its right, moves its left divider right and shrinks
/// B). Up/Down on a side-by-side pair does nothing.
#[test]
fn an_arrow_prefers_the_divider_on_its_side() {
    let tree = || {
        PaneNode::split(
            SplitDirection::Vertical,
            0.5,
            PaneNode::leaf(stub_pane(1)),
            PaneNode::leaf(stub_pane(2)),
        )
    };
    let bounds = PaneBounds::new(0.0, 0.0, 1600.0, 900.0);

    let mut pm = manager(tree(), bounds);
    let w1 = pm.get_pane(1).unwrap().bounds.width;
    assert!(pm.resize_toward(1, NavigationDirection::Right, 0.05));
    assert!(
        pm.get_pane(1).unwrap().bounds.width > w1,
        "A grows to the right"
    );

    let mut pm = manager(tree(), bounds);
    let x2 = pm.get_pane(2).unwrap().bounds.x;
    assert!(pm.resize_toward(2, NavigationDirection::Right, 0.05));
    assert!(
        pm.get_pane(2).unwrap().bounds.x > x2,
        "B's left divider moves right"
    );

    let mut pm = manager(tree(), bounds);
    assert!(!pm.resize_toward(1, NavigationDirection::Down, 0.05));
}

// ---------------------------------------------------------------------------
// Criterion 5: pane_min_size is never violated after any operation.
// ---------------------------------------------------------------------------

/// Random sequences of every tree operation — arrow resize, divider drag
/// to any point, split (both directions, before and after), close, swap,
/// equalize, every preset, zoom — starting from an equalized feasible
/// layout. After each operation no pane is smaller than `pane_min_size`.
#[test]
fn pane_min_size_is_never_violated_after_any_operation() {
    let mut ops = 0;
    let (mut split_ok, mut split_refused) = (0usize, 0usize);
    let (mut insert_ok, mut insert_refused) = (0usize, 0usize);
    for seed in 1..=200u64 {
        let mut rng = Rng(seed.wrapping_mul(0xD1B5_4A32_D192_ED03));
        let mut next = 1;
        let leaves = 1 + rng.below(4);
        let bounds = window(&mut rng);
        let mut pm = manager(random_tree(&mut rng, leaves, &mut next), bounds);
        pm.equalize();
        if !pm.min_size_violations().is_empty() {
            continue; // the window is too small for this tree; not a start state
        }
        for step in 0..40 {
            let op = rng.below(10);
            let focused = pm.focused_pane_id().expect("focused");
            let what = match op {
                0 => {
                    let d = random_direction(&mut rng);
                    pm.resize_toward(focused, d, 0.05 + 0.4 * rng.unit());
                    format!("resize {d:?}")
                }
                1 => {
                    let n = pm.get_dividers().len();
                    if n == 0 {
                        continue;
                    }
                    let i = rng.below(n);
                    let (x, y) = (bounds.width * rng.unit(), bounds.height * rng.unit());
                    pm.drag_divider(i, x, y);
                    format!("drag {i} to ({x}, {y})")
                }
                2 => {
                    if pm.pane_count() >= 16 {
                        continue;
                    }
                    let dir = direction(&mut rng);
                    let before = rng.below(2) == 0;
                    let id = pm.next_pane_id;
                    pm.next_pane_id += 1;
                    match pm.split_with_pane(stub_pane(id), dir, before, true, 0.5) {
                        Ok(_) => split_ok += 1,
                        Err(_) => split_refused += 1,
                    }
                    format!("split {dir:?} before={before}")
                }
                9 => {
                    // Demote: a two-pane subtree inserted beside the focused
                    // pane (PaneManager::insert_subtree_at).
                    if pm.pane_count() >= 15 {
                        continue;
                    }
                    let (a, b) = (pm.next_pane_id + 100, pm.next_pane_id + 101);
                    let subtree = PaneNode::split(
                        direction(&mut rng),
                        0.5,
                        PaneNode::leaf(stub_pane(a)),
                        PaneNode::leaf(stub_pane(b)),
                    );
                    match pm.insert_subtree_at(focused, subtree, direction(&mut rng), 0.5) {
                        Ok(_) => insert_ok += 1,
                        Err(_) => insert_refused += 1,
                    }
                    "demote insert".to_string()
                }
                3 => {
                    if pm.pane_count() < 2 {
                        continue;
                    }
                    pm.close_pane(focused);
                    "close".to_string()
                }
                4 => {
                    let ids = pm.root.as_ref().unwrap().all_pane_ids();
                    let other = ids[rng.below(ids.len())];
                    pm.swap_panes(focused, other);
                    "swap".to_string()
                }
                5 => {
                    pm.equalize();
                    "equalize".to_string()
                }
                6 => {
                    let preset = LayoutPreset::ALL[rng.below(LayoutPreset::ALL.len())];
                    pm.apply_preset(preset);
                    format!("preset {preset:?}")
                }
                7 => {
                    pm.toggle_zoom();
                    "zoom".to_string()
                }
                _ => {
                    pm.focus_cycle(rng.below(2) == 0);
                    "focus".to_string()
                }
            };
            // Zoom shows one pane over the tab; the tree underneath is what
            // must stay feasible.
            pm.unzoom();
            assert!(
                pm.min_size_violations().is_empty(),
                "seed {seed} step {step}: `{what}` left panes below the minimum: {:?} of {:?}",
                pm.min_size_violations(),
                pm.all_panes()
                    .iter()
                    .map(|p| (p.id, p.bounds.width, p.bounds.height))
                    .collect::<Vec<_>>()
            );
            for pane in pm.all_panes() {
                let (cols, rows) = displayed_cells(pane.bounds);
                assert!(
                    cols >= MIN_CELLS && rows >= MIN_CELLS,
                    "seed {seed} step {step}: `{what}` left pane {} showing {cols}x{rows} cells",
                    pane.id
                );
            }
            ops += 1;
        }
    }
    assert!(ops > 4000, "explored {ops} operations");
    // The boundary is exercised: some splits and inserts fit, some do not.
    assert!(
        split_ok > 50 && split_refused > 50,
        "splits {split_ok} ok / {split_refused} refused"
    );
    assert!(
        insert_ok > 10 && insert_refused > 10,
        "inserts {insert_ok} ok / {insert_refused} refused"
    );
}

/// A split that would make a pane smaller than the minimum is refused and
/// hands the pane back; the tree is unchanged.
#[test]
fn a_split_below_the_minimum_is_refused() {
    // 300 px wide: two 10-cell (160 px) halves plus a divider do not fit.
    let mut pm = manager(
        PaneNode::leaf(stub_pane(1)),
        PaneBounds::new(0.0, 0.0, 300.0, 900.0),
    );
    assert!(!pm.can_split(SplitDirection::Vertical));
    assert!(
        pm.split_with_pane(stub_pane(2), SplitDirection::Vertical, false, true, 0.5)
            .is_err()
    );
    assert_eq!(pm.pane_count(), 1);
    assert!(
        pm.can_split(SplitDirection::Horizontal),
        "the other axis fits"
    );
}

// ---------------------------------------------------------------------------
// Criterion 2: equalize gives every leaf an equal area within one cell,
// for every layout preset.
// ---------------------------------------------------------------------------

/// Every preset, 2..=16 panes, at the real divider width, from scrambled
/// ratios: after equalize every pane's area is the mean pane area within
/// one cell column and one cell row of its own extent
/// (`|w*h - mean| <= CELL_W*h + CELL_H*w`). Dividers make the share
/// slightly uneven (each nesting level subtracts one divider), which is
/// what the one-cell tolerance absorbs.
#[test]
fn equalize_gives_equal_areas_within_one_cell_for_every_preset() {
    let bounds = PaneBounds::new(0.0, 0.0, 3840.0, 2160.0);
    for preset in LayoutPreset::ALL {
        for n in 2..=16usize {
            let mut next = 1;
            let mut rng = Rng(n as u64 * 7919);
            let tree = random_tree(&mut rng, n, &mut next);
            let mut pm = manager(tree, bounds);
            pm.set_min_pane_size(1, (CELL_W, CELL_H), (0.0, 0.0));
            assert!(pm.apply_preset(preset), "{preset:?} n={n} applies");
            for i in 0..pm.get_dividers().len() {
                pm.drag_divider(i, bounds.width * rng.unit(), bounds.height * rng.unit());
            }
            assert!(pm.equalize(), "{preset:?} n={n} equalizes");

            let panes: Vec<PaneBounds> = pm.all_panes().iter().map(|p| p.bounds).collect();
            let mean = panes.iter().map(|b| b.width * b.height).sum::<f32>() / n as f32;
            for b in &panes {
                let area = b.width * b.height;
                let tolerance = CELL_W * b.height + CELL_H * b.width;
                assert!(
                    (area - mean).abs() <= tolerance,
                    "{preset:?} n={n}: area {area} vs mean {mean} (tolerance {tolerance}); all {:?}",
                    panes
                        .iter()
                        .map(|b| (b.width, b.height))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}

#[test]
fn split_left_and_up_place_the_new_pane_before_the_focused_one() {
    let bounds = PaneBounds::new(0.0, 0.0, 1600.0, 900.0);
    let mut pm = manager(PaneNode::leaf(stub_pane(1)), bounds);
    pm.split_with_pane(stub_pane(2), SplitDirection::Vertical, true, true, 0.5)
        .unwrap_or_else(|_| panic!("split left"));
    assert!(pm.get_pane(2).unwrap().bounds.x < pm.get_pane(1).unwrap().bounds.x);
    assert_eq!(pm.focused_pane_id(), Some(2));
    pm.focus_pane(1);
    pm.split_with_pane(stub_pane(3), SplitDirection::Horizontal, true, false, 0.5)
        .unwrap_or_else(|_| panic!("split up"));
    assert!(pm.get_pane(3).unwrap().bounds.y < pm.get_pane(1).unwrap().bounds.y);
    assert_eq!(
        pm.focused_pane_id(),
        Some(1),
        "focus_new = false keeps focus"
    );
}

#[test]
fn a_double_clicked_divider_equalizes_only_its_split() {
    let bounds = PaneBounds::new(0.0, 0.0, 1600.0, 900.0);
    let mut pm = manager(
        PaneNode::split(
            SplitDirection::Vertical,
            0.3,
            PaneNode::leaf(stub_pane(1)),
            PaneNode::split(
                SplitDirection::Horizontal,
                0.2,
                PaneNode::leaf(stub_pane(2)),
                PaneNode::leaf(stub_pane(3)),
            ),
        ),
        bounds,
    );
    let inner_before = pm.get_pane(2).unwrap().bounds.height;
    assert!(pm.equalize_divider(0));
    // Root split: 1 leaf vs 2 leaves → one third.
    let w1 = pm.get_pane(1).unwrap().bounds.width;
    assert!((w1 - (1600.0 - DIVIDER) / 3.0).abs() < 1.0, "{w1}");
    assert_eq!(
        pm.get_pane(2).unwrap().bounds.height,
        inner_before,
        "inner split kept"
    );
}

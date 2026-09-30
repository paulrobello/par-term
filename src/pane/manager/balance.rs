//! Auto-balance after a split (UX.md PN7, `split_balance`), so repeated
//! splits do not leave panes at 50/25/12.5%.

use super::PaneManager;
use crate::pane::types::{PaneId, PaneNode, SplitDirection};
use par_term_config::SplitBalance;

/// Leaves of a chain of `dir` splits: a subtree that is not a `dir` split
/// counts as one.
fn chain_units(node: &PaneNode, dir: SplitDirection) -> usize {
    match node {
        PaneNode::Split {
            direction,
            first,
            second,
            ..
        } if *direction == dir => chain_units(first, dir) + chain_units(second, dir),
        _ => 1,
    }
}

/// Give every member of the `dir` chain rooted at `node` an equal share.
fn balance_chain(node: &mut PaneNode, dir: SplitDirection) {
    if let PaneNode::Split {
        direction,
        ratio,
        first,
        second,
    } = node
        && *direction == dir
    {
        let a = chain_units(first, dir) as f32;
        let b = chain_units(second, dir) as f32;
        *ratio = a / (a + b);
        balance_chain(first, dir);
        balance_chain(second, dir);
    }
}

/// Path from `node` to the leaf `pane`: `true` = first child.
fn path_to(node: &PaneNode, pane: PaneId) -> Option<Vec<bool>> {
    match node {
        PaneNode::Leaf(p) => (p.id == pane).then(Vec::new),
        PaneNode::Split { first, second, .. } => {
            if let Some(mut path) = path_to(first, pane) {
                path.insert(0, true);
                Some(path)
            } else {
                let mut path = path_to(second, pane)?;
                path.insert(0, false);
                Some(path)
            }
        }
    }
}

/// The node reached by following `path` from `node`.
fn descend<'a>(mut node: &'a mut PaneNode, path: &[bool]) -> &'a mut PaneNode {
    for &go_first in path {
        match node {
            PaneNode::Split { first, second, .. } => {
                node = if go_first { first } else { second };
            }
            PaneNode::Leaf(_) => break,
        }
    }
    node
}

fn direction_of(node: &PaneNode) -> Option<SplitDirection> {
    match node {
        PaneNode::Split { direction, .. } => Some(*direction),
        PaneNode::Leaf(_) => None,
    }
}

impl PaneManager {
    /// Rebalance after `new_pane` was split off (PN7). `Siblings` evens out
    /// the run of same-direction splits the new pane sits in (the panes
    /// beside it along the split axis); `All` equalizes the whole tab.
    /// Refused, like equalize, when it would push a pane below the minimum
    /// size. Returns whether the layout changed.
    pub fn balance_after_split(&mut self, new_pane: PaneId, balance: SplitBalance) -> bool {
        match balance {
            SplitBalance::None => false,
            SplitBalance::All => self.equalize(),
            SplitBalance::Siblings => self.guard_ratios(|pm| {
                let Some(root) = pm.root.as_mut() else {
                    return;
                };
                let Some(path) = path_to(root, new_pane) else {
                    return;
                };
                let Some(parent_len) = path.len().checked_sub(1) else {
                    return;
                };
                let Some(dir) = direction_of(descend(root, &path[..parent_len])) else {
                    return;
                };
                // Climb while the ancestors split the same way.
                let mut top = parent_len;
                while top > 0 && direction_of(descend(root, &path[..top - 1])) == Some(dir) {
                    top -= 1;
                }
                balance_chain(descend(root, &path[..top]), dir);
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::types::{Pane, PaneBounds};
    use par_term_terminal::TerminalManager;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use tokio::sync::RwLock;

    fn stub(id: PaneId) -> Pane {
        Pane::new_wrapping_terminal(
            id,
            Arc::new(RwLock::new(
                TerminalManager::new_with_scrollback(20, 5, 0).expect("terminal"),
            )),
            None,
            Arc::new(AtomicBool::new(false)),
        )
    }

    fn widths(pm: &PaneManager) -> Vec<(PaneId, f32)> {
        pm.all_panes()
            .iter()
            .map(|p| (p.id, p.bounds.width.round()))
            .collect()
    }

    /// Three side-by-side panes from two splits of the right pane
    /// (1 | (2 | 3) at 50/50 then 50/50 = 50/25/25), with a stacked
    /// pane 4 under pane 1: `((1 / 4) | (2 | 3))`.
    fn manager() -> PaneManager {
        let mut pm = PaneManager::new();
        pm.set_divider_width(0.0);
        pm.set_root(PaneNode::split(
            SplitDirection::Vertical,
            0.5,
            PaneNode::split(
                SplitDirection::Horizontal,
                0.5,
                PaneNode::leaf(stub(1)),
                PaneNode::leaf(stub(4)),
            ),
            PaneNode::split(
                SplitDirection::Vertical,
                0.5,
                PaneNode::leaf(stub(2)),
                PaneNode::leaf(stub(3)),
            ),
        ));
        pm.set_bounds(PaneBounds::new(0.0, 0.0, 900.0, 400.0));
        pm
    }

    #[test]
    fn none_leaves_the_layout_alone() {
        let mut pm = manager();
        assert!(!pm.balance_after_split(3, SplitBalance::None));
        assert_eq!(
            widths(&pm),
            vec![(1, 450.0), (4, 450.0), (2, 225.0), (3, 225.0)]
        );
    }

    /// PN7 siblings: the side-by-side run holding pane 3 becomes thirds;
    /// the stacked pair under the first column is not an equal third of
    /// anything and keeps its own 50/50.
    #[test]
    fn siblings_evens_the_run_beside_the_new_pane() {
        let mut pm = manager();
        assert!(pm.balance_after_split(3, SplitBalance::Siblings));
        assert_eq!(
            widths(&pm),
            vec![(1, 300.0), (4, 300.0), (2, 300.0), (3, 300.0)]
        );
    }

    /// Siblings on a stacked split touches only that stack.
    #[test]
    fn siblings_on_a_stacked_split_touches_only_the_stack() {
        let mut pm = manager();
        assert!(pm.balance_after_split(4, SplitBalance::Siblings));
        assert_eq!(
            widths(&pm),
            vec![(1, 450.0), (4, 450.0), (2, 225.0), (3, 225.0)]
        );
    }

    /// PN7 all: every leaf gets an equal area (the equalize rule).
    #[test]
    fn all_equalizes_the_tab() {
        let mut pm = manager();
        assert!(pm.balance_after_split(3, SplitBalance::All));
        assert_eq!(
            widths(&pm),
            vec![(1, 450.0), (4, 450.0), (2, 225.0), (3, 225.0)]
        );
        let mut pm = PaneManager::new();
        pm.set_divider_width(0.0);
        pm.set_root(PaneNode::split(
            SplitDirection::Vertical,
            0.5,
            PaneNode::leaf(stub(1)),
            PaneNode::split(
                SplitDirection::Vertical,
                0.5,
                PaneNode::leaf(stub(2)),
                PaneNode::leaf(stub(3)),
            ),
        ));
        pm.set_bounds(PaneBounds::new(0.0, 0.0, 900.0, 400.0));
        assert!(pm.balance_after_split(3, SplitBalance::All));
        assert_eq!(widths(&pm), vec![(1, 300.0), (2, 300.0), (3, 300.0)]);
    }
}

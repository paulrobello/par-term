//! Pane zoom (UX.md A1) and focus history (A7/A8) for `PaneManager`.
//!
//! Zoom is state, not a one-off bounds override: the render path calls
//! `set_bounds` every frame, so the zoomed pane's full-tab bounds are
//! re-applied inside [`PaneManager::recalculate_bounds`]. The split tree is
//! never edited while zoomed, which is what makes unzoom restore the exact
//! prior layout. Hidden panes keep their tree bounds (and so their terminal
//! size): they get no SIGWINCH while the zoom lasts.

use super::PaneManager;
use crate::pane::types::{Pane, PaneId};

impl PaneManager {
    /// The zoomed pane, if any.
    pub fn zoomed_pane_id(&self) -> Option<PaneId> {
        self.zoomed_pane_id
    }

    /// Whether a pane currently fills the tab.
    pub fn is_zoomed(&self) -> bool {
        self.zoomed_pane_id.is_some()
    }

    /// Zoom the focused pane, or unzoom when already zoomed. A single pane
    /// has nothing to zoom over, so it stays unzoomed. Returns the new state.
    pub fn toggle_zoom(&mut self) -> bool {
        if self.zoomed_pane_id.is_some() {
            self.unzoom();
            return false;
        }
        if self.pane_count() < 2 {
            return false;
        }
        self.zoomed_pane_id = self.focused_pane_id;
        self.recalculate_bounds();
        self.zoomed_pane_id.is_some()
    }

    /// Leave zoom. Returns whether a pane was zoomed.
    pub fn unzoom(&mut self) -> bool {
        if self.zoomed_pane_id.take().is_some() {
            self.recalculate_bounds();
            true
        } else {
            false
        }
    }

    /// Mirror a daemon-side zoom (`%layout-change` zoom flag) onto this
    /// tree: the daemon owns the state, this only reflects it. The zoomed
    /// pane also becomes focused, as tmux makes a zoomed pane active.
    pub fn set_zoom_from_daemon(&mut self, pane: Option<PaneId>) {
        let pane = pane.filter(|id| self.get_pane(*id).is_some());
        if let Some(id) = pane {
            self.set_focus(id);
        }
        self.zoomed_pane_id = pane;
        self.recalculate_bounds();
    }

    /// Give the zoomed pane the whole tab. Called at the end of every
    /// bounds recalculation; clears a zoom whose pane has gone.
    pub(super) fn apply_zoom_bounds(&mut self) {
        let Some(id) = self.zoomed_pane_id else {
            return;
        };
        let total = self.total_bounds;
        match self.get_pane_mut(id) {
            Some(pane) => {
                let tree_bounds = std::mem::replace(&mut pane.bounds, total);
                self.zoomed_tree_bounds = Some(tree_bounds);
            }
            None => self.zoomed_pane_id = None,
        }
    }

    /// The panes drawn and hit-tested: the zoomed pane alone while zoomed,
    /// every pane otherwise.
    pub fn visible_panes(&self) -> Vec<&Pane> {
        match self.zoomed_pane_id.and_then(|id| self.get_pane(id)) {
            Some(pane) => vec![pane],
            None => self.all_panes(),
        }
    }

    /// The visible pane under a pixel position (zoom-aware hit test).
    pub fn visible_pane_at(&self, x: f32, y: f32) -> Option<&Pane> {
        self.visible_panes()
            .into_iter()
            .find(|pane| pane.bounds.contains(x, y))
    }

    /// Move focus to `id`, remembering the previous pane for `last_pane`.
    /// Focusing another pane leaves zoom first (A1: a focus move unzooms).
    pub(super) fn set_focus(&mut self, id: PaneId) {
        if self.focused_pane_id == Some(id) {
            return;
        }
        self.previous_focused_pane_id = self.focused_pane_id;
        self.focused_pane_id = Some(id);
        if self.zoomed_pane_id.is_some_and(|z| z != id) {
            self.unzoom();
        }
    }

    /// Focus the next (`forward`) or previous pane in tree order, wrapping.
    /// Returns the newly focused pane.
    pub fn focus_cycle(&mut self, forward: bool) -> Option<PaneId> {
        let ids = self.root.as_ref()?.all_pane_ids();
        if ids.len() < 2 {
            return None;
        }
        let current = self
            .focused_pane_id
            .and_then(|f| ids.iter().position(|id| *id == f))
            .unwrap_or(0);
        let next = if forward {
            (current + 1) % ids.len()
        } else {
            (current + ids.len() - 1) % ids.len()
        };
        self.set_focus(ids[next]);
        Some(ids[next])
    }

    /// Focus the previously focused pane (A8). Returns it, or `None` when
    /// there is no live previous pane.
    pub fn focus_last(&mut self) -> Option<PaneId> {
        let previous = self
            .previous_focused_pane_id
            .filter(|id| self.get_pane(*id).is_some())?;
        self.set_focus(previous);
        Some(previous)
    }
}

#[cfg(test)]
mod tests {
    use super::super::PaneManager;
    use crate::pane::types::SplitDirection;
    use crate::pane::types::{NavigationDirection, Pane, PaneBounds, PaneId, PaneNode};
    use par_term_terminal::TerminalManager;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use tokio::sync::RwLock;

    fn stub_pane(id: PaneId) -> Pane {
        let terminal = TerminalManager::new_with_scrollback(80, 24, 100).expect("stub terminal");
        Pane::new_wrapping_terminal(
            id,
            Arc::new(RwLock::new(terminal)),
            None,
            Arc::new(AtomicBool::new(false)),
        )
    }

    /// Three panes: 1 on the left, 2 over 3 on the right.
    fn three_panes() -> PaneManager {
        let mut pm = PaneManager::new();
        pm.root = Some(PaneNode::split(
            SplitDirection::Vertical,
            0.5,
            PaneNode::leaf(stub_pane(1)),
            PaneNode::split(
                SplitDirection::Horizontal,
                0.5,
                PaneNode::leaf(stub_pane(2)),
                PaneNode::leaf(stub_pane(3)),
            ),
        ));
        pm.focused_pane_id = Some(1);
        pm.next_pane_id = 4;
        pm.set_bounds(PaneBounds::new(0.0, 0.0, 800.0, 600.0));
        pm
    }

    #[test]
    fn zoom_gives_the_focused_pane_the_whole_tab_and_unzoom_restores_it() {
        let mut pm = three_panes();
        let before: Vec<PaneBounds> = pm.all_panes().iter().map(|p| p.bounds).collect();

        assert!(pm.toggle_zoom(), "three panes can zoom");
        assert_eq!(pm.zoomed_pane_id(), Some(1));
        assert_eq!(pm.get_pane(1).unwrap().bounds, pm.total_bounds);
        assert!(
            pm.get_dividers().is_empty(),
            "no divider is drawn while zoomed"
        );
        assert_eq!(pm.visible_panes().len(), 1);

        // The per-frame bounds refresh must not undo the zoom.
        pm.set_bounds(PaneBounds::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(pm.get_pane(1).unwrap().bounds, pm.total_bounds);

        assert!(!pm.toggle_zoom(), "second toggle unzooms");
        let after: Vec<PaneBounds> = pm.all_panes().iter().map(|p| p.bounds).collect();
        assert_eq!(before, after, "unzoom restores the exact prior layout");
    }

    #[test]
    fn a_single_pane_does_not_zoom() {
        let mut pm = PaneManager::new();
        pm.root = Some(PaneNode::leaf(stub_pane(1)));
        pm.focused_pane_id = Some(1);
        assert!(!pm.toggle_zoom());
        assert!(!pm.is_zoomed());
    }

    #[test]
    fn a_focus_move_unzooms() {
        let mut pm = three_panes();
        pm.toggle_zoom();
        pm.navigate(NavigationDirection::Right);
        assert!(!pm.is_zoomed(), "directional focus unzooms");
        assert_ne!(pm.focused_pane_id(), Some(1));

        pm.focus_pane(1);
        pm.toggle_zoom();
        pm.focus_cycle(true);
        assert!(!pm.is_zoomed(), "next-pane unzooms");

        pm.focus_pane(1);
        pm.toggle_zoom();
        pm.focus_pane(1);
        assert!(pm.is_zoomed(), "refocusing the zoomed pane keeps the zoom");
    }

    #[test]
    fn navigation_from_a_zoomed_pane_uses_the_real_layout() {
        // Zoomed pane 1 spans the tab; its neighbor to the right must still
        // be found from pane 1's tree position, not the zoomed extent.
        let mut pm = three_panes();
        pm.toggle_zoom();
        pm.navigate(NavigationDirection::Right);
        assert!(matches!(pm.focused_pane_id(), Some(2) | Some(3)));
    }

    #[test]
    fn a_split_or_close_unzooms() {
        let mut pm = three_panes();
        pm.toggle_zoom();
        pm.close_pane(3);
        assert!(!pm.is_zoomed(), "close unzooms");

        pm.toggle_zoom();
        pm.swap_panes(1, 2);
        assert!(!pm.is_zoomed(), "swap unzooms");

        pm.toggle_zoom();
        assert!(pm.is_zoomed());
        let id = pm.next_pane_id;
        pm.next_pane_id += 1;
        assert!(
            pm.split_with_pane(stub_pane(id), SplitDirection::Vertical, false, true, 0.5)
                .is_ok(),
            "split while zoomed"
        );
        assert!(!pm.is_zoomed(), "split unzooms");
    }

    #[test]
    fn the_zoom_hit_test_sees_only_the_zoomed_pane() {
        let mut pm = three_panes();
        pm.focus_pane(3);
        pm.toggle_zoom();
        // (100, 100) lies in pane 1's tree area, now covered by pane 3.
        assert_eq!(pm.visible_pane_at(100.0, 100.0).map(|p| p.id), Some(3));
        assert_eq!(pm.focus_pane_at(100.0, 100.0), Some(3));
        assert!(pm.is_zoomed(), "clicking the zoomed pane keeps the zoom");
    }

    #[test]
    fn next_previous_and_last_pane_cycle_in_tree_order() {
        let mut pm = three_panes();
        assert_eq!(pm.focus_cycle(true), Some(2));
        assert_eq!(pm.focus_cycle(true), Some(3));
        assert_eq!(pm.focus_cycle(true), Some(1), "wraps forward");
        assert_eq!(pm.focus_cycle(false), Some(3), "wraps backward");
        assert_eq!(pm.focus_last(), Some(1), "last pane toggles back");
        assert_eq!(pm.focus_last(), Some(3), "and forth");
    }

    #[test]
    fn last_pane_skips_a_closed_pane() {
        let mut pm = three_panes();
        pm.focus_pane(2);
        pm.focus_pane(3);
        pm.close_pane(2);
        assert_eq!(pm.focus_last(), None, "the previous pane is gone");
    }

    #[test]
    fn a_daemon_zoom_mirrors_and_focuses_the_zoomed_pane() {
        let mut pm = three_panes();
        pm.set_zoom_from_daemon(Some(2));
        assert_eq!(pm.zoomed_pane_id(), Some(2));
        assert_eq!(pm.focused_pane_id(), Some(2));
        assert_eq!(pm.get_pane(2).unwrap().bounds, pm.total_bounds);
        pm.set_zoom_from_daemon(None);
        assert!(!pm.is_zoomed());
        pm.set_zoom_from_daemon(Some(99));
        assert!(!pm.is_zoomed(), "an unknown pane is not zoomed");
    }
}

//! Mirror a daemon-side pane zoom (`resize-pane -Z`) onto the native tree.
//!
//! The `%layout-change` of a zoomed window carries the true (unzoomed)
//! tree plus the zoomed pane id. The layout consumers apply the tree, which
//! sizes every mirror terminal from its leaf; this step then shows the
//! zoomed pane alone and sizes its mirror to the whole window, which is the
//! size the daemon gave its PTY. Without it the mirror is drawn at its tree
//! size while the program inside redraws for the full window.

use crate::app::window_state::WindowState;
use crate::tmux::{LayoutNode, TmuxLayout, TmuxPaneId, TmuxWindowId};

/// A layout's whole extent in cells `(cols, rows)`.
pub(super) fn layout_extent(layout: &TmuxLayout) -> (usize, usize) {
    match &layout.root {
        LayoutNode::Pane { width, height, .. }
        | LayoutNode::HorizontalSplit { width, height, .. }
        | LayoutNode::VerticalSplit { width, height, .. } => (*width, *height),
    }
}

impl WindowState {
    pub(super) fn apply_daemon_zoom(
        &mut self,
        window_id: TmuxWindowId,
        layout: &TmuxLayout,
        zoomed: Option<TmuxPaneId>,
    ) {
        let Some(tab_id) = self.tmux_state.tmux_sync.get_tab(window_id) else {
            return;
        };
        let native = zoomed
            .and_then(|pane| self.tmux_state.tmux_pane_owner(pane))
            .filter(|(owner, _)| *owner == tab_id)
            .map(|(_, native)| native);
        let Some(pm) = self
            .tab_manager
            .get_tab_mut(tab_id)
            .and_then(|tab| tab.pane_manager_mut())
        else {
            return;
        };
        if native.is_none() && !pm.is_zoomed() {
            return;
        }
        pm.set_zoom_from_daemon(native);
        if let Some(pane) = native.and_then(|id| pm.get_pane(id)) {
            let (cols, rows) = layout_extent(layout);
            pane.resize_terminal(cols.max(1), rows.max(1));
        }
        self.focus_state.needs_redraw = true;
    }
}

#[cfg(test)]
mod tests {
    use super::layout_extent;
    use crate::tmux::TmuxLayout;

    #[test]
    fn the_extent_is_the_root_nodes_size() {
        let layout = TmuxLayout::parse("0000,120x40,0,0{60x40,0,0,1,59x40,61,0,2}").unwrap();
        assert_eq!(layout_extent(&layout), (120, 40));
        let single = TmuxLayout::parse("0000,80x24,0,0,3").unwrap();
        assert_eq!(layout_extent(&single), (80, 24));
    }
}

//! The tree picker's cross-window side (UX.md A15): the snapshot every
//! open picker lists, and the jump to the chosen row.
//!
//! Built in `about_to_wait` only for windows whose picker is open — the
//! `move_tab_candidates` pattern: a read pass over every window, then a
//! write pass into the ones that asked.

use winit::window::WindowId;

use super::WindowManager;
use crate::app::window_state::WindowState;
use crate::tree_picker_ui::{TreeRow, TreeSnapshot, TreeTarget};

impl WindowState {
    /// This window's rows: the window, each tab (hidden par-mux tabs
    /// included, marked), and each pane of a split tab.
    pub(crate) fn tree_rows(&self, window_id: WindowId) -> TreeSnapshot {
        let mut rows = Vec::new();
        let session = self
            .tmux_state
            .transport
            .as_ref()
            .and(self.tmux_state.tmux_session_name.as_deref());
        let active = self
            .tab_manager
            .active_tab()
            .map(|t| t.title.trim().to_string())
            .filter(|t| !t.is_empty());
        rows.push(TreeRow {
            target: TreeTarget::Window(window_id),
            depth: 0,
            label: format!("Window {}", self.window_index),
            detail: [session.map(|s| format!("par-mux {s}")), active]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · "),
        });
        for tab in self.tab_manager.tabs() {
            // The tmux gateway tab is plumbing, not a place to go.
            if tab.tmux.tmux_gateway_active {
                continue;
            }
            let mut detail: Vec<String> = Vec::new();
            if tab.is_hidden {
                detail.push("hidden".to_string());
            }
            if tab.mux_view.attached {
                detail.push("attached".to_string());
            }
            if let Some(agent) = tab.mux_view.agent_badge() {
                detail.push(agent.describe().to_string());
            }
            if let Some(cwd) = tab.get_cwd() {
                detail.push(cwd);
            }
            rows.push(TreeRow {
                target: TreeTarget::Tab(window_id, tab.id),
                depth: 1,
                label: if tab.title.trim().is_empty() {
                    "Tab".to_string()
                } else {
                    tab.title.clone()
                },
                detail: detail.join(" · "),
            });
            let Some(pm) = tab.pane_manager() else {
                continue;
            };
            if !pm.has_multiple_panes() {
                continue;
            }
            for pane in pm.all_panes() {
                let mut detail: Vec<String> = Vec::new();
                if let Some(agent) = tab.mux_view.pane_agent(pane.id) {
                    detail.push(agent.describe().to_string());
                }
                if let Some(cwd) = pane.get_cwd() {
                    detail.push(cwd);
                }
                let title = pane.get_title();
                rows.push(TreeRow {
                    target: TreeTarget::Pane(window_id, tab.id, pane.id),
                    depth: 2,
                    label: if title.trim().is_empty() {
                        format!("Pane {}", pane.id)
                    } else {
                        title
                    },
                    detail: detail.join(" · "),
                });
            }
        }
        rows
    }

    /// Show `tab_id` (re-showing a hidden par-mux tab) and focus `pane`.
    pub(crate) fn jump_to_tree_target(
        &mut self,
        tab_id: crate::tab::TabId,
        pane: Option<crate::pane::PaneId>,
    ) {
        if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
            tab.is_hidden = false;
        }
        self.switch_to_tab_id(tab_id);
        if let Some(pane) = pane
            && let Some(pm) = self
                .tab_manager
                .active_tab_mut()
                .and_then(|t| t.pane_manager_mut())
        {
            pm.focus_pane(pane);
            self.after_user_pane_focus();
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }
}

impl WindowManager {
    /// Refresh the snapshot for every window whose tree picker is open.
    pub(crate) fn refresh_tree_pickers(&mut self) {
        if !self
            .windows
            .values()
            .any(|ws| ws.overlay_ui.tree_picker_ui.visible)
        {
            return;
        }
        let mut ordered: Vec<(&WindowId, &WindowState)> = self.windows.iter().collect();
        ordered.sort_by_key(|(_, ws)| ws.window_index);
        let snapshot: TreeSnapshot = ordered
            .into_iter()
            .flat_map(|(id, ws)| ws.tree_rows(*id))
            .collect();
        for ws in self.windows.values_mut() {
            if ws.overlay_ui.tree_picker_ui.visible {
                ws.overlay_ui.tree_picker_ui.set_rows(snapshot.clone());
            }
        }
    }

    /// Apply every window's pending tree-picker jump.
    pub(crate) fn apply_tree_picker_jumps(&mut self) {
        let jumps: Vec<TreeTarget> = self
            .windows
            .values_mut()
            .filter_map(|ws| ws.overlay_ui.pending_tree_jump.take())
            .collect();
        for target in jumps {
            let (window, tab, pane) = match target {
                TreeTarget::Window(w) => (w, None, None),
                TreeTarget::Tab(w, t) => (w, Some(t), None),
                TreeTarget::Pane(w, t, p) => (w, Some(t), Some(p)),
            };
            if let (Some(tab), Some(ws)) = (tab, self.windows.get_mut(&window)) {
                ws.jump_to_tree_target(tab, pane);
            }
            self.focus_window_by_id(window);
        }
    }
}

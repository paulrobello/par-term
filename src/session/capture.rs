//! Capture current session state from live windows

use super::{SessionPaneNode, SessionState, SessionTab, SessionWindow};
use crate::app::window_state::WindowState;
use crate::pane::PaneNode;
use par_term_config::snapshot_types::TabSnapshot;
use std::collections::HashMap;
use winit::window::WindowId;

/// Capture the current session state from all open windows
pub fn capture_session(windows: &HashMap<WindowId, WindowState>) -> SessionState {
    let mut session_windows = Vec::new();

    for window_state in windows.values() {
        let Some(window) = &window_state.window else {
            continue;
        };

        // Get window position and size in logical pixels.
        // Dividing physical values by scale_factor gives scale-factor-
        // independent logical pixels that winit correctly places via
        // LogicalPosition on restore (important for mixed-DPI setups).
        // Use inner_size (content area) not outer_size (includes decorations).
        let scale = window.scale_factor();
        let window_pos = window.outer_position().unwrap_or_default();
        let inner_size = window.inner_size();

        // Capture visible tabs only — hidden tabs (e.g. tmux gateway) are
        // transient control-mode connections that should not be persisted.
        let visible_tabs = window_state.tab_manager.visible_tabs();
        let tabs: Vec<SessionTab> = visible_tabs
            .iter()
            .map(|tab| SessionTab {
                snapshot: capture_tab_snapshot(tab),
            })
            .collect();

        // active_tab_index must be relative to the visible-only list
        let active_tab_id = window_state.tab_manager.active_tab_id();
        let active_tab_index = active_tab_id
            .and_then(|id| visible_tabs.iter().position(|t| t.id == id))
            .unwrap_or(0);

        let (tmux_session_name, mux_session_name) =
            window_state.tmux_state.persisted_session_names();
        session_windows.push(SessionWindow {
            position: (
                (window_pos.x as f64 / scale) as i32,
                (window_pos.y as f64 / scale) as i32,
            ),
            size: (
                (inner_size.width as f64 / scale) as u32,
                (inner_size.height as f64 / scale) as u32,
            ),
            tabs,
            active_tab_index,
            tmux_session_name,
            mux_session_name,
        });
    }

    SessionState {
        saved_at: chrono::Utc::now().to_rfc3339(),
        windows: session_windows,
    }
}

/// Snapshot one tab for session or arrangement persistence (PN11: both keep
/// the pane tree).
pub fn capture_tab_snapshot(tab: &crate::tab::Tab) -> TabSnapshot {
    // Only capture pane_layout for multi-pane (Split) layouts. Single-pane
    // tabs use pane_layout=None so that restore uses the tab-level CWD
    // without calling restore_pane_layout(). Capturing a Leaf here would
    // cause restore_pane_layout() to spawn a second shell unnecessarily —
    // and its Pane::Drop would kill the first shell via the shared Arc,
    // leading to a window that closes on the first redraw after restore.
    let pane_layout =
        tab.pane_manager
            .as_ref()
            .and_then(|pm| pm.root())
            .and_then(|root| match root {
                PaneNode::Leaf(_) => None,
                PaneNode::Split { .. } => Some(capture_pane_node(root)),
            });
    TabSnapshot {
        cwd: tab.get_cwd(),
        title: tab.title.clone(),
        custom_color: tab.custom_color,
        user_title: if tab.user_named {
            Some(tab.title.clone())
        } else {
            None
        },
        pane_user_title: tab.sole_pane_user_title(),
        custom_icon: tab.custom_icon.clone(),
        pane_layout,
    }
}

/// Recursively capture a pane tree node into a session-serializable form
pub fn capture_pane_node(node: &PaneNode) -> SessionPaneNode {
    match node {
        PaneNode::Leaf(pane) => SessionPaneNode::Leaf {
            // Without shell integration there is no live cwd; the directory
            // the pane was started in is the next best.
            cwd: pane.get_cwd().or_else(|| pane.working_directory.clone()),
            user_title: pane.user_named.then(|| pane.title.clone()),
        },
        PaneNode::Split {
            direction,
            ratio,
            first,
            second,
        } => SessionPaneNode::Split {
            direction: *direction,
            ratio: *ratio,
            first: Box::new(capture_pane_node(first)),
            second: Box::new(capture_pane_node(second)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::capture_tab_snapshot;
    use crate::pane::{Pane, PaneNode, SplitDirection};
    use crate::tab::Tab;
    use par_term_config::snapshot_types::SessionPaneNode;
    use par_term_terminal::TerminalManager;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use tokio::sync::RwLock;

    fn stub(id: crate::pane::PaneId, cwd: &str) -> Pane {
        let terminal = TerminalManager::new_with_scrollback(20, 5, 0).expect("stub terminal");
        Pane::new_wrapping_terminal(
            id,
            Arc::new(RwLock::new(terminal)),
            Some(cwd.to_string()),
            Arc::new(AtomicBool::new(false)),
        )
    }

    fn leaf_cwd(node: &SessionPaneNode) -> Option<&str> {
        match node {
            SessionPaneNode::Leaf { cwd, .. } => cwd.as_deref(),
            SessionPaneNode::Split { .. } => None,
        }
    }

    /// PN11: the snapshot arrangements and Duplicate Tab share keeps a
    /// split tab's pane tree with each leaf's directory; a single-pane tab
    /// stores no tree.
    #[test]
    fn a_split_tab_snapshot_keeps_its_pane_tree() {
        let mut tab = Tab::new_stub(1, 1);
        assert!(
            capture_tab_snapshot(&tab).pane_layout.is_none(),
            "a single pane restores from the tab cwd"
        );

        tab.pane_manager_mut()
            .expect("pm")
            .set_root(PaneNode::split(
                SplitDirection::Vertical,
                0.3,
                PaneNode::leaf(stub(1, "/left")),
                PaneNode::split(
                    SplitDirection::Horizontal,
                    0.6,
                    PaneNode::leaf(stub(2, "/top")),
                    PaneNode::leaf(stub(3, "/bottom")),
                ),
            ));

        let Some(SessionPaneNode::Split {
            direction,
            ratio,
            first,
            second,
        }) = capture_tab_snapshot(&tab).pane_layout
        else {
            panic!("a split root is captured");
        };
        assert_eq!(direction, SplitDirection::Vertical);
        assert!((ratio - 0.3).abs() < f32::EPSILON);
        assert_eq!(leaf_cwd(&first), Some("/left"));
        let SessionPaneNode::Split {
            direction,
            first,
            second,
            ..
        } = *second
        else {
            panic!("the nested split is kept");
        };
        assert_eq!(direction, SplitDirection::Horizontal);
        assert_eq!(leaf_cwd(&first), Some("/top"));
        assert_eq!(leaf_cwd(&second), Some("/bottom"));
    }
}

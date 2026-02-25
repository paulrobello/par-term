//! Capture current session state from live windows

use super::{SessionPaneNode, SessionState, SessionTab, SessionWindow};
use crate::app::window_state::WindowState;
use crate::pane::PaneNode;
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

        // Capture tabs
        let tabs: Vec<SessionTab> = window_state
            .tab_manager
            .tabs()
            .iter()
            .map(|tab| {
                let pane_layout = tab
                    .pane_manager
                    .as_ref()
                    .and_then(|pm| pm.root())
                    .map(capture_pane_node);

                SessionTab {
                    cwd: tab.get_cwd(),
                    title: tab.title.clone(),
                    custom_color: tab.custom_color,
                    user_title: if tab.user_named {
                        Some(tab.title.clone())
                    } else {
                        None
                    },
                    custom_icon: tab.custom_icon.clone(),
                    pane_layout,
                }
            })
            .collect();

        let active_tab_index = window_state.tab_manager.active_tab_index().unwrap_or(0);

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
        });
    }

    SessionState {
        saved_at: chrono::Utc::now().to_rfc3339(),
        windows: session_windows,
    }
}

/// Recursively capture a pane tree node into a session-serializable form
pub fn capture_pane_node(node: &PaneNode) -> SessionPaneNode {
    match node {
        PaneNode::Leaf(pane) => SessionPaneNode::Leaf {
            cwd: pane.get_cwd(),
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

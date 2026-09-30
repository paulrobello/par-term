//! Pane hover focus (UX.md PN14, `pane_focus_follows_mouse`).

use crate::app::window_state::WindowState;
use crate::config::Config;
use crate::pane::{Pane, PaneBounds, PaneNode, SplitDirection};
use crate::tab::Tab;
use par_term_terminal::TerminalManager;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::RwLock;

fn stub(id: crate::pane::PaneId) -> Pane {
    Pane::new_wrapping_terminal(
        id,
        Arc::new(RwLock::new(
            TerminalManager::new_with_scrollback(20, 5, 0).expect("terminal"),
        )),
        None,
        Arc::new(AtomicBool::new(false)),
    )
}

/// Panes 1 | 2 over 800x400 (1 at x < 400), focused on 1.
fn window(hover: bool) -> WindowState {
    let mut config = Config::default();
    config.mouse.pane_focus_follows_mouse = hover;
    let runtime = Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime"),
    );
    let mut ws = WindowState::new(config, runtime);
    let mut tab = Tab::new_stub(1, 1);
    let pm = tab.pane_manager_mut().expect("pm");
    pm.set_root(PaneNode::split(
        SplitDirection::Vertical,
        0.5,
        PaneNode::leaf(stub(1)),
        PaneNode::leaf(stub(2)),
    ));
    pm.focus_pane(1);
    pm.set_bounds(PaneBounds::new(0.0, 0.0, 800.0, 400.0));
    ws.tab_manager.push_tab_for_test(tab);
    ws
}

fn focused(ws: &WindowState) -> Option<crate::pane::PaneId> {
    ws.tab_manager
        .active_tab()
        .and_then(|t| t.focused_pane_id())
}

#[test]
fn hovering_a_pane_focuses_it_only_when_enabled() {
    let mut ws = window(false);
    assert!(!ws.hover_focus_pane(600.0, 200.0), "off by default");
    assert_eq!(focused(&ws), Some(1));

    let mut ws = window(true);
    assert!(ws.hover_focus_pane(600.0, 200.0));
    assert_eq!(focused(&ws), Some(2));
    assert!(
        !ws.hover_focus_pane(610.0, 210.0),
        "moving inside the focused pane changes nothing"
    );
    assert!(ws.hover_focus_pane(100.0, 200.0));
    assert_eq!(focused(&ws), Some(1));
}

#[test]
fn hover_focus_waits_while_a_button_is_held_or_the_tab_is_zoomed() {
    let mut ws = window(true);
    if let Some(tab) = ws.tab_manager.active_tab_mut() {
        tab.active_mouse_mut().button_pressed = true;
    }
    assert!(!ws.hover_focus_pane(600.0, 200.0), "a drag keeps its pane");
    if let Some(tab) = ws.tab_manager.active_tab_mut() {
        tab.active_mouse_mut().button_pressed = false;
    }

    if let Some(pm) = ws
        .tab_manager
        .active_tab_mut()
        .and_then(|t| t.pane_manager_mut())
    {
        pm.toggle_zoom();
    }
    assert!(!ws.hover_focus_pane(600.0, 200.0), "zoom hides the others");
    assert_eq!(focused(&ws), Some(1));
}

//! UX.md P4 criterion 2: the tree picker lists windows, tabs, panes, and
//! hidden tabs, and jumps to any of them.
//!
//! Uses the real [`WindowState::tree_rows`] snapshot and
//! [`WindowState::jump_to_tree_target`] jump. The hidden tab here is a
//! par-mux tab hidden by a last-pane close (D7), attached to an in-process
//! daemon so it is exactly the state a user reaches.

use crate::app::tmux_handler::notifications::mux_test_seams::attached_window_with_tabs;
use crate::app::window_state::WindowState;
use crate::pane::{Pane, PaneNode, SplitDirection};
use crate::tab::Tab;
use crate::tree_picker_ui::{TreePickerUI, TreeTarget};
use par_term_terminal::TerminalManager;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::RwLock;
use winit::window::WindowId;

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

/// A local window: tab 1 split into panes 1 | 2, tab 2 single-pane.
fn local_window() -> WindowState {
    let runtime = Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime"),
    );
    let mut ws = WindowState::new(crate::config::Config::default(), runtime);
    let mut split = Tab::new_stub(1, 1);
    split.set_title("build");
    let pm = split.pane_manager_mut().expect("pm");
    pm.set_root(PaneNode::split(
        SplitDirection::Vertical,
        0.5,
        PaneNode::leaf(stub(1)),
        PaneNode::leaf(stub(2)),
    ));
    pm.focus_pane(1);
    ws.tab_manager.push_tab_for_test(split);
    let mut logs = Tab::new_stub(2, 2);
    logs.set_title("logs");
    ws.tab_manager.push_tab_for_test(logs);
    ws.window_index = 1;
    ws
}

#[test]
fn the_tree_lists_the_window_its_tabs_and_a_split_tabs_panes() {
    let ws = local_window();
    let w = WindowId::from(7u64);
    let rows = ws.tree_rows(w);
    let targets: Vec<TreeTarget> = rows.iter().map(|r| r.target).collect();
    assert_eq!(
        targets,
        vec![
            TreeTarget::Window(w),
            TreeTarget::Tab(w, 1),
            TreeTarget::Pane(w, 1, 1),
            TreeTarget::Pane(w, 1, 2),
            TreeTarget::Tab(w, 2),
        ],
        "window, then each tab with its panes (a single-pane tab lists none)"
    );
    assert_eq!(rows[0].label, "Window 1");
    assert_eq!(rows[1].label, "build");
    assert_eq!(rows[4].label, "logs");
}

#[test]
fn jumping_to_a_tab_or_a_pane_lands_there() {
    let mut ws = local_window();
    ws.switch_to_tab_id(2);
    ws.jump_to_tree_target(1, Some(2));
    assert_eq!(ws.tab_manager.active_tab_id(), Some(1));
    assert_eq!(
        ws.tab_manager
            .active_tab()
            .and_then(|t| t.focused_pane_id()),
        Some(2),
        "the pane row focuses its pane"
    );
    ws.jump_to_tree_target(2, None);
    assert_eq!(ws.tab_manager.active_tab_id(), Some(2));
}

/// A hidden par-mux tab is listed (marked hidden), filtering on "hidden"
/// finds it, and jumping to it re-shows it.
#[test]
fn a_hidden_mux_tab_is_listed_and_a_jump_reshows_it() {
    let (mut ws, path) = attached_window_with_tabs("p4-tree", 2);
    let hidden = ws.tab_manager.tabs()[0].id;
    ws.switch_to_tab_id(hidden);
    assert!(!ws.hide_active_mux_tab(), "hiding keeps the window");
    assert!(ws.tab_manager.get_tab(hidden).unwrap().is_hidden);
    ws.refresh_tab_mux_views();

    let w = WindowId::from(3u64);
    let rows = ws.tree_rows(w);
    let row = rows
        .iter()
        .find(|r| r.target == TreeTarget::Tab(w, hidden))
        .expect("the hidden tab has a row");
    assert!(row.detail.contains("hidden"), "{}", row.detail);
    assert!(row.detail.contains("attached"), "{}", row.detail);

    let mut picker = TreePickerUI::new();
    picker.set_rows(rows.clone());
    picker.set_query_for_test("hidden");
    let hits = picker.filtered();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].target, TreeTarget::Tab(w, hidden));

    ws.jump_to_tree_target(hidden, None);
    assert!(
        !ws.tab_manager.get_tab(hidden).unwrap().is_hidden,
        "the jump re-shows the hidden tab"
    );
    assert_eq!(ws.tab_manager.active_tab_id(), Some(hidden));
    assert_eq!(ws.hidden_mux_tab_count(), 0);
    let _ = std::fs::remove_file(&path);
}

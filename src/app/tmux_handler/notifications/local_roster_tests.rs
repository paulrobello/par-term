//! Local-pane agent roster tests: the synthetic namespace, the detector's
//! reconcile and prune paths, and the focus resolver's local branch.
//! Sibling test file to local_roster.rs (the production gate's repo
//! pattern for files near the 800-line line-count gate).

use super::agent_roster::AgentRoster;
use super::local_roster::{LocalRosterDetector, focus_local_roster_pane};
use crate::tab::TabManager;
use par_term_tmux::TmuxPaneId;
use std::time::{Duration, Instant};

#[test]
fn local_namespace_cannot_collide_with_daemon_ids() {
    // Daemon ids are plain %N counters: no real id reaches bit 63, so
    // decode must reject them and encode must always round-trip.
    for daemon_id in [0u64, 5, 1024] {
        assert!(AgentRoster::decode_local_pane(daemon_id).is_none());
    }
    let synthetic = AgentRoster::local_pane_id(3, 7);
    assert!(AgentRoster::is_local_pane(synthetic));
    assert_eq!(AgentRoster::decode_local_pane(synthetic), Some((3, 7)));
    assert!(synthetic > u32::MAX as u64, "beyond every daemon counter");
}

#[test]
fn set_local_agents_reconciles_wholesale() {
    let mut roster = AgentRoster::new();
    assert!(roster.set_local_agents(vec![(1, 1, "claude".to_string())]));
    // The identical pass again: nothing changed.
    assert!(!roster.set_local_agents(vec![(1, 1, "claude".to_string())]));
    // A changed agent on the same pane is a change.
    assert!(roster.set_local_agents(vec![(1, 1, "codex".to_string())]));
    // A pane the pass does not name is gone (closed pane / exited agent).
    assert!(roster.set_local_agents(vec![]));
    assert_eq!(roster.iter().count(), 0);
}

#[test]
fn fill_and_clear_preserve_local_rows() {
    let mut roster = AgentRoster::new();
    roster.set_local_agents(vec![(2, 1, "claude".to_string())]);
    // Daemon traffic must not wipe the local namespace...
    roster.fill_from_list(vec![]);
    assert!(roster.has_local(), "a fill says nothing about local panes");
    // ...and neither must a daemon death or detach.
    roster.clear();
    assert!(roster.has_local());
    // The local row itself is intact and still renders.
    assert_eq!(
        roster.summary_line(&|_: TmuxPaneId| true).as_deref(),
        Some("\u{1f465} 1~ working")
    );
}

#[test]
fn local_rows_render_detected_working_only() {
    let mut roster = AgentRoster::new();
    roster.set_local_agents(vec![(1, 1, "claude".to_string())]);
    let visible = |pane: TmuxPaneId| roster.local_seen(pane);
    let summary = roster.summary_line(&visible).unwrap();
    assert!(
        summary.contains("1~ working"),
        "scrape marker, working group: {summary}"
    );
    let tooltip = roster.tooltip_text(&visible).unwrap();
    assert!(
        tooltip.contains("claude · working · detected (local pane)"),
        "no synthetic number in the hover: {tooltip}"
    );
    assert!(
        tooltip.contains("local panes: detected working only"),
        "the honesty note is on the hover: {tooltip}"
    );
    let rows = roster.palette_rows(&visible);
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].label,
        "claude: detected working (local pane; no hook states)"
    );
    assert_eq!(
        rows[0].action_id,
        format!("agent-roster-focus:{}", AgentRoster::local_pane_id(1, 1))
    );
    assert_eq!(rows[0].priority, 1, "detection is never an attention tier");
}

#[test]
fn poll_without_local_panes_prunes_the_namespace() {
    let mut detector = LocalRosterDetector::new();
    let mut roster = AgentRoster::new();
    roster.set_local_agents(vec![(9, 9, "claude".to_string())]);
    let tabs = TabManager::new();
    let owners: std::collections::HashMap<TmuxPaneId, (crate::tab::TabId, crate::pane::PaneId)> =
        std::collections::HashMap::new();
    assert!(detector.poll(&tabs, &owners, &mut roster));
    assert!(
        !roster.has_local(),
        "nothing local: no synthetic row survives"
    );
    assert!(!detector.scan_in_flight(), "no worker for an empty scan");
}

/// The full poll cycle against a real local pane: a scan starts on the
/// first tick, lands off-loop, and a bare shell reports no agent. Bounded
/// wait — the worker is a process-table read, normally milliseconds.
#[test]
fn poll_lands_a_scan_and_a_bare_shell_reports_no_agent() {
    let mut ws = super::mux::tests::manners_state();
    ws.new_tab_local();
    assert!(
        !ws.tab_manager.tabs().is_empty(),
        "test window has a local tab"
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let _ = ws.tmux_state.local_roster_detector.poll(
            &ws.tab_manager,
            &ws.tmux_state.tmux_pane_owners,
            &mut ws.tmux_state.agent_roster,
        );
        if !ws.tmux_state.local_roster_detector.scan_in_flight() {
            break;
        }
        assert!(Instant::now() < deadline, "the scan never landed");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !ws.tmux_state.agent_roster.has_local(),
        "a bare shell is not an agent — detection must not invent rows"
    );
}

#[test]
fn focus_resolves_a_live_local_pane_and_rejects_a_stale_one() {
    let mut ws = super::mux::tests::manners_state();
    ws.new_tab_local();
    let tab_id = ws.tab_manager.tabs()[0].id;
    let pane_id = ws.tab_manager.tabs()[0]
        .pane_manager()
        .expect("a local tab has panes")
        .all_panes()[0]
        .id;
    let synthetic = AgentRoster::local_pane_id(tab_id, pane_id);
    assert!(focus_local_roster_pane(&mut ws, synthetic));
    assert_eq!(ws.tab_manager.active_tab().map(|t| t.id), Some(tab_id));
    assert_eq!(
        ws.tab_manager
            .active_tab()
            .and_then(|t| t.pane_manager())
            .and_then(|pm| pm.focused_pane_id()),
        Some(pane_id)
    );
    assert!(
        !focus_local_roster_pane(&mut ws, AgentRoster::local_pane_id(999, 999)),
        "a stale row resolves to nothing and the caller logs a no-op"
    );
}

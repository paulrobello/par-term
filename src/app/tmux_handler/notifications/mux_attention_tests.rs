//! UX.md P4 criteria 3 and 4 against a live in-process daemon.
//!
//! Criterion 3: next-agent-needing-attention cycles blocked then
//! done-unseen agents across tabs, and the tab and pane badges match the
//! roster. Criterion 4: the session chip shows the session name, health,
//! and hidden count, and holds the last error until dismissed.
//!
//! Roster state enters through the real push path
//! (`check_mux_notifications` → `apply_agent_pushes`): each test sends the
//! daemon a hook report on its socket, exactly as an agent's hook does.

use super::mux::tests::manners_state;
use super::mux_test_seams::{SettledSend, quiesce, wait_until};
use crate::app::window_state::WindowState;
use crate::session_chip::{MuxHealth, SessionChipAction};
use crate::tab::pane_badges::AgentAttention;
use std::io::{BufRead, BufReader, Write};
use std::time::{Duration, Instant};

fn pump(ws: &mut WindowState, what: &str, done: impl Fn(&WindowState) -> bool) {
    wait_until(what, || {
        done(ws) || {
            ws.check_tmux_notifications();
            done(ws)
        }
    });
}

fn attached(tag: &str, tabs: usize) -> (WindowState, std::path::PathBuf) {
    super::mux_test_seams::attached_window_with_tabs(tag, tabs)
}

/// Monotonic `seq` for hook reports: the daemon drops a report at or
/// below the last accepted `seq` for its pane and source.
static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Report `state` for pane `%pane` the way an agent hook does: one JSON
/// line on the daemon socket, one JSON reply.
fn report(path: &std::path::Path, pane: u64, agent: &str, state: &str) {
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let mut stream = std::os::unix::net::UnixStream::connect(path).expect("hook connect");
    let line = format!(
        "{{\"id\":{seq},\"method\":\"pane.report_agent\",\"params\":{{\"pane_id\":\"%{pane}\",\
         \"agent\":\"{agent}\",\"seq\":{seq},\"state\":\"{state}\"}}}}\n"
    );
    stream.write_all(line.as_bytes()).expect("hook write");
    let mut reply = String::new();
    BufReader::new(stream)
        .read_line(&mut reply)
        .expect("hook reply");
    assert!(
        !reply.contains("\"error\""),
        "the daemon refused the hook report: {reply}"
    );
}

fn roster_state(ws: &WindowState, pane: u64) -> Option<String> {
    ws.tmux_state
        .agent_roster
        .iter()
        .find(|e| e.pane == pane)
        .map(|e| e.state.clone())
}

fn tab_of(ws: &WindowState, pane: u64) -> crate::tab::TabId {
    ws.tmux_state.tmux_pane_owner(pane).expect("mapped").0
}

fn focused_mux_pane(ws: &WindowState) -> Option<u64> {
    ws.focused_mux_pane_from_native()
}

/// Criterion 3: blocked agents first (in tab order), then done-unseen;
/// badges on tabs and pane titles follow the roster.
#[test]
fn next_attention_cycles_blocked_then_done_unseen_and_badges_match_the_roster() {
    let (mut ws, path) = attached("p4-attn", 3);
    // %0 in tab 1, %1 in tab 2, %2 in tab 3. Start on tab 1.
    ws.switch_to_tab_id(tab_of(&ws, 0));

    // %2 finishes while the user is on tab 1: working → idle marks it
    // done-unseen. %1 blocks. %0 works.
    report(&path, 2, "claude", "working");
    pump(&mut ws, "%2 working", |ws| {
        roster_state(ws, 2).as_deref() == Some("working")
    });
    report(&path, 2, "claude", "idle");
    report(&path, 1, "codex", "blocked");
    report(&path, 0, "pi", "working");
    pump(&mut ws, "roster pushes", |ws| {
        roster_state(ws, 2).as_deref() == Some("idle")
            && roster_state(ws, 1).as_deref() == Some("blocked")
            && roster_state(ws, 0).as_deref() == Some("working")
    });
    assert!(ws.tmux_state.agent_roster.is_done_unseen(2));

    // Badges match the roster, per tab and per pane.
    ws.refresh_tab_mux_views();
    let view = |ws: &WindowState, pane: u64| {
        ws.tab_manager
            .get_tab(tab_of(ws, pane))
            .expect("tab")
            .mux_view
            .clone()
    };
    assert_eq!(view(&ws, 0).agent_badge(), Some(AgentAttention::Working));
    assert_eq!(view(&ws, 1).agent_badge(), Some(AgentAttention::Blocked));
    assert_eq!(view(&ws, 2).agent_badge(), Some(AgentAttention::DoneUnseen));
    let native_2 = ws.tmux_state.tmux_pane_owner(2).unwrap().1;
    assert_eq!(
        view(&ws, 2).pane_agent(native_2),
        Some(AgentAttention::DoneUnseen),
        "the pane title badge reads the same roster state"
    );
    assert!(
        view(&ws, 0).attached,
        "an attached tab carries the attached badge"
    );

    // First press: the blocked agent (%1, tab 2).
    assert!(ws.execute_keybinding_action("focus_next_attention_agent"));
    assert_eq!(ws.tab_manager.active_tab_id(), Some(tab_of(&ws, 1)));
    assert_eq!(focused_mux_pane(&ws), Some(1));

    // Second press: the done-unseen agent (%2, tab 3); focusing it marks it
    // seen, and its badge drops to none (idle).
    assert!(ws.execute_keybinding_action("focus_next_attention_agent"));
    assert_eq!(ws.tab_manager.active_tab_id(), Some(tab_of(&ws, 2)));
    assert_eq!(focused_mux_pane(&ws), Some(2));
    assert!(
        !ws.tmux_state.agent_roster.is_done_unseen(2),
        "seen once focused"
    );
    ws.refresh_tab_mux_views();
    assert_eq!(view(&ws, 2).agent_badge(), None, "seen and idle: no badge");

    // Third press: only the blocked agent is left, so the cycle wraps to it.
    assert!(ws.execute_keybinding_action("focus_next_attention_agent"));
    assert_eq!(focused_mux_pane(&ws), Some(1));

    // Unblocked: nothing needs attention, the press stays put.
    report(&path, 1, "codex", "working");
    pump(&mut ws, "%1 working", |ws| {
        roster_state(ws, 1).as_deref() == Some("working")
    });
    let before = ws.tab_manager.active_tab_id();
    ws.execute_keybinding_action("focus_next_attention_agent");
    assert_eq!(ws.tab_manager.active_tab_id(), before);
    let _ = std::fs::remove_file(&path);
}

/// UX.md U11 and U12: a reordered par-mux tab moves its daemon window
/// (`move-window`), so `list-windows` — the order reattach rebuilds — follows
/// the tab bar; clearing a tab's name sends the re-derived auto title
/// instead of leaving the old name daemon-side.
#[test]
fn tab_reorder_and_blank_rename_reach_the_daemon() {
    let (mut ws, path) = attached("p4-order", 2);
    let daemon_order = |ws: &WindowState| -> Vec<u64> {
        ws.tmux_state
            .transport
            .as_ref()
            .unwrap()
            .send_settled("list-windows")
            .unwrap()
            .iter()
            .filter_map(|l| l.strip_prefix('@')?.split(':').next()?.parse().ok())
            .collect()
    };
    let before = daemon_order(&ws);
    assert_eq!(before.len(), 2);
    let first_tab = ws.tab_manager.tabs()[0].id;
    let first_window = ws.mux_window_for_tab(first_tab).unwrap();
    ws.switch_to_tab_id(first_tab);
    quiesce(&mut ws);
    ws.move_tab_right();
    assert_eq!(ws.tab_manager.tabs()[1].id, first_tab, "moved locally");
    assert_eq!(
        daemon_order(&ws).last(),
        Some(&first_window),
        "the daemon's window list follows the tab bar: {:?}",
        daemon_order(&ws)
    );

    // U12: a blank rename sends the auto title, never an empty name.
    quiesce(&mut ws);
    ws.handle_tab_bar_action_after_render(crate::tab_bar_ui::TabBarAction::RenameTab(
        first_tab,
        "custom".to_string(),
    ));
    let name_of = |ws: &WindowState, window: u64| -> String {
        ws.tmux_state
            .transport
            .as_ref()
            .unwrap()
            .send_settled("list-windows")
            .unwrap()
            .iter()
            .find_map(|l| {
                let (id, name) = l.strip_prefix('@')?.split_once(": ")?;
                (id.parse::<u64>().ok()? == window).then(|| name.to_string())
            })
            .unwrap_or_default()
    };
    assert_eq!(name_of(&ws, first_window), "custom");
    quiesce(&mut ws);
    ws.handle_tab_bar_action_after_render(crate::tab_bar_ui::TabBarAction::RenameTab(
        first_tab,
        String::new(),
    ));
    let auto = ws.tab_manager.get_tab(first_tab).unwrap().title.clone();
    assert!(!auto.is_empty());
    assert_ne!(
        name_of(&ws, first_window),
        "custom",
        "the daemon no longer holds the cleared name"
    );
    assert_eq!(name_of(&ws, first_window), auto.trim());
    let _ = std::fs::remove_file(&path);
}

/// Criterion 4: the chip shows the session name, health and hidden count,
/// and keeps the last par-mux error — through the view ending — until the
/// user dismisses it.
#[test]
fn the_session_chip_shows_state_and_holds_the_last_error_until_dismissed() {
    let (mut ws, path) = attached("p4-chip", 2);
    let chip = ws.session_chip();
    assert_eq!(chip.session.as_deref(), Some("p4-chip"));
    assert_eq!(chip.health, MuxHealth::Connected);
    assert_eq!(chip.hidden_tabs, 0);
    assert!(chip.label().contains("p4-chip"));

    // Hide one tab (last-pane close keeps its daemon window, D7).
    ws.switch_to_tab_id(tab_of(&ws, 1));
    assert!(!ws.hide_active_mux_tab());
    let chip = ws.session_chip();
    assert_eq!(chip.hidden_tabs, 1);
    assert!(chip.label().contains("1 hidden"), "{}", chip.label());

    // A par-mux error lands on the chip.
    ws.record_mux_error("par-mux: split failed — test");
    assert_eq!(
        ws.session_chip().last_error.as_deref(),
        Some("par-mux: split failed — test")
    );

    // The daemon dies: the view ends, the chip keeps the held error.
    ws.tmux_state.transport = None;
    ws.handle_tmux_session_ended_for_test();
    let chip = ws.session_chip();
    assert!(chip.session.is_none(), "no session once the view ended");
    assert_eq!(
        chip.last_error.as_deref(),
        Some("par-mux: daemon connection lost")
    );
    assert!(chip.is_visible(), "the error keeps the chip on screen");

    // Nothing but a dismiss clears it: a later toast does not.
    ws.show_toast("unrelated");
    assert!(ws.session_chip().last_error.is_some());
    ws.handle_session_chip_action(SessionChipAction::DismissError);
    assert!(ws.session_chip().last_error.is_none());
    assert!(!ws.session_chip().is_visible());
    let _ = std::fs::remove_file(&path);
}

/// Criterion 4, health: an unresponsive daemon shows on the chip until the
/// transport reports recovery.
#[test]
fn the_chip_reports_attaching_and_unresponsive_health() {
    let mut ws = manners_state();
    let (_tx, rx) = std::sync::mpsc::channel();
    ws.tmux_state.mux_attach_pending = Some(super::mux::MuxAttachPending {
        name: "pending".to_string(),
        rx,
    });
    let chip = ws.session_chip();
    assert_eq!(chip.session.as_deref(), Some("pending"));
    assert_eq!(chip.health, MuxHealth::Attaching);
    assert!(chip.label().contains("attaching"));

    ws.tmux_state.mux_attach_pending = None;
    let (mut ws, path) = attached("p4-health", 1);
    ws.tmux_state.mux_health = MuxHealth::Unresponsive;
    assert!(ws.session_chip().label().contains("not responding"));
    let _ = std::fs::remove_file(&path);
}

/// UX.md OV7: every par-mux error is an error toast that persists until
/// dismissed — not only the split failure. Before MP1 Q3 the rest posted
/// as two-second info toasts and faded before the user could read them.
#[test]
fn every_mux_error_is_a_persistent_error_toast() {
    use crate::app::overlay::toast::{TOAST_LIFETIME, ToastKind};
    let mut ws = manners_state();
    ws.record_mux_error("par-mux: close failed — test");
    let toast = ws.overlay_state.toasts.newest().expect("the error toasts");
    assert_eq!(toast.kind, ToastKind::Error);
    assert_eq!(toast.expires, None, "an error never expires");
    ws.overlay_state
        .toasts
        .expire(Instant::now() + TOAST_LIFETIME + Duration::from_secs(60));
    assert_eq!(
        ws.last_toast_text(),
        Some("par-mux: close failed — test"),
        "still on screen long after an info toast would have faded"
    );
}

/// The unresponsive-daemon error persists, and the matching recovery
/// retires it instead of leaving a stale error beside "responding again".
#[test]
fn daemon_recovery_retires_the_unresponsive_error_toast() {
    use crate::app::overlay::toast::ToastKind;
    let mut ws = manners_state();
    ws.apply_mux_health_event(
        "par-mux daemon not responding — input queued until it recovers".to_string(),
    );
    assert_eq!(ws.tmux_state.mux_health, MuxHealth::Unresponsive);
    let toast = ws.overlay_state.toasts.newest().expect("error toast");
    assert_eq!(toast.kind, ToastKind::Error);

    ws.apply_mux_health_event("par-mux daemon responding again".to_string());
    assert_eq!(ws.tmux_state.mux_health, MuxHealth::Connected);
    let texts: Vec<&str> = ws
        .overlay_state
        .toasts
        .toasts()
        .iter()
        .map(|t| t.message.as_str())
        .collect();
    assert_eq!(
        texts,
        ["par-mux daemon responding again"],
        "the stale 'not responding' error is gone"
    );
}

/// par-mux errors share one persistent slot: a burst of failures shows the
/// latest (as the chip does) instead of filling the three-toast queue with
/// errors, which would make every later info toast — the tab-close Undo —
/// evict itself on arrival.
#[test]
fn par_mux_errors_share_one_slot_and_leave_room_for_undo() {
    use crate::app::overlay::toast::ToastKind;
    let mut ws = manners_state();
    ws.record_mux_error("par-mux: close failed — a");
    ws.record_mux_error("par-mux: swap failed — b");
    ws.record_mux_error("par-mux: launch failed — c");
    let errors: Vec<&str> = ws
        .overlay_state
        .toasts
        .toasts()
        .iter()
        .filter(|t| t.kind == ToastKind::Error)
        .map(|t| t.message.as_str())
        .collect();
    assert_eq!(errors, ["par-mux: launch failed — c"], "only the latest");
    ws.post_reopen_toast("Tab closed — undo within 5s", "Undo");
    assert!(
        ws.last_toast_text()
            .is_some_and(|t| t.starts_with("Tab closed — undo within 5s")),
        "the Undo toast is still on screen after a burst of par-mux errors: {:?}",
        ws.last_toast_text()
    );

    // Recovery retires the unresponsive error only, never an unrelated one
    // that replaced it in the slot.
    ws.apply_mux_health_event("par-mux daemon responding again".to_string());
    assert!(
        ws.overlay_state
            .toasts
            .toasts()
            .iter()
            .any(|t| t.message == "par-mux: launch failed — c"),
        "an unrelated par-mux error survives the daemon's recovery"
    );
}

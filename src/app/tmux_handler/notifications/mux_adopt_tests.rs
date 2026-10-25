//! adopt-pane app-side tests against a live in-process daemon: the pane
//! context menu's action path (`WindowState::adopt_pane_agent`) labels the
//! daemon pane, the roster picks the row up through the real push path,
//! and the daemon's refusal lands in the mux error toast slot.
//!
//! Screen paint follows the vault note
//! `Debugging/par-mux-headless-test-daemon-gotchas`: `send-keys -H` feeds
//! INPUT, so the title is painted by RUNNING `printf` in the pane's
//! shell, gated on a marker the shell ECHOES only after printf completes
//! (built by concatenation, so the echoed command line itself cannot
//! match).

use super::mux_test_seams::{DAEMON_DEADLINE, SettledSend, attached_window_with_tabs, wait_until};
use crate::app::window_state::WindowState;

/// Paint the claude idle OSC 2 title on daemon pane %0 and prove the paint
/// landed: the shell runs printf (title set) before its own `echo` output
/// (the marker) reaches the screen. `exec cat` then replaces the shell so
/// no prompt follows — a prompt repainting the title (Ubuntu bash's default
/// PS1 emits `\e]0;user@host:dir\a`) would wipe the OSC 2 paint before the
/// adopt classifies, which is exactly how the Linux CI legs failed.
fn paint_claude_idle_title(ws: &mut WindowState) {
    let transport = ws.tmux_state.transport.as_ref().expect("transport");
    let paint = "printf '\\033]2;✳ ready\\007'; echo P4-ADOPT-READ\"\"Y; exec cat";
    transport
        .send_settled(&format!("send-keys -t %0 \"{paint}\" Enter"))
        .expect("send-keys accepted");
    wait_until("the marker echoes", || {
        matches!(
            transport.query_settled("capture-pane -t %0 -p").as_deref(),
            Ok(body) if body.iter().any(|l| l.contains("P4-ADOPT-READY"))
        )
    });
}

/// The roster row for pane `%pane`, as `(agent, state)`.
fn roster_row(ws: &WindowState, pane: u64) -> Option<(String, String)> {
    ws.tmux_state
        .agent_roster
        .iter()
        .find(|e| e.pane == pane)
        .map(|e| (e.agent.clone(), e.state.clone()))
}

/// Criterion 2: the adopt the context-menu action sends labels the daemon
/// pane, and the classified `%agent-state-changed` push lands in the app's
/// roster cache through the real drain path.
#[test]
fn adopt_via_the_app_action_lands_in_the_roster() {
    let (mut ws, _path) = attached_window_with_tabs("p4-adopt", 1);
    paint_claude_idle_title(&mut ws);
    assert!(
        ws.adopt_pane_agent("claude"),
        "the daemon accepted the adopt"
    );
    let observed = {
        let deadline = std::time::Instant::now() + DAEMON_DEADLINE;
        loop {
            ws.check_tmux_notifications();
            if let Some(row) = roster_row(&ws, 0) {
                break Some(row);
            }
            if std::time::Instant::now() >= deadline {
                break None;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    };
    assert!(
        observed
            .as_ref()
            .is_some_and(|(agent, state)| agent == "claude" && state == "idle"),
        "the roster shows the adopted agent: observed {observed:?}, roster {:?}, \
         error slot {:?}, screen {:?}",
        ws.tmux_state
            .agent_roster
            .iter()
            .map(|e| (e.pane, e.agent.clone(), e.state.clone()))
            .collect::<Vec<_>>(),
        ws.tmux_state.mux_last_error,
        ws.tmux_state
            .transport
            .as_ref()
            .and_then(|t| t.query_settled("capture-pane -t %0 -p").ok()),
    );
    assert!(
        ws.tmux_state.mux_last_error.is_none(),
        "the error slot stayed clear: {:?}",
        ws.tmux_state.mux_last_error
    );
}

/// Criterion 3: the daemon's refusal for a kind without scrape patterns
/// reaches the user as the persistent mux error toast slot.
#[test]
fn adopt_refusal_for_a_kind_without_patterns_reaches_the_error_slot() {
    let (mut ws, _path) = attached_window_with_tabs("p4-adopt-refuse", 1);
    assert!(
        !ws.adopt_pane_agent("zeta-agent"),
        "the daemon refuses a kind without scrape patterns"
    );
    assert!(
        ws.tmux_state
            .mux_last_error
            .as_deref()
            .is_some_and(|m| m.contains("no scrape patterns")),
        "the refusal surfaced in the error slot: {:?}",
        ws.tmux_state.mux_last_error
    );
}

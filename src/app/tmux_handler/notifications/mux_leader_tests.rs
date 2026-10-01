//! The leader key (UX.md K4) in an attached par-mux tab, against a live
//! in-process daemon: leader `c`, `n`, `p`, `z`, and `x` take the same
//! entry point as in a local tab (`handle_leader_press`), and the
//! daemon's own window list, layout flags, and pane list prove each one
//! landed daemon-side. The local-tab and tmux-gateway halves are in
//! `app::leader::window_tests`.

use super::mux_test_seams::attached_window_with_tabs;
use crate::app::leader::{ArmedBy, LeaderPress, LeaderStep};
use crate::app::window_state::WindowState;
use std::time::{Duration, Instant};
use winit::keyboard::{Key, ModifiersState};

fn send(ws: &WindowState, cmd: &str) -> Vec<String> {
    ws.tmux_state
        .transport
        .as_ref()
        .expect("transport")
        .send_command(cmd)
        .unwrap_or_else(|e| panic!("{cmd}: {e}"))
}

/// Drain notifications (pumping a size so layouts arrive) until `done`.
fn pump(ws: &mut WindowState, what: &str, done: impl Fn(&WindowState) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done(ws) {
        assert!(Instant::now() < deadline, "{what}: timed out");
        if let Some(t) = ws.tmux_state.transport.as_ref() {
            let panes: Vec<u64> = ws.tmux_state.tmux_pane_owners.keys().copied().collect();
            for pane in panes {
                let _ = t.send_command_no_wait(&format!("refresh-client -t %{pane} -C 80x24"));
            }
        }
        ws.check_tmux_notifications();
        std::thread::sleep(Duration::from_millis(30));
    }
}

fn press(ws: &mut WindowState, key: Key, mods: ModifiersState) -> Option<LeaderStep> {
    ws.input_handler
        .update_modifiers(winit::event::Modifiers::from(mods));
    let step = ws.handle_leader_press(LeaderPress::pressed(&key));
    ws.input_handler
        .update_modifiers(winit::event::Modifiers::default());
    step
}

fn leader(ws: &mut WindowState) -> Option<LeaderStep> {
    let mods = if cfg!(target_os = "macos") {
        ModifiersState::SUPER
    } else {
        ModifiersState::CONTROL | ModifiersState::SHIFT
    };
    press(ws, Key::Character("b".into()), mods)
}

/// Leader then `c`: the step the leader took for the follow-up key.
fn leader_key(ws: &mut WindowState, c: &str) -> Option<LeaderStep> {
    assert_eq!(leader(ws), Some(LeaderStep::Arm(ArmedBy::Leader)));
    press(ws, Key::Character(c.into()), ModifiersState::empty())
}

fn daemon_windows(ws: &WindowState) -> usize {
    send(ws, "list-windows")
        .iter()
        .filter(|l| l.trim_start().starts_with('@'))
        .count()
}

fn daemon_panes(ws: &WindowState) -> usize {
    send(ws, "list-panes -a")
        .iter()
        .filter(|l| l.contains('%'))
        .count()
}

fn active_window(ws: &WindowState) -> Option<u64> {
    ws.tab_manager
        .active_tab_id()
        .and_then(|id| ws.mux_window_for_tab(id))
}

fn zoomed(ws: &WindowState) -> bool {
    ws.tab_manager
        .active_tab()
        .and_then(|t| t.pane_manager())
        .is_some_and(|pm| pm.zoomed_pane_id().is_some())
}

/// UX.md acceptance criteria 1 and 4, attached half: the leader table runs
/// daemon-side in an attached tab.
#[test]
fn leader_c_n_p_z_x_drive_an_attached_tab_daemon_side() {
    let (mut ws, path) = attached_window_with_tabs("leader-attached", 1);
    let start_windows = daemon_windows(&ws);
    let first = active_window(&ws).expect("an attached tab");

    // c: a new tab is a daemon window (new-window), mapped on arrival.
    assert_eq!(
        leader_key(&mut ws, "c"),
        Some(LeaderStep::Run {
            action: "new_tab",
            repeat: false
        })
    );
    pump(&mut ws, "leader c maps a second daemon window", |ws| {
        ws.tab_manager.tab_count() == 2 && ws.tmux_state.tmux_pane_owners.len() == 2
    });
    assert_eq!(daemon_windows(&ws), start_windows + 1, "daemon-side window");

    // n / p: tab switches select the daemon window too.
    ws.switch_to_tab_id(ws.tmux_state.tmux_pane_owner(0).expect("%0").0);
    assert_eq!(active_window(&ws), Some(first));
    leader_key(&mut ws, "n");
    let second = active_window(&ws).expect("second tab is attached");
    assert_ne!(second, first, "leader n: next tab");
    leader_key(&mut ws, "p");
    assert_eq!(active_window(&ws), Some(first), "leader p: previous tab");

    // z: split the tab daemon-side, then zoom it through the leader.
    assert!(ws.split_pane_via_mux(true), "daemon-side split");
    pump(&mut ws, "split mapped", |ws| {
        ws.tab_manager
            .active_tab()
            .and_then(|t| t.pane_manager())
            .is_some_and(|pm| pm.pane_count() == 2)
    });
    let panes_before = daemon_panes(&ws);
    leader_key(&mut ws, "z");
    pump(&mut ws, "leader z zooms daemon-side", zoomed);
    leader_key(&mut ws, "z");
    pump(&mut ws, "leader z unzooms daemon-side", |ws| !zoomed(ws));

    // x: close the focused pane — kill-pane on the daemon.
    leader_key(&mut ws, "x");
    pump(&mut ws, "leader x closes the pane", |ws| {
        ws.tab_manager
            .active_tab()
            .and_then(|t| t.pane_manager())
            .is_some_and(|pm| pm.pane_count() == 1)
    });
    assert_eq!(
        daemon_panes(&ws),
        panes_before - 1,
        "the daemon pane is gone"
    );
    let _ = std::fs::remove_file(&path);
}

/// UX.md criterion 3, attached: the leader pressed twice reaches the
/// daemon pane as typed input. With a Ctrl+A leader (an encodable chord on
/// every platform), the pane receives `^A` — proven by the daemon's own
/// capture of the pane running `cat -v`.
#[test]
fn leader_twice_reaches_the_attached_pane_as_typed_input() {
    let (mut ws, path) = attached_window_with_tabs("leader-literal", 1);
    let mut config = (**ws.config.load()).clone();
    config.input.leader_key = "Ctrl+A".to_string();
    ws.config.store(std::sync::Arc::new(config));
    let pane = *ws.tmux_state.tmux_pane_owners.keys().next().expect("pane");
    send(&ws, &format!("send-keys -t %{pane} 'exec cat -v' Enter"));
    std::thread::sleep(Duration::from_millis(300));

    let ctrl_a =
        |ws: &mut WindowState| press(ws, Key::Character("a".into()), ModifiersState::CONTROL);
    assert_eq!(ctrl_a(&mut ws), Some(LeaderStep::Arm(ArmedBy::Leader)));
    assert_eq!(ctrl_a(&mut ws), Some(LeaderStep::Literal));
    send(&ws, &format!("send-keys -t %{pane} Enter"));

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let screen = send(&ws, &format!("capture-pane -t %{pane} -p")).join("\n");
        if screen.contains("^A") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the literal ^A never reached the daemon pane:\n{screen}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = std::fs::remove_file(&path);
}

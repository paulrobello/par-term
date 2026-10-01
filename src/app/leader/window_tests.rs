//! The leader on a live `WindowState`, through the same entry point the
//! key handler and the `--ui-test` injector use
//! ([`WindowState::handle_leader_press`]). The attached par-mux half lives
//! in `tmux_handler::notifications::mux_leader_tests`.

use super::{ArmedBy, LeaderPress, LeaderStep, LeaderTiming};
use crate::app::overlay::OverlayId;
use crate::app::overlay::routing::{KeyFacts, KeyRoute};
use crate::app::window_state::WindowState;
use crate::config::Config;
use crate::pane::{Pane, PaneNode, SplitDirection};
use crate::tab::Tab;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::keyboard::{Key, ModifiersState, NamedKey};

fn runtime() -> Arc<tokio::runtime::Runtime> {
    Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime"),
    )
}

/// The default leader for this platform, as modifiers plus a key.
fn leader_mods() -> ModifiersState {
    if cfg!(target_os = "macos") {
        ModifiersState::SUPER
    } else {
        ModifiersState::CONTROL | ModifiersState::SHIFT
    }
}

fn set_mods(ws: &mut WindowState, mods: ModifiersState) {
    ws.input_handler
        .update_modifiers(winit::event::Modifiers::from(mods));
}

/// Press `key` with `mods` held, through the leader's entry point.
fn press_with(ws: &mut WindowState, key: Key, mods: ModifiersState) -> Option<LeaderStep> {
    set_mods(ws, mods);
    let step = ws.handle_leader_press(LeaderPress::pressed(&key));
    set_mods(ws, ModifiersState::empty());
    step
}

fn leader(ws: &mut WindowState) -> Option<LeaderStep> {
    press_with(ws, Key::Character("b".into()), leader_mods())
}

fn press(ws: &mut WindowState, c: &str) -> Option<LeaderStep> {
    press_with(ws, Key::Character(c.into()), ModifiersState::empty())
}

/// A window with `tabs` stub tabs (no PTY), the first one a 1 | 2 split.
fn window(tabs: usize) -> WindowState {
    let mut ws = WindowState::new(Config::default(), runtime());
    for i in 0..tabs {
        let mut tab = Tab::new_stub(i as u64 + 1, i + 1);
        if i == 0 {
            let pm = tab.pane_manager_mut().expect("pm");
            pm.set_root(PaneNode::split(
                SplitDirection::Vertical,
                0.5,
                PaneNode::leaf(stub_pane(1)),
                PaneNode::leaf(stub_pane(2)),
            ));
            pm.focus_pane(1);
            pm.set_bounds(crate::pane::PaneBounds::new(0.0, 0.0, 800.0, 400.0));
        }
        ws.tab_manager.push_tab_for_test(tab);
    }
    ws
}

fn stub_pane(id: crate::pane::PaneId) -> Pane {
    Pane::new_wrapping_terminal(
        id,
        Arc::new(tokio::sync::RwLock::new(
            par_term_terminal::TerminalManager::new_with_scrollback(20, 5, 0).expect("terminal"),
        )),
        None,
        Arc::new(std::sync::atomic::AtomicBool::new(false)),
    )
}

fn active_index(ws: &WindowState) -> usize {
    ws.tab_manager.active_tab_index().expect("active tab")
}

fn zoomed(ws: &WindowState) -> bool {
    ws.tab_manager
        .active_tab()
        .and_then(|t| t.pane_manager())
        .is_some_and(|pm| pm.is_zoomed())
}

#[test]
fn leader_n_p_z_drive_a_local_tab() {
    let mut ws = window(3);
    assert_eq!(active_index(&ws), 0);

    assert_eq!(leader(&mut ws), Some(LeaderStep::Arm(ArmedBy::Leader)));
    assert!(
        ws.overlay_is_open(OverlayId::Leader),
        "armed is a stack Mode"
    );
    assert!(
        !ws.any_modal_ui_visible(),
        "the leader never joins the modal guard"
    );
    press(&mut ws, "n");
    assert_eq!(active_index(&ws), 1, "leader n: next tab");
    assert!(!ws.leader.is_armed(), "a non-repeat key disarms");

    leader(&mut ws);
    press(&mut ws, "p");
    assert_eq!(active_index(&ws), 0, "leader p: previous tab");

    leader(&mut ws);
    press(&mut ws, "z");
    assert!(zoomed(&ws), "leader z: zoom the focused pane");
    leader(&mut ws);
    press(&mut ws, "z");
    assert!(!zoomed(&ws), "leader z again: unzoom");
}

#[test]
fn an_unarmed_letter_is_not_the_leaders() {
    let mut ws = window(1);
    assert_eq!(press(&mut ws, "n"), None, "n types n");
    assert!(!ws.leader.is_armed());
}

#[test]
fn escape_cancels_through_the_overlay_stack() {
    let mut ws = window(2);
    leader(&mut ws);
    // The real routing: the armed leader is the stack's top key owner, so
    // Escape is delegated to its handler, never closed by the stack.
    let route = ws.route_overlay_key(&KeyFacts {
        is_escape: true,
        ..KeyFacts::default()
    });
    assert_eq!(route, KeyRoute::ToOverlay(OverlayId::Leader));
    assert!(route.continues_to_key_dispatch());
    assert_eq!(
        press_with(
            &mut ws,
            Key::Named(NamedKey::Escape),
            ModifiersState::empty()
        ),
        Some(LeaderStep::Cancel)
    );
    assert!(!ws.leader.is_armed());
    assert_eq!(press(&mut ws, "n"), None, "after Escape, n types again");
    assert_eq!(active_index(&ws), 0);
}

#[test]
fn another_overlays_chord_does_not_replace_the_armed_leader() {
    let mut ws = window(1);
    leader(&mut ws);
    let route = ws.route_overlay_key(&KeyFacts {
        is_escape: false,
        bound_action: Some("toggle_command_palette".to_string()),
        is_command_chord: true,
    });
    assert_eq!(
        route,
        KeyRoute::ToOverlay(OverlayId::Leader),
        "a Mode owns its keys"
    );
}

#[test]
fn the_timeout_cancels_and_the_overlay_waits_for_its_delay() {
    let mut ws = window(2);
    {
        let mut config = (**ws.config.load()).clone();
        config.input.leader_timeout_ms = 2000;
        config.input.leader_overlay_delay_ms = 400;
        ws.config.store(Arc::new(config));
    }
    let t0 = Instant::now();
    let timing = LeaderTiming::from_config(&ws.config.load().input);
    ws.leader.arm(t0, ArmedBy::Leader, timing);

    assert_eq!(ws.tick_leader(t0), Some(t0 + Duration::from_millis(400)));
    assert!(ws.overlay_state.which_key.is_none(), "not before the delay");
    let wake = ws.tick_leader(t0 + Duration::from_millis(400));
    let which_key = ws.overlay_state.which_key.as_ref().expect("overlay shown");
    assert!(which_key.rows.iter().any(|r| r.key == "z"));
    assert_eq!(wake, Some(t0 + Duration::from_millis(2000)));

    assert_eq!(ws.tick_leader(t0 + Duration::from_millis(2000)), None);
    assert!(!ws.leader.is_armed(), "the timeout cancels");
    assert!(
        ws.overlay_state.which_key.is_none(),
        "and drops the overlay"
    );
}

#[test]
fn an_expired_arm_does_not_swallow_the_next_key() {
    let mut ws = window(2);
    let past = Instant::now() - Duration::from_secs(10);
    ws.leader.arm(
        past,
        ArmedBy::Leader,
        LeaderTiming::from_config(&ws.config.load().input),
    );
    assert_eq!(press(&mut ws, "n"), None, "a stale arm lets n through");
    assert_eq!(active_index(&ws), 0);
}

#[test]
fn the_which_key_lists_the_users_live_binding() {
    let mut ws = window(1);
    ws.keybinding_registry =
        par_term_keybindings::KeybindingRegistry::from_config(&[par_term_config::KeyBinding {
            key: "Ctrl+Alt+F9".to_string(),
            action: "toggle_pane_zoom".to_string(),
        }]);
    leader(&mut ws);
    let which_key = ws.build_which_key();
    let zoom = which_key.rows.iter().find(|r| r.key == "z").expect("z row");
    assert_eq!(zoom.chord.as_deref(), Some("Ctrl+Alt+F9"));
}

#[test]
fn a_repeat_key_stays_armed() {
    let mut ws = window(1);
    leader(&mut ws);
    assert_eq!(
        press(&mut ws, "o"),
        Some(LeaderStep::Run {
            action: "next_pane",
            repeat: true
        })
    );
    assert!(ws.leader.is_armed(), "o is a repeat key");
    assert_eq!(
        ws.tab_manager
            .active_tab()
            .and_then(|t| t.focused_pane_id()),
        Some(2)
    );
    press(&mut ws, "o");
    assert_eq!(
        ws.tab_manager
            .active_tab()
            .and_then(|t| t.focused_pane_id()),
        Some(1),
        "still armed: o again cycles back"
    );
    press(&mut ws, "z");
    assert!(!ws.leader.is_armed(), "a non-repeat key ends it");
}

#[test]
fn an_empty_leader_key_turns_the_leader_off() {
    let mut ws = window(1);
    let mut config = (**ws.config.load()).clone();
    config.input.leader_key = String::new();
    ws.config.store(Arc::new(config));
    assert_eq!(leader(&mut ws), None);
    assert!(!ws.leader.is_armed());
}

/// A tab whose shell is `cat`: what reaches its PTY echoes into the grid.
#[cfg(unix)]
fn cat_window() -> WindowState {
    let config = Config {
        shell: crate::config::ShellConfig {
            custom_shell: Some("cat".into()),
            login_shell: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let tab_config = config.clone();
    let rt = runtime();
    let mut ws = WindowState::new(config, Arc::clone(&rt));
    ws.tab_manager
        .new_tab(&tab_config, rt, false, Some((80, 24)))
        .expect("cat tab");
    ws
}

#[cfg(unix)]
fn wait_for_screen(ws: &WindowState, what: &str, done: impl Fn(&str) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        ws.runtime.block_on(async {
            tokio::time::sleep(Duration::from_millis(50)).await;
        });
        let tab = ws.tab_manager.active_tab().expect("tab");
        let screen = tab
            .terminal
            .try_read()
            .map(|t| t.export_text())
            .unwrap_or_default();
        if done(&screen) {
            return;
        }
        assert!(Instant::now() < deadline, "{what}; screen:\n{screen}");
    }
}

/// K8: the leader pressed twice sends the chord's own bytes to the pane.
/// Off macOS the leader is Ctrl+Shift+B, which encodes as ^B (0x02);
/// `cat` echoes it as `^B`. On macOS Cmd+B has no terminal encoding, so
/// the second press only disarms.
#[cfg(unix)]
#[test]
fn leader_twice_sends_the_literal_chord_to_the_pane() {
    let mut ws = cat_window();
    assert_eq!(leader(&mut ws), Some(LeaderStep::Arm(ArmedBy::Leader)));
    assert_eq!(leader(&mut ws), Some(LeaderStep::Literal));
    assert!(!ws.leader.is_armed());
    if cfg!(target_os = "macos") {
        return;
    }
    wait_for_screen(&ws, "the literal ^B never reached the pane", |s| {
        s.contains("^B")
    });
}

/// The same double-tap with a leader that does encode, on every platform:
/// Ctrl+A arms, Ctrl+A again sends 0x01 (`^A` in the echo), and the next
/// key after it is ordinary typing.
#[cfg(unix)]
#[test]
fn a_ctrl_leader_twice_reaches_the_pty_as_its_control_byte() {
    let mut ws = cat_window();
    let mut config = (**ws.config.load()).clone();
    config.input.leader_key = "Ctrl+A".to_string();
    ws.config.store(Arc::new(config));
    let ctrl_a =
        |ws: &mut WindowState| press_with(ws, Key::Character("a".into()), ModifiersState::CONTROL);
    assert_eq!(ctrl_a(&mut ws), Some(LeaderStep::Arm(ArmedBy::Leader)));
    assert_eq!(ctrl_a(&mut ws), Some(LeaderStep::Literal));
    wait_for_screen(&ws, "the literal ^A never reached the pane", |s| {
        s.contains("^A")
    });
}

/// UX.md K4: with a tmux gateway connected, the tmux prefix arms the same
/// leader, and a key tmux's table owns is sent to tmux as its command. The
/// gateway here is a `cat` tab, so what the leader wrote to the gateway
/// echoes back into its grid.
#[cfg(unix)]
#[test]
fn the_tmux_prefix_is_a_leader_alias_in_a_gateway_tab() {
    let mut ws = cat_window();
    let mut config = (**ws.config.load()).clone();
    config.tmux.tmux_enabled = true;
    ws.config.store(Arc::new(config));
    let gateway = ws.tab_manager.active_tab_id().expect("tab");
    ws.tab_manager
        .active_tab_mut()
        .expect("tab")
        .tmux
        .tmux_gateway_active = true;
    ws.tmux_state.tmux_gateway_tab_id = Some(gateway);
    let mut session = crate::tmux::TmuxSession::new();
    session.set_gateway_initiating();
    session.set_gateway_connected("leader-test".to_string());
    ws.tmux_state.tmux_session = Some(session);
    assert!(ws.leader_tmux_tab());

    // Ctrl+B is tmux's prefix (`tmux_prefix_key: C-b`).
    let prefix =
        |ws: &mut WindowState| press_with(ws, Key::Character("b".into()), ModifiersState::CONTROL);
    assert_eq!(prefix(&mut ws), Some(LeaderStep::Arm(ArmedBy::TmuxPrefix)));
    assert!(ws.overlay_is_open(OverlayId::Leader));
    assert!(matches!(press(&mut ws, "c"), Some(LeaderStep::Tmux { .. })));
    wait_for_screen(&ws, "leader c never reached tmux", |s| {
        s.contains("new-window")
    });

    // The par-term leader arms the same machine there too.
    leader(&mut ws);
    press(&mut ws, "z");
    wait_for_screen(&ws, "leader z never reached tmux", |s| {
        s.contains("resize-pane -Z")
    });
    // par-term UI keys stay par-term's in a gateway tab.
    leader(&mut ws);
    press(&mut ws, "?");
    assert!(
        ws.overlay_ui.help_ui.visible,
        "leader ? opens par-term help"
    );
    ws.overlay_ui.help_ui.visible = false;

    // A local tab beside the gateway keeps par-term's actions: leader x
    // there closes the local pane, never a tmux pane in another tab, and
    // the tmux prefix is the shell's.
    ws.tab_manager.push_tab_for_test(Tab::new_stub(99, 2));
    ws.tab_manager.switch_to(99);
    assert!(!ws.leader_tmux_tab(), "a local tab is not a tmux tab");
    assert_eq!(prefix(&mut ws), None, "Ctrl+B reaches the local shell");
    assert_eq!(leader(&mut ws), Some(LeaderStep::Arm(ArmedBy::Leader)));
    assert_eq!(
        press(&mut ws, "x"),
        Some(LeaderStep::Run {
            action: "close_pane",
            repeat: false
        })
    );
}

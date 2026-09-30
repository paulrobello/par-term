//! PTY tests for the program a pane runs: Restart Pane reruns it (UX.md A9)
//! and a split of a profile tab inherits it (D5). They spawn real
//! processes, so they are `#[ignore]` like the other PTY tests; run with
//! `cargo test -p par-term --lib -- --include-ignored pane_launch_tests`.

use crate::app::window_state::WindowState;
use crate::config::Config;
use crate::pane::{LaunchCommand, SplitDirection};
use crate::profile::Profile;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn runtime() -> Arc<tokio::runtime::Runtime> {
    Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("runtime"),
    )
}

/// A profile whose program prints `marker` and then waits, so the pane's
/// screen shows which program is running.
fn marker_profile(marker: &str) -> Profile {
    let mut profile = Profile::new("marker");
    profile.command = Some("/bin/sh".to_string());
    profile.command_args = Some(vec!["-c".to_string(), format!("echo {marker}; exec cat")]);
    profile
}

fn window_with_profile_tab(config: &Config, profile: &Profile) -> WindowState {
    let rt = runtime();
    let mut ws = WindowState::new(config.clone(), Arc::clone(&rt));
    let id = ws
        .tab_manager
        .new_tab_from_profile(config, rt, profile, Some((80, 24)))
        .expect("profile tab");
    ws.tab_manager.switch_to(id);
    ws
}

fn pane_text(ws: &WindowState, pane: crate::pane::PaneId) -> String {
    ws.tab_manager
        .active_tab()
        .and_then(|t| t.pane_manager())
        .and_then(|pm| pm.get_pane(pane))
        .and_then(|p| p.terminal.try_read().ok().map(|t| t.export_text()))
        .unwrap_or_default()
}

fn wait_for(ws: &WindowState, pane: crate::pane::PaneId, needle: &str, times: usize) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let text = pane_text(ws, pane);
        if text.matches(needle).count() >= times {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "pane {pane} never showed {needle:?} x{times}: {text:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn focused(ws: &WindowState) -> crate::pane::PaneId {
    ws.tab_manager
        .active_tab()
        .and_then(|t| t.focused_pane_id())
        .expect("focused pane")
}

/// A9: restarting a profile tab's pane reruns the profile's program, not
/// the configured shell. The restart clears the screen, so the marker
/// showing again proves the program ran again.
#[test]
#[ignore = "requires PTY spawn"]
fn restart_pane_reruns_the_profile_program() {
    let config = Config::default();
    let mut ws = window_with_profile_tab(&config, &marker_profile("restart-marker"));
    let pane = focused(&ws);
    wait_for(&ws, pane, "restart-marker", 1);
    assert_eq!(
        ws.tab_manager
            .active_tab()
            .and_then(|t| t.pane_manager())
            .and_then(|pm| pm.get_pane(pane))
            .and_then(|p| p.launch.clone())
            .map(|l| l.program),
        Some("/bin/sh".to_string()),
        "the first pane records the profile program"
    );

    // Blank the screen so only a rerun can put the marker back.
    if let Some(term) = ws
        .tab_manager
        .active_tab()
        .and_then(|t| t.pane_manager())
        .and_then(|pm| pm.get_pane(pane))
        .and_then(|p| p.terminal.try_read().ok())
    {
        term.process_data(b"\x1b[2J\x1b[H");
    }
    assert!(!pane_text(&ws, pane).contains("restart-marker"));

    ws.restart_focused_pane();
    wait_for(&ws, pane, "restart-marker", 1);
}

/// D5: with `split_inherits_profile` on (the default) a split of a profile
/// tab runs the profile's program; off, it runs the configured shell.
#[test]
#[ignore = "requires PTY spawn"]
fn a_split_inherits_the_profile_program_unless_disabled() {
    for inherit in [true, false] {
        let mut config = Config::default();
        config.panes.split_inherits_profile = inherit;
        let mut ws = window_with_profile_tab(&config, &marker_profile("split-marker"));
        let first = focused(&ws);
        wait_for(&ws, first, "split-marker", 1);

        let new_pane = ws
            .split_pane_direction(SplitDirection::Vertical, true, None, 50)
            .expect("split");
        let launch = ws
            .tab_manager
            .active_tab()
            .and_then(|t| t.pane_manager())
            .and_then(|pm| pm.get_pane(new_pane))
            .and_then(|p| p.launch.clone());
        if inherit {
            assert_eq!(launch.map(|l| l.program), Some("/bin/sh".to_string()));
            wait_for(&ws, new_pane, "split-marker", 1);
        } else {
            assert_eq!(launch, None, "the configured shell, resolved at restart");
            std::thread::sleep(Duration::from_millis(500));
            assert!(!pane_text(&ws, new_pane).contains("split-marker"));
        }
    }
}

/// A split's explicit command (a trigger or snippet) wins over the
/// inherited profile program.
#[test]
#[ignore = "requires PTY spawn"]
fn an_explicit_split_command_wins_over_the_profile() {
    let config = Config::default();
    let mut ws = window_with_profile_tab(&config, &marker_profile("profile-marker"));
    let new_pane = ws
        .split_pane_direction(
            SplitDirection::Horizontal,
            true,
            Some((
                "/bin/sh".to_string(),
                vec![
                    "-c".to_string(),
                    "echo explicit-marker; exec cat".to_string(),
                ],
            )),
            50,
        )
        .expect("split");
    wait_for(&ws, new_pane, "explicit-marker", 1);
    let launch = ws
        .tab_manager
        .active_tab()
        .and_then(|t| t.pane_manager())
        .and_then(|pm| pm.get_pane(new_pane))
        .and_then(|p| p.launch.clone());
    assert_eq!(
        launch,
        Some(LaunchCommand::new(
            "/bin/sh".to_string(),
            vec![
                "-c".to_string(),
                "echo explicit-marker; exec cat".to_string()
            ]
        ))
    );
}

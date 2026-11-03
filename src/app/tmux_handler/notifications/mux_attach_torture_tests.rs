//! Attach-torture regression for card 01a198054b5d (the stale-pixel card):
//! every fresh attach carries buffered pre-seed `%output` — bytes the daemon
//! streamed while no par-term client was attached, whose effect the attach
//! seed's clear+screen already contains. Seed, backlog replay, and live
//! routing are three writers racing on one grid; when they interleave
//! wrongly the pane buffer scrambles with the exact signature Paul saw
//! (substituted glyphs, threaded box-drawing dashes, missing letters).
//!
//! The seed-path fixes pin here under REAL daemon timing, which the
//! deterministic ordering tests in `mux_pane_moves.rs` cannot sample:
//! `attach_until_installed` drives the production install (pump arming
//! included), and each cycle writes cursor-addressed repaint bytes while
//! detached, so seed, backlog replay, and live output are all in flight
//! together at attach time. Each cycle then asserts the mirror's screen
//! ends byte-equal to the daemon's grid — the scramble cannot hide.
//!
//! Daemon-timing heavy and iteration-scaled, so this is `#[ignore]`-gated —
//! run it explicitly, never per-gate:
//!
//! ```sh
//! cargo test -p par-term --lib mux_attach_torture -- --ignored --nocapture
//! ```
//!
//! `PAR_TERM_ATTACH_TORTURE_CYCLES` scales the cycle count (default 10).

use super::mux::MuxAttachPending;
use super::mux::tests::{manners_state, socket_path, spawn_daemon};
use super::mux_test_seams::{DAEMON_DEADLINE, SettledSend, attach_until_installed};
use crate::app::window_state::WindowState;
use par_term_emu_core_rust::mux::MuxClient;
use par_term_mux::MuxSessionClient;
use std::time::{Duration, Instant};

/// The per-cycle TUI emitter. Paints an alt-screen frame embedding a
/// cycle-distinct marker, then stages cursor-addressed mid-row overwrites
/// spaced 150ms apart (~1.2s, spanning the attach), then holds the grid
/// (no further writes) so the final state is deterministic on both sides
/// of the comparison. The cycle number arrives as argv[1].
const EMITTER_PY: &str = r##"import sys, time
out = sys.stdout
out.write("\x1b[?1049h\x1b[2J\x1b[H")
out.write("\x1b[41m")
marker = "CYCLE%02d" % int(sys.argv[1])
words = ["ALPHA", "BRAVO", "CHARLIE", "DELTA", "ECHO", "FOXTROT", "GOLF", "HOTEL"]
for r in range(24):
    if r in (0, 23):
        line = "#" * 80
    elif r == 2:
        line = "# " + marker + " " + "o" * (80 - 4 - len(marker)) + "#"
    else:
        line = "#" + "o" * 78 + "#"
    out.write("\x1b[%d;1H%s" % (r + 1, line))
out.write("\x1b[0m")
for i, word in enumerate(words):
    time.sleep(0.15)
    out.write("\x1b[12;%dH%s" % (12 + i * 7, word))
    out.flush()
out.write("\x1b[?25l")
out.flush()
time.sleep(300)
"##;

/// Trailing whitespace is grid padding and trailing empty lines are unused
/// rows; everything else must match byte-for-byte.
fn normalized(s: &str) -> Vec<&str> {
    let mut rows: Vec<&str> = s.lines().map(|l| l.trim_end()).collect();
    while matches!(rows.last(), Some(&"")) {
        rows.pop();
    }
    rows
}

/// The mirror pane showing `%0`, as `(alt_screen_active, screen_text)`.
fn mirror_screen(ws: &WindowState) -> Option<(bool, String)> {
    let (tab_id, native) = ws.tmux_state.tmux_pane_owner(0)?;
    let tab = ws.tab_manager.get_tab(tab_id)?;
    let pane = tab.pane_manager()?.get_pane(native)?;
    let term = pane.terminal.try_read().ok()?;
    Some((
        term.is_alt_screen_active(),
        term.content().unwrap_or_default(),
    ))
}

/// One attach cycle: session + emitter (whose writes stage while detached),
/// detach, then a fresh [`WindowState`] attaching through the production
/// path, and the byte-exact screen comparison. Returns only via assert.
fn run_cycle(cycle: usize) {
    // One daemon per cycle: daemon pane ids are daemon-global, so with a
    // shared daemon cycle N's session pane is not `%0` and every `-t %0`
    // probe (and keystroke) would hit cycle 1's sleeping pane. The tortured
    // variable is the attach race, not pane-id allocation.
    let path = socket_path(&format!("attach-torture-{cycle}"));
    spawn_daemon(&path);
    let session = format!("torture{cycle}");
    let script_path = std::env::temp_dir().join(format!(
        "par-term-torture-{}-{cycle}.py",
        std::process::id()
    ));
    std::fs::write(&script_path, EMITTER_PY).expect("write emitter script");

    // Session + emitter: all of its writes land BEFORE any par-term client
    // attaches — they are exactly the buffered pre-seed output the fixes
    // guard, and their effect is baked into the attach seed.
    let marker = format!("CYCLE{cycle:02}");
    {
        let mut first = MuxSessionClient::connect(&path).expect("setup client");
        first.create_or_attach(&session).expect("create session");
        std::thread::sleep(Duration::from_millis(300));
        first
            .send_keys_literal(0, &format!("python3 {} {cycle}", script_path.display()))
            .expect("send emitter command");
        first.send_keys(0, b"\r").expect("send enter");

        // The daemon grid must show the FINAL overwrite before we attach:
        // the emitter has finished writing; its bytes are the backlog.
        let mut grid;
        let deadline = Instant::now() + DAEMON_DEADLINE;
        loop {
            grid = first
                .send("capture-pane -t %0 -p")
                .expect("capture-pane")
                .join("\n");
            if grid.contains(&marker) && grid.contains("HOTEL") {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the emitter never finished painting: {grid:?}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    } // setup client drops — detach; the emitter holds its grid daemon-side

    // The relaunch: a fresh app state attaching through the production path
    // (`poll_mux_attach` → `install_mux_transport` → `attach_sequence`,
    // pump arming included — the exact machinery the fixes live in).
    let mut ws = manners_state();
    attach_until_installed(
        &mut ws,
        || {
            let core_client = MuxClient::connect(&path).expect("attach client");
            let (tx, rx) = std::sync::mpsc::channel();
            tx.send(Ok(core_client)).unwrap();
            drop(tx);
            MuxAttachPending {
                name: session.clone(),
                rx,
            }
        },
        &path,
    );

    // Pump until the mirror's screen converges byte-exact to the daemon's
    // grid: the seed must land, the buffered replay must be dropped/folded
    // by the pump machinery, and live output must continue on top. The
    // refresh pump forces layout adoption each tick, as production does.
    let deadline = Instant::now() + DAEMON_DEADLINE;
    let mut grid = String::new();
    let mut screen = String::new();
    let mut alt = false;
    let matched = loop {
        ws.check_mux_notifications();
        if let Some(t) = ws.tmux_state.transport.as_ref() {
            let _ = t.send_command_no_wait("refresh-client -t %0 -C 80x24");
            if let Ok(rows) = t.query_settled("capture-pane -t %0 -p") {
                grid = rows.join("\n");
            }
        }
        if let Some((a, text)) = mirror_screen(&ws) {
            alt = a;
            screen = text;
            if normalized(&grid) == normalized(&screen) {
                break true;
            }
        }
        if Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(
        matched && alt && screen.contains(&marker) && grid.contains("HOTEL"),
        "cycle {cycle}: the mirror screen never converged byte-exact to the daemon grid\n\
         marker present: {}\n\
         daemon grid:\n{grid}\n\
         mirror screen (alt={alt}):\n{screen}",
        screen.contains(&marker)
    );

    let _ = ws.detach_mux_session();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&path);
}

/// The torture: N relaunch cycles, each its own daemon so `%0` addressing
/// stays exact (daemon pane ids are daemon-global — see `run_cycle`).
#[test]
#[ignore = "daemon-timing torture; run explicitly on a low-load window"]
fn attach_torture_keeps_the_mirror_screen_byte_exact_across_relaunches() {
    let cycles = std::env::var("PAR_TERM_ATTACH_TORTURE_CYCLES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    for cycle in 1..=cycles {
        run_cycle(cycle);
    }
}

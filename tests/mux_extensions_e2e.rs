//! End-to-end proof that the INSTALLED par-mux extension drives a live
//! daemon (card criterion 5): the real bundled asset is installed into a
//! temp agent directory by [`par_term::mux_extension_installer`], a bun
//! driver loads THAT FILE and fires the agent's lifecycle events, and the
//! accepted report comes back from the daemon as an `%agent-state-changed`
//! broadcast — proving asset and hook endpoint agree on the wire. The wait
//! requires TWO broadcasts: the state report's own push, then the
//! REBROADCAST only an accepted session report produces — the assets send
//! theirs path-only with a `session_resume_argv`, the shape the
//! id-or-path contract exists for. The driver then fires
//! `session_shutdown(quit)`, and the pane must LEAVE the `list-agents`
//! roster on an `%agent-released` broadcast — the sender-side leg of the
//! pane.release_agent protocol (core aed41c2): without it the roster shows
//! a dead agent working until the pane dies.
//!
//! Gated on `mux`, which is on by default. Run directly:
//!
//! ```sh
//! cargo test -p par-term --test mux_extensions_e2e -- --nocapture
//! ```
//!
//! Skips (passing, with a note) where bun is absent — the asset is a
//! TypeScript pi extension and cannot execute without a JS runtime.

#![cfg(feature = "mux")]

use par_term::mux_extension_installer;
use par_term_emu_core_rust::mux::MuxServer;
use par_term_mux::MuxSessionClient;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// A minimal fake pi runtime: records `pi.on(...)` handlers so the test can
/// fire the lifecycle events the real host would fire. `events.on`
/// subscriptions (the herdr:blocked listener) are accepted and ignored.
///
/// The session_start/agent_start pair fires `BURSTS` times: the asset's
/// sends must serialize (the daemon orders reports by ARRIVAL against the
/// per-source monotonic seq), and the repeat turns a one-in-N interleaving
/// race into a deterministic regression — a serialized asset emits an
/// exact broadcast count, any overtake drops below it.
const DRIVER: &str = r#"
const target = process.argv[2];
const mod = await import(target);
const handlers = {};
const pi = {
  events: { on: (_name, _cb) => {} },
  on: (name, cb) => { (handlers[name] ??= []).push(cb); },
};
mod.default(pi);
const ctx = {
  mode: "tui",
  hasUI: true,
  isIdle: () => false,
  sessionManager: {
    getSessionFile: () => "/tmp/par-mux-e2e-session.json",
    getSessionId: () => "e2e-session-1",
  },
};
const bursts = Number(process.argv[3] ?? 1);
for (let i = 0; i < bursts; i++) {
  for (const cb of handlers["session_start"] ?? []) await cb({ reason: "startup" }, ctx);
  for (const cb of handlers["agent_start"] ?? []) await cb({}, ctx);
}
await new Promise((resolve) => setTimeout(resolve, 1500));
// A quit must release the pane's claim (pane.release_agent): the shutdown
// handler awaits the send, so the release is on the wire before exit.
for (const cb of handlers["session_shutdown"] ?? []) await cb({ type: "session_shutdown", reason: "quit" });
await new Promise((resolve) => setTimeout(resolve, 1500));
"#;

/// Burst count for the interleaving regression (see `DRIVER`): 25 rounds of
/// the racing pair. Serialized, each burst yields a fixed, per-asset
/// broadcast count; a send that overtakes its predecessor lands
/// stale-dropped and the count falls short.
const BURSTS: usize = 25;
/// pi fires a session report on BOTH session_start and agent_start: burst 1
/// gives two broadcasts (its first session report precedes any state),
/// later bursts three (the session_start report rebroadcasts the prior
/// burst's surviving state).
const PI_EXPECTED: usize = 3 * BURSTS - 1;
/// omp reports a session only on agent_start: exactly two per burst.
const OMP_EXPECTED: usize = 2 * BURSTS;

fn bun_available() -> bool {
    match Command::new("bun").arg("--version").output() {
        Ok(output) => output.status.success(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(err) => panic!("could not probe bun: {err}"),
    }
}

fn socket_path(tag: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "par-term-mux-ext-e2e-{}-{tag}.sock",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

fn spawn_daemon(path: &Path) {
    let server = MuxServer::bind(path).expect("daemon binds");
    // run() serves until the process ends — the core's own e2e lifecycle.
    std::thread::spawn(move || server.run());
}

fn connect(path: &Path) -> MuxSessionClient {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match MuxSessionClient::connect(path) {
            Ok(client) => return client,
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => panic!("daemon never accepted a connection: {e}"),
        }
    }
}

/// Install the real asset, drive it with bun against a live daemon, and
/// assert the report lands as an accepted broadcast.
fn installed_extension_drives_the_daemon(
    agent: &str,
    expected: usize,
    install: impl FnOnce(&Path) -> std::io::Result<PathBuf>,
) {
    if !bun_available() {
        eprintln!("skipping {agent} e2e: bun not found on PATH");
        return;
    }

    let root = tempfile::tempdir().expect("temp dir");
    let installed = install(root.path()).expect("extension installs");

    let socket = socket_path(agent);
    spawn_daemon(&socket);
    let mut client = connect(&socket);
    let outcome = client
        .create_or_attach(&format!("ext-e2e-{agent}"))
        .expect("create_or_attach");
    assert!(
        matches!(outcome, par_term_mux::AttachOutcome::Created(_)),
        "fresh daemon has no session to attach to: {outcome:?}"
    );
    assert!(
        !client.list_panes().expect("list-panes").is_empty(),
        "a fresh session has a pane to report against"
    );

    // A fresh session's first pane is %0 — the id the daemon would seed as
    // PAR_MUX_PANE_ID for a pane process (core pane.rs seeds id.to_string()).
    let driver = root.path().join("driver.mjs");
    std::fs::write(&driver, DRIVER).expect("write driver");
    let run = Command::new("bun")
        .arg(&driver)
        .arg(&installed)
        .arg(BURSTS.to_string())
        .env("PAR_MUX_ENV", "1")
        .env("PAR_MUX_SOCKET", &socket)
        .env("PAR_MUX_PANE_ID", "%0")
        .env_remove("OMPCODE")
        .current_dir(root.path())
        .output()
        .expect("bun runs the installed extension");
    assert!(
        run.status.success(),
        "driver failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );

    // 20s, not 10s: under full-`cargo test --workspace` load the daemon
    // spawn + bun driver + reports can outlive a 10s budget outright (saw 0
    // broadcasts in 10s, 5/5 green in isolation — the same flake class the
    // agent-usage watcher tolerance was widened for at f36db8a9).
    let deadline = Instant::now() + Duration::from_secs(20);
    // The acceptance proof, exactly countable because the asset serializes
    // its sends: the state report's own push, plus the REBROADCAST only an
    // ACCEPTED session report produces, per the per-asset `expected`. A
    // send that overtakes its predecessor arrives with a LOWER seq than
    // the pane already recorded and is stale-dropped silently (the flake
    // this count exists to catch); an error-replied session report sends
    // nothing either. Either way the count falls short.
    let mut broadcasts = 0usize;
    let mut released = false;
    while Instant::now() < deadline {
        let (notes, _) = client.drain_core_notifications();
        broadcasts += notes
            .iter()
            .filter(|note| {
                matches!(
                    note,
                    par_term_emu_core_rust::tmux_control::TmuxNotification::AgentStateChanged {
                        pane_id,
                        agent: reported_agent,
                        state,
                        source,
                    } if pane_id == "%0"
                        && reported_agent == agent
                        && state == "working"
                        && source == "hook"
                )
            })
            .count();
        released |= notes.iter().any(|note| {
            matches!(
                note,
                par_term_emu_core_rust::tmux_control::TmuxNotification::AgentReleased {
                    pane_id,
                    agent: released_agent,
                } if pane_id == "%0" && released_agent == agent
            )
        });
        if broadcasts >= expected && released {
            // A2b task 1 live leg: the report the broadcast announced must
            // also read back through the roster query, with the hook
            // provenance the endpoint records — and after the shutdown
            // release, the pane must be GONE from the roster (the stale-
            // claim repro the release protocol exists to clear).
            let roster = client.list_agents().expect("list-agents");
            assert!(
                roster.iter().all(|entry| entry.pane != 0),
                "pane %0 must leave the roster after session_shutdown(quit): {roster:?}"
            );
            assert_eq!(
                broadcasts, expected,
                "serialized sends broadcast exactly once per accepted report"
            );
            let _ = std::fs::remove_file(&socket);
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!(
        "{agent} extension: expected {expected} broadcasts (state pushes + \
         accepted-session rebroadcasts) AND the quit release, saw {broadcasts} \
         broadcasts and released={released} in 20s"
    );
}

#[test]
fn pi_extension_drives_a_live_daemon() {
    installed_extension_drives_the_daemon("pi", PI_EXPECTED, |root| {
        let pi_home = root.join("pi-home");
        std::fs::create_dir_all(&pi_home).expect("agent home");
        mux_extension_installer::install_pi_extension_into(&pi_home.join("extensions"))
    });
}

#[test]
fn omp_extension_drives_a_live_daemon() {
    installed_extension_drives_the_daemon("omp", OMP_EXPECTED, |root| {
        let omp_agent_root = root.join("omp-home").join("agent");
        std::fs::create_dir_all(&omp_agent_root).expect("agent home");
        mux_extension_installer::install_omp_extension_into(
            &root.join("pi-home"),
            &omp_agent_root.join("extensions"),
        )
    });
}

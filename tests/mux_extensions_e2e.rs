//! End-to-end proof that the INSTALLED par-mux extension drives a live
//! daemon (card criterion 5): the real bundled asset is installed into a
//! temp agent directory by [`par_term::mux_extension_installer`], a bun
//! driver loads THAT FILE and fires the agent's lifecycle events, and the
//! accepted report comes back from the daemon as an `%agent-state-changed`
//! broadcast — proving asset and hook endpoint agree on the wire.
//!
//! Gated on `mux` (compiled empty without it) and run through
//! `make with-local-core` per the vendored-core standing policy:
//!
//! ```sh
//! scripts/with-local-core.sh \
//!   cargo test -p par-term --features mux --test mux_extensions_e2e -- --nocapture
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
for (const cb of handlers["session_start"] ?? []) await cb({ reason: "startup" }, ctx);
for (const cb of handlers["agent_start"] ?? []) await cb({}, ctx);
await new Promise((resolve) => setTimeout(resolve, 1500));
"#;

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

    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let (notes, _) = client.drain_core_notifications();
        let hit = notes.iter().any(|note| {
            matches!(
                note,
                par_term_emu_core_rust::tmux_control::TmuxNotification::AgentStateChanged {
                    pane_id,
                    agent: reported_agent,
                    state,
                } if pane_id == "%0" && reported_agent == agent && state == "working"
            )
        });
        if hit {
            let _ = std::fs::remove_file(&socket);
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("{agent} extension report never arrived as an accepted broadcast");
}

#[test]
fn pi_extension_drives_a_live_daemon() {
    installed_extension_drives_the_daemon("pi", |root| {
        let pi_home = root.join("pi-home");
        std::fs::create_dir_all(&pi_home).expect("agent home");
        mux_extension_installer::install_pi_extension_into(&pi_home.join("extensions"))
    });
}

#[test]
fn omp_extension_drives_a_live_daemon() {
    installed_extension_drives_the_daemon("omp", |root| {
        let omp_agent_root = root.join("omp-home").join("agent");
        std::fs::create_dir_all(&omp_agent_root).expect("agent home");
        mux_extension_installer::install_omp_extension_into(
            &root.join("pi-home"),
            &omp_agent_root.join("extensions"),
        )
    });
}

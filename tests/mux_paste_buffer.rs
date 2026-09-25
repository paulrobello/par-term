//! Byte-exact round-trip proof for the clipboard sync's `set-buffer`
//! command (card 01a0d932: "Mux: sync par-term copies into the par-mux
//! daemon paste buffer"). The production formatting lives in
//! [`par_term_tmux::set_buffer_command`]; this drives THAT function's
//! output through a live in-process daemon (`MuxServer::bind` on a temp
//! socket — the same lifecycle the extension e2e uses) so the quoting is
//! validated against the real `shell_split` parser, not a mock of it.
//! `show-buffer` must return the copied text unchanged: multi-line
//! content, embedded single and double quotes, backslashes, tabs.
//!
//! Gated on `mux`, which is on by default. Run directly:
//!
//! ```sh
//! cargo test -p par-term --test mux_paste_buffer -- --nocapture
//! ```

#![cfg(feature = "mux")]

use par_term_emu_core_rust::mux::MuxServer;
use par_term_mux::MuxSessionClient;
use par_term_tmux::set_buffer_command;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn socket_path() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "par-term-mux-paste-buffer-{}.sock",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

fn spawn_daemon(path: &std::path::Path) {
    let server = MuxServer::bind(path).expect("daemon binds");
    // run() serves until the process ends — the core's own e2e lifecycle.
    std::thread::spawn(move || server.run());
}

fn connect(path: &std::path::Path) -> MuxSessionClient {
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

#[test]
fn set_buffer_round_trips_byte_exact_through_the_daemon() {
    let socket = socket_path();
    spawn_daemon(&socket);
    let mut client = connect(&socket);
    client
        .create_or_attach("paste-buffer-e2e")
        .expect("create_or_attach");

    let samples = [
        "single line",
        "line one\nline two\nline three",
        "it's got \"double quotes\" and 'singles'",
        "mixed 'quote\nnewline' plus \\ backslash",
        "trailing spaces   and\ttabs\t",
    ];
    for sample in samples {
        let set_replies = client
            .send(&set_buffer_command(sample))
            .expect("set-buffer reply");
        let shown = client.send("show-buffer").expect("show-buffer reply");
        assert_eq!(
            shown.join("\n"),
            sample,
            "round-trip mangled the sample; set-buffer replies were {set_replies:?}"
        );
    }
}

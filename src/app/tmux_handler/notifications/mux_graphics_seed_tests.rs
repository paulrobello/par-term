//! The 0.69.0 `refresh-client -G` adoption: graphics placed in a daemon
//! pane BEFORE a par-term attach replay in the mirror pane after it.
//!
//! The seed is the daemon's `refresh-client -t %N -G` reply — the screen
//! restore sequence plus a bounded `%gfx` section. par-term splits the
//! reply on the last `%graphics-begin` marker, re-encodes each placement
//! as a synthetic OSC 1337 sequence, and processes those bytes through the
//! mirror terminal's own emulator. This test drives the real attach
//! (`attach_until_installed` → the production `attach_sequence`), the
//! layout consumer, and the delivery sweep, then reads the mirror's
//! graphics store.

use super::mux::MuxAttachPending;
use super::mux::tests::{socket_path, spawn_daemon};
use super::mux_test_seams::{attach_until_installed, poll_until};
use crate::app::window_state::WindowState;
use base64::Engine as _;
use image::ImageEncoder as _;
use par_term_emu_core_rust::mux::MuxClient;

/// A shell command whose output is an iTerm2 inline-image sequence the
/// daemon's emulator parses into a stored graphic. The whole OSC sequence
/// rides base64-encoded and the shell decodes it with `base64 -d` — the
/// daemon's send-keys path mangles both raw control bytes (rendered as
/// caret notation) and backslash escapes (stripped by its shell-word
/// splitting).
fn image_command() -> String {
    let (width, height) = (3u32, 2u32);
    let rgba: Vec<u8> = (0..width * height * 4).map(|i| (i * 7) as u8).collect();
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&rgba, width, height, image::ExtendedColorType::Rgba8)
        .expect("png encode");
    let payload = base64::engine::general_purpose::STANDARD.encode(&png);
    // The whole OSC sequence rides base64-encoded: the daemon's send-keys
    // path mangles both raw control bytes (caret notation) and backslash
    // escapes (stripped by its shell-word splitting), so the shell itself
    // generates the sequence with `base64 -d`.
    let sequence = format!(
        "\x1b]1337;File=inline=1;size={};width={width}px;height={height}px;name=t:{}\x07",
        png.len(),
        payload
    );
    format!(
        "echo {} | base64 -d",
        base64::engine::general_purpose::STANDARD.encode(sequence.as_bytes())
    )
}

/// Create the session and paint an image into `%0` daemon-side: type the
/// printf command as literal text, press Enter as its own key press (the
/// wire cannot carry a newline inside `-l` text), then a sentinel echo.
/// The wait is on a line that EQUALS the sentinel — the typed command
/// line contains the same words, so containment would match the echo.
fn seed_graphic(path: &std::path::Path, session: &str) {
    let mut first = MuxClient::connect(path).expect("first client");
    first
        .send(&format!("new-session -s {session}"))
        .expect("new-session");
    for piece in [image_command(), "echo DONE42".to_string()] {
        first
            .send(&format!("send-keys -t %0 -l '{piece}'"))
            .expect("send-keys text");
        first.send("send-keys -t %0 Enter").expect("press Enter");
    }
    let confirmed = poll_until(|| {
        first
            .send("capture-pane -t %0 -p")
            .expect("capture-pane")
            .iter()
            .any(|l| l.trim() == "DONE42")
    });
    assert!(
        confirmed,
        "the pane never executed the image command: {:?}",
        first.send("capture-pane -t %0 -p").unwrap_or_default()
    );
    // Stage 0: the daemon's own store holds the placement — the probe
    // reply must carry the marker, or the rest of the test proves nothing.
    let probe = first.send("refresh-client -t %0 -G").expect("seed probe");
    assert!(
        probe.iter().any(|l| l.trim_end() == "%graphics-begin"),
        "the daemon stored no graphic: probe reply {:?}",
        &probe[..probe.len().min(6)]
    );
    // Dropping the client is the detach; the daemon keeps the graphic.
}

fn window_state() -> WindowState {
    let runtime = std::sync::Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime"),
    );
    WindowState::new(crate::config::Config::default(), runtime)
}

/// Attach `ws` to `session` through the production attach path.
fn reattach(ws: &mut WindowState, path: &std::path::Path, session: &str) {
    attach_until_installed(
        ws,
        || {
            let core_client = MuxClient::connect(path).expect("reattach client");
            let (tx, rx) = std::sync::mpsc::channel();
            tx.send(Ok(core_client)).unwrap();
            drop(tx);
            MuxAttachPending {
                name: session.to_string(),
                rx,
            }
        },
        path,
    );
}

/// The graphics store length of the mirror pane showing `%0`, `None`
/// while the layout consumer has not mapped the pane yet.
fn mirror_graphics(ws: &WindowState) -> Option<usize> {
    let (tab_id, native) = ws.tmux_state.tmux_pane_owner(0)?;
    let tab = ws.tab_manager.get_tab(tab_id)?;
    let pane = tab.pane_manager().and_then(|pm| pm.get_pane(native))?;
    Some(
        pane.terminal
            .try_read()
            .expect("mirror terminal")
            .graphics_count(),
    )
}

/// A graphic placed in the daemon pane before the attach shows in the
/// mirror pane after it.
#[test]
fn graphics_placed_before_attach_replay_in_the_mirror() {
    let path = socket_path("m10-gfx-seed");
    spawn_daemon(&path);
    seed_graphic(&path, "m10gfx");

    let mut ws = window_state();
    reattach(&mut ws, &path, "m10gfx");

    // 1. The attach's -G reply carried a graphics section for %0.
    let seeded = poll_until(|| ws.tmux_state.mux_graphics_osc.contains_key(&0));
    assert!(
        seeded,
        "the daemon seed carried no graphics section for %0 (screen seeds: {:?})",
        ws.tmux_state.mux_screen_seeds.keys().collect::<Vec<_>>()
    );

    // 2. Pump the layout consumer (creates the mirror pane) and the
    //    delivery sweep until the seed lands in the pane's store.
    let replayed = poll_until(|| {
        if let Some(t) = ws.tmux_state.transport.as_ref() {
            // Force a layout push so the consumer creates the mirror pane.
            let _ = t.send_command_no_wait("refresh-client -t %0 -C 80x24");
        }
        ws.check_mux_notifications();
        mirror_graphics(&ws).unwrap_or(0) > 0
    });
    assert!(
        replayed,
        "the mirror pane's graphics store stayed empty: the seed did not replay"
    );

    assert!(ws.detach_mux_session(), "detach the client");
    let _ = std::fs::remove_file(&path);
}

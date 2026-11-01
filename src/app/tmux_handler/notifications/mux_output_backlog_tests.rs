//! Lock-miss output backlog tests: `%output` chunks arriving while a
//! pane's terminal lock is contended must buffer and replay in order, not
//! drop (card 01a194ebe3 — ghost fragments after roster/tab switches).
//!
//! Deliberately daemon-free: a shell-less mirror terminal gives the same
//! lock contention the roster-switch storm produces, deterministically.
//! The OSC 52 assertions pin the clipboard-bridge scope the
//! par-term-emu-core-rust card 01a1938df1 asked for: a clipboard escape
//! split across a lock-miss boundary must survive the replay byte-exact.

use crate::app::window_state::WindowState;
use par_term_terminal::TerminalManager;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// OSC 52 set-clipboard for `payload_b64` (BEL-terminated) — the wire form
/// a pane program emits and the mirror's parser stores.
fn osc52_bytes(payload_b64: &str) -> Vec<u8> {
    format!("\x1b]52;c;{payload_b64}\x07").into_bytes()
}

/// A `WindowState` with no window or renderer — the same seam the mux
/// manners tests and the clipboard-sync tests use.
fn bare_state() -> WindowState {
    let runtime = std::sync::Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime"),
    );
    WindowState::new(crate::config::Config::default(), runtime)
}

/// The mirror-pane clipboard the OSC 52 poll resolves to in a mux tab.
fn mirror_clipboard(ws: &WindowState) -> Option<String> {
    let tab = ws.tab_manager.active_tab().expect("active tab");
    tab.try_with_read_terminal(|term| term.get_clipboard())
        .flatten()
}

/// `%output` chunking is transport-arbitrary: the daemon may split one
/// clipboard escape across chunks, and a chunk landing on a contended lock
/// used to be dropped outright — truncating the escape so the clipboard
/// write silently never happened. Pins all three parts of the fix: the
/// lock-miss chunk is buffered (red proof), the replay is order-preserving,
/// and a split OSC 52 survives the replay byte-exact.
#[test]
fn lock_miss_output_replays_in_order_and_split_osc52_survives() {
    let mut ws = bare_state();
    let tab_id = ws
        .tab_manager
        .new_tab(
            &ws.config.load(),
            std::sync::Arc::clone(&ws.runtime),
            false,
            None,
        )
        .expect("local tab");
    ws.tab_manager.switch_to(tab_id);

    // A shell-less mirror terminal as the daemon-pane terminal — same seam
    // the clipboard-sync tests use for the focused-mirror read path.
    let mirror = TerminalManager::new_with_scrollback(80, 24, 100).expect("mirror terminal");
    let mirror_lock = Arc::new(tokio::sync::RwLock::new(mirror));
    let pane_manager = crate::pane::PaneManager::new_with_existing_terminal(
        Arc::clone(&mirror_lock),
        None,
        Arc::new(AtomicBool::new(false)),
    );
    ws.tab_manager
        .active_tab_mut()
        .expect("active tab for mutation")
        .pane_manager = Some(pane_manager);

    // Map daemon pane %0 to the mirror (native pane id 1: the first pane
    // a fresh PaneManager mints).
    ws.tmux_state.tmux_pane_owners.insert(0, (tab_id, 1));

    // Hold the mirror's write lock: %output now hits the try_read miss,
    // the exact contention a roster-switch re-extract produces.
    let guard = mirror_lock.try_write().expect("mirror lock free");

    // A text marker followed by the first half of an OSC 52 setting
    // "CLIP-ONE" (base64 split mid-payload).
    ws.handle_tmux_output(0, b"MARKER-ONE \x1b]52;c;Q0xJ");
    assert_eq!(
        ws.tmux_state
            .mux_output_backlog
            .get(&0)
            .map(|b| b.as_slice()),
        Some(&b"MARKER-ONE \x1b]52;c;Q0xJ"[..]),
        "a lock-miss chunk must be buffered, not dropped"
    );

    // Second half completes the escape.
    ws.handle_tmux_output(0, b"UC1PTkU=\x07");
    assert_eq!(
        ws.tmux_state
            .mux_output_backlog
            .get(&0)
            .map(|b| b.as_slice()),
        Some(&b"MARKER-ONE \x1b]52;c;Q0xJUC1PTkU=\x07"[..]),
        "lock-miss chunks accumulate in arrival order"
    );

    // Release the lock; the next chunk replays the backlog first, then
    // routes live. Observables that distinguish order: the replayed split
    // escape completes (clipboard becomes CLIP-ONE), and MARKER-ONE lands
    // before MARKER-TWO in the mirror text.
    drop(guard);
    ws.handle_tmux_output(0, b"MARKER-TWO\r\n");
    assert_eq!(
        mirror_clipboard(&ws),
        Some("CLIP-ONE".to_string()),
        "the split OSC 52 must survive the buffered replay"
    );
    assert!(
        !ws.tmux_state.mux_output_backlog.contains_key(&0),
        "the backlog must drain fully once the lock frees"
    );

    // MARKER-ONE (replayed backlog) must sit before MARKER-TWO (live chunk)
    // in the mirror text — the replay preserves stream order.
    let tab = ws.tab_manager.active_tab().expect("active tab");
    let text = tab
        .try_with_read_terminal(|term| {
            let cells = term.get_cells_with_scrollback(0, None, false, None);
            cells
                .iter()
                .map(|cell| {
                    if cell.grapheme.is_empty() || cell.wide_char_spacer {
                        ' '
                    } else {
                        cell.grapheme.chars().next().unwrap_or(' ')
                    }
                })
                .collect::<String>()
        })
        .unwrap_or_default();
    let one = text.find("MARKER-ONE").expect("MARKER-ONE in mirror text");
    let two = text.find("MARKER-TWO").expect("MARKER-TWO in mirror text");
    assert!(
        one < two,
        "the replayed backlog must land before the live chunk"
    );

    // Live routing still works after the drain.
    ws.handle_tmux_output(0, &osc52_bytes("Q0xJUC1UV08=")); // "CLIP-TWO"
    assert_eq!(
        mirror_clipboard(&ws),
        Some("CLIP-TWO".to_string()),
        "the live chunk must route normally after the replay"
    );
}

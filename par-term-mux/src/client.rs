//! The par-mux client: connection lifecycle and the sync pipeline.
//!
//! The core's `MuxClient` owns transport (framing, reply blocks, pushed
//! notifications, daemon spawn). This wrapper adds what par-term's side of
//! the protocol needs: routing core notifications through the UNMODIFIED
//! `ParserBridge` → `TmuxSync` pipeline, and emitting the command forms
//! par-term actually sends — key names from `escape_keys_for_tmux`,
//! literal `-l`, hex `-H`, absolute `resize-pane -x/-y`, and
//! `refresh-client -C`.

use par_term_emu_core_rust::mux::MuxClient;
use par_term_tmux::{ParserBridge, SyncAction, TmuxPaneId, TmuxSync, escape_keys_for_tmux};
use std::io::{self, ErrorKind};
use std::path::Path;
use std::sync::mpsc::TryRecvError;

/// A par-term client attached to a par-mux daemon.
pub struct MuxSessionClient {
    client: MuxClient,
    sync: TmuxSync,
    /// Whether the abrupt-disconnect `SessionEnded` has been emitted, so a
    /// dead channel reports the end of the session exactly once.
    session_ended_emitted: bool,
}

impl MuxSessionClient {
    /// Connect to a daemon already listening at `path`.
    pub fn connect(path: &Path) -> io::Result<Self> {
        Ok(Self {
            client: MuxClient::connect(path)?,
            sync: enabled_sync(),
            session_ended_emitted: false,
        })
    }

    /// The transparency entry point: connect to the default socket for
    /// `name`, starting a daemon if none is running.
    pub fn connect_or_spawn(name: &str) -> io::Result<Self> {
        Ok(Self {
            client: MuxClient::connect_or_spawn(name)?,
            sync: enabled_sync(),
            session_ended_emitted: false,
        })
    }

    /// Connect to a daemon at `path`, spawning one when no live server owns
    /// it. Losing the spawn race is not an error — the winner's daemon is
    /// the correct one to talk to.
    pub fn connect_or_spawn_at(path: &Path) -> io::Result<Self> {
        Ok(Self {
            client: MuxClient::connect_or_spawn_at(path)?,
            sync: enabled_sync(),
            session_ended_emitted: false,
        })
    }

    /// Drain pushed notifications and convert them to app actions.
    ///
    /// Core notifications flow through `ParserBridge` and `TmuxSync`
    /// exactly as the tmux gateway path does — that unmodified reuse is
    /// what the protocol-compatibility claim rests on. A notification
    /// channel that died without `%exit` (daemon killed abruptly) surfaces
    /// as `SessionEnded` once.
    pub fn poll_actions(&mut self) -> Vec<SyncAction> {
        let mut core_notes = Vec::new();
        let mut disconnected = false;
        loop {
            match self.client.notifications().try_recv() {
                Ok(note) => core_notes.push(note),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }
        let frontend_notes = ParserBridge::convert_all(core_notes);
        let mut actions = self.sync.process_notifications(&frontend_notes);
        if disconnected && !self.session_ended_emitted {
            self.session_ended_emitted = true;
            actions.push(SyncAction::SessionEnded);
        }
        actions
    }

    /// Run one control-mode command and return its reply block body.
    pub fn send(&mut self, command: &str) -> io::Result<Vec<String>> {
        self.client.send(command)
    }

    /// Route raw input bytes to a pane as tmux key names.
    ///
    /// This is the form the tmux gateway sends today: the daemon's
    /// `send-keys` accepts `escape_keys_for_tmux`'s key names (`C-a`…
    /// `C-z`, `Escape`, `BSpace`, `Space`, quoted literals, `0xNN`) with
    /// no implicit newline appended.
    pub fn send_keys(&mut self, pane: TmuxPaneId, data: &[u8]) -> io::Result<Vec<String>> {
        let escaped = escape_keys_for_tmux(data);
        self.send(&format!("send-keys -t %{pane} {escaped}"))
    }

    /// Send literal text (no key translation, no appended newline).
    pub fn send_keys_literal(&mut self, pane: TmuxPaneId, text: &str) -> io::Result<Vec<String>> {
        self.send(&format!("send-keys -t %{pane} -l '{text}'"))
    }

    /// Send raw bytes as hex (`-H`).
    pub fn send_keys_hex(&mut self, pane: TmuxPaneId, bytes: &[u8]) -> io::Result<Vec<String>> {
        let hex = bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        self.send(&format!("send-keys -t %{pane} -H {hex}"))
    }

    /// Resize a pane to absolute dimensions — the renderer-driven form
    /// (`-x` columns and/or `-y` rows), replacing the gateway's relative
    /// `-L/-R/-U/-D` cells.
    pub fn resize_pane_absolute(
        &mut self,
        pane: TmuxPaneId,
        cols: Option<u16>,
        rows: Option<u16>,
    ) -> io::Result<Vec<String>> {
        if cols.is_none() && rows.is_none() {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "absolute resize needs -x and/or -y",
            ));
        }
        let mut command = format!("resize-pane -t %{pane}");
        if let Some(cols) = cols {
            command.push_str(&format!(" -x {cols}"));
        }
        if let Some(rows) = rows {
            command.push_str(&format!(" -y {rows}"));
        }
        self.send(&command)
    }

    /// Push the client's grid size; the daemon re-fits the window and
    /// broadcasts `%layout-change` carrying the new geometry.
    pub fn set_client_size(&mut self, cols: u16, rows: u16) -> io::Result<Vec<String>> {
        self.send(&format!("refresh-client -C {cols}x{rows}"))
    }

    /// The sync state, for window/pane ↔ tab/pane mapping on the app side.
    pub fn sync(&mut self) -> &mut TmuxSync {
        &mut self.sync
    }

    /// Terminate the daemon this client spawned, if it started one. A
    /// client that attached to an existing server is a no-op — the tmux
    /// model is daemon-outlives-client.
    pub fn kill_spawned_daemon(&mut self) -> io::Result<()> {
        self.client.kill_spawned_daemon()
    }
}

fn enabled_sync() -> TmuxSync {
    let mut sync = TmuxSync::new();
    sync.enable();
    sync
}

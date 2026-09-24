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

    /// Wrap an already-connected core client. The async attach path uses
    /// this: a worker thread connects/spawns off the event loop and hands
    /// over the plain `MuxClient`; the main thread wraps and installs it.
    pub fn from_core(client: MuxClient) -> Self {
        Self {
            client,
            sync: enabled_sync(),
            session_ended_emitted: false,
        }
    }

    /// Drain raw core notifications and report channel death.
    ///
    /// The app wiring uses this (not [`Self::poll_actions`]) so the
    /// app-level `TmuxSync` stays the single window→tab / pane→native
    /// mapping owner — the same shape the tmux gateway path has, where
    /// polling.rs drains raw notifications and runs the sync itself.
    /// `disconnected` means the notification channel died without `%exit`
    /// (daemon killed abruptly); callers surface that as `SessionEnded`.
    pub fn drain_core_notifications(
        &mut self,
    ) -> (
        Vec<par_term_emu_core_rust::tmux_control::TmuxNotification>,
        bool,
    ) {
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
        (core_notes, disconnected)
    }

    /// Drain pushed notifications and convert them to app actions.
    ///
    /// Core notifications flow through `ParserBridge` and `TmuxSync`
    /// exactly as the tmux gateway path does — that unmodified reuse is
    /// what the protocol-compatibility claim rests on. A notification
    /// channel that died without `%exit` (daemon killed abruptly) surfaces
    /// as `SessionEnded` once.
    pub fn poll_actions(&mut self) -> Vec<SyncAction> {
        let (core_notes, disconnected) = self.drain_core_notifications();
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

    /// Push the client's grid size for the window holding `pane`; the
    /// daemon re-fits that window and broadcasts `%layout-change` carrying
    /// the new geometry. The `-t` target is required by the server — the
    /// pane names the window whose size is being reported (latest report
    /// wins, core T4.C).
    pub fn set_client_size(
        &mut self,
        pane: TmuxPaneId,
        cols: u16,
        rows: u16,
    ) -> io::Result<Vec<String>> {
        self.send(&format!("refresh-client -t %{pane} -C {cols}x{rows}"))
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

    /// Ask the daemon for its build stamp — the `version` command's one
    /// body line (`<version>+<sha>`). The raw reply is returned unvalidated;
    /// [`check_daemon_version`] decides what it means, because a daemon
    /// predating the command answers an error block rather than a stamp.
    pub fn daemon_version(&mut self) -> io::Result<String> {
        let body = self.send("version")?;
        Ok(body.into_iter().next().unwrap_or_default())
    }
}

/// What comparing the daemon's `version` reply against the client's linked
/// core stamp proved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionCheck {
    /// The daemon's build is the client's build.
    Match,
    /// The daemon's build differs from the client's — both display forms
    /// carried for the toast/log ("restart the daemon" is the remedy).
    Mismatch { daemon: String, client: String },
    /// Unprovable either way: the versions are equal but at least one side
    /// was built outside a repository (sha `unknown`), so a same-version
    /// difference cannot be distinguished from a match.
    Unknown,
}

/// Compare a daemon's `version` reply (`daemon_reply`, the raw first body
/// line from [`MuxSessionClient::daemon_version`]) against `client_stamp`
/// (the linked core's `mux::build_stamp()`).
///
/// The daemon outlives its clients, so a stale daemon serving a newer
/// client is the normal failure this exists to catch: daemon-side fixes
/// read as "didn't work" until the daemon is restarted. A reply that is
/// not stamp-shaped is a daemon from before the `version` command existed —
/// older than any client that can ask, which is itself the mismatch.
pub fn check_daemon_version(daemon_reply: &str, client_stamp: &str) -> VersionCheck {
    let shape = |s: &str| {
        s.split_once('+')
            .is_some_and(|(version, sha)| !version.is_empty() && !sha.is_empty())
    };
    if !shape(daemon_reply) {
        return VersionCheck::Mismatch {
            daemon: format!("{daemon_reply:?} (predates the version command)"),
            client: client_stamp.to_string(),
        };
    }
    let (daemon_version, daemon_sha) = daemon_reply.split_once('+').expect("shape checked");
    let (client_version, client_sha) = client_stamp.split_once('+').expect("shape checked");
    if daemon_version != client_version {
        return VersionCheck::Mismatch {
            daemon: daemon_reply.to_string(),
            client: client_stamp.to_string(),
        };
    }
    match (daemon_sha == "unknown", client_sha == "unknown") {
        // Both shas known: the commit-level comparison the stamps exist for.
        (false, false) if daemon_sha != client_sha => VersionCheck::Mismatch {
            daemon: daemon_reply.to_string(),
            client: client_stamp.to_string(),
        },
        (false, false) => VersionCheck::Match,
        // Either sha unknown: equal versions are all the evidence there is.
        _ => VersionCheck::Unknown,
    }
}

fn enabled_sync() -> TmuxSync {
    let mut sync = TmuxSync::new();
    sync.enable();
    sync
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_stamp_matches_and_unknown_sha_pairs_are_inconclusive() {
        assert_eq!(
            check_daemon_version("0.50.0+abc1234", "0.50.0+abc1234"),
            VersionCheck::Match
        );
        // Published-core builds carry no sha: equal versions cannot prove
        // anything either way, and crying wolf on every such attach would
        // teach users to dismiss the toast.
        assert_eq!(
            check_daemon_version("0.50.0+unknown", "0.50.0+unknown"),
            VersionCheck::Unknown
        );
        assert_eq!(
            check_daemon_version("0.50.0+abc1234", "0.50.0+unknown"),
            VersionCheck::Unknown
        );
    }

    #[test]
    fn differing_version_or_sha_is_a_mismatch() {
        assert_eq!(
            check_daemon_version("0.49.0+abc1234", "0.50.0+abc1234"),
            VersionCheck::Mismatch {
                daemon: "0.49.0+abc1234".into(),
                client: "0.50.0+abc1234".into()
            }
        );
        // The incident this exists for: same crate version, different
        // commit — a daemon spawned before a daemon-side fix.
        assert_eq!(
            check_daemon_version("0.50.0+abc1234", "0.50.0+def5678"),
            VersionCheck::Mismatch {
                daemon: "0.50.0+abc1234".into(),
                client: "0.50.0+def5678".into()
            }
        );
    }

    #[test]
    fn a_pre_version_daemon_is_itself_the_mismatch() {
        // A daemon built before the command answers an error block body
        // ("unknown command: version") — older than every client that can
        // ask, so it must never read as healthy.
        assert_eq!(
            check_daemon_version("unknown command: version", "0.50.0+abc1234"),
            VersionCheck::Mismatch {
                daemon: "\"unknown command: version\" (predates the version command)".into(),
                client: "0.50.0+abc1234".into()
            }
        );
        // An empty body (defensive) is just as unstamp-shaped.
        assert_eq!(
            check_daemon_version("", "0.50.0+abc1234"),
            VersionCheck::Mismatch {
                daemon: "\"\" (predates the version command)".into(),
                client: "0.50.0+abc1234".into()
            }
        );
    }
}

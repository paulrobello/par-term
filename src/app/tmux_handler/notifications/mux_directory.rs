//! The par-mux session directory (UX.md A16/A22): which sessions every
//! running daemon holds, without starting a daemon to find out.
//!
//! A session is identified by the daemon socket that serves it plus its
//! name. par-term's own attach convention names the socket after the
//! session (`par-mux-<name>.sock`, `MuxClient::connect_or_spawn`), but a
//! daemon can hold several sessions, so a listed session whose name is not
//! its socket's stem is still attachable — through its socket, never
//! through a name-derived spawn that would create an empty namesake.
//!
//! Each daemon is queried on its own thread with a deadline: a hung daemon
//! costs its own row, never the rest of the list or the frame.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long one daemon may take to answer `list-sessions`.
pub(crate) const DAEMON_QUERY_DEADLINE: Duration = Duration::from_secs(2);

pub(crate) use crate::session_picker_mux::{MuxDirectory, MuxSessionRow, daemon_name_of};

/// The directory the default par-mux sockets live in.
pub(crate) fn default_socket_dir() -> Option<PathBuf> {
    par_term_emu_core_rust::mux::ipc::default_socket_path("x")
        .parent()
        .map(Path::to_path_buf)
}

/// Every par-mux socket file in `dir`, name-sorted.
pub(crate) fn socket_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut sockets: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| daemon_name_of(p).is_some())
        .collect();
    sockets.sort();
    sockets
}

/// Ask one daemon for its sessions. Connect-only: a socket nothing serves
/// fails here, and nothing is spawned in its place.
fn query_daemon(socket: &Path) -> Result<Vec<MuxSessionRow>, String> {
    let daemon = daemon_name_of(socket).unwrap_or_default();
    let mut client = par_term_mux::MuxSessionClient::connect(socket)
        .map_err(|e| format!("par-mux daemon '{daemon}': {e}"))?;
    let sessions = client
        .list_sessions()
        .map_err(|e| format!("par-mux daemon '{daemon}': {e}"))?;
    Ok(sessions
        .into_iter()
        .map(|s| MuxSessionRow {
            socket: socket.to_path_buf(),
            daemon: daemon.clone(),
            id: s.id,
            name: s.name,
        })
        .collect())
}

/// Scan every daemon socket in `dir`, one thread per daemon, each bounded
/// by `deadline`. A dead socket file (a daemon that exited without
/// cleaning up) is skipped silently; a live daemon that errors or hangs is
/// reported in `errors`.
pub(crate) fn scan(dir: &Path, deadline: Duration) -> MuxDirectory {
    let sockets = socket_files(dir);
    let (tx, rx) = mpsc::channel();
    for socket in &sockets {
        let tx = tx.clone();
        let socket = socket.clone();
        let spawned = std::thread::Builder::new()
            .name("mux-directory".into())
            .spawn(move || {
                let _ = tx.send((socket.clone(), query_daemon(&socket)));
            });
        if spawned.is_err() {
            log::warn!("mux directory: could not start a query thread");
        }
    }
    drop(tx);
    let mut dir_out = MuxDirectory::default();
    let mut pending: Vec<PathBuf> = sockets.clone();
    let end = Instant::now() + deadline;
    while !pending.is_empty() {
        let left = end.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok((socket, result)) => {
                pending.retain(|p| p != &socket);
                match result {
                    Ok(rows) => dir_out.sessions.extend(rows),
                    Err(e) if is_dead_socket(&e) => {}
                    Err(e) => dir_out.errors.push(e),
                }
            }
            Err(_) => break,
        }
    }
    for socket in pending {
        let daemon = daemon_name_of(&socket).unwrap_or_default();
        dir_out
            .errors
            .push(format!("par-mux daemon '{daemon}' did not answer"));
    }
    dir_out
        .sessions
        .sort_by(|a, b| (&a.daemon, &a.name).cmp(&(&b.daemon, &b.name)));
    dir_out
}

/// A leftover socket file with no daemon behind it — not worth a message.
fn is_dead_socket(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("connection refused")
        || lower.contains("no such file")
        || lower.contains("not found")
}

/// A background scan in flight. The picker, the palette cache, and the
/// session chip all read the same finished result; polling never blocks.
pub(crate) struct PendingScan {
    rx: mpsc::Receiver<MuxDirectory>,
}

impl PendingScan {
    /// Start a scan of `dir` off the calling thread.
    pub(crate) fn start(dir: PathBuf) -> Option<Self> {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("mux-directory-scan".into())
            .spawn(move || {
                let _ = tx.send(scan(&dir, DAEMON_QUERY_DEADLINE));
            })
            .ok()?;
        Some(Self { rx })
    }

    /// The finished scan, once it lands; `Err(())` when the worker died.
    pub(crate) fn poll(&self) -> Option<Result<MuxDirectory, ()>> {
        match self.rx.try_recv() {
            Ok(dir) => Some(Ok(dir)),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty or missing directory scans to nothing, with no error.
    #[test]
    fn a_missing_directory_scans_empty() {
        let dir = std::env::temp_dir().join(format!("par-term-no-mux-{}", std::process::id()));
        assert_eq!(
            scan(&dir, Duration::from_millis(200)),
            MuxDirectory::default()
        );
    }
}

//! Typed error type for par-term-terminal.
//!
//! Every fallible [`TerminalManager`](crate::TerminalManager) method returns
//! [`TerminalError`], so callers can tell a failed spawn from a dead PTY on
//! write or a failed resize without parsing strings. The underlying core error
//! ([`PtyError`] or [`ScreenshotError`]) is kept as the `source`.
//!
//! The coprocess methods are the exception: they pass through the core
//! coprocess manager's `String` errors unchanged.

use par_term_emu_core_rust::pty_error::PtyError;
use par_term_emu_core_rust::screenshot::ScreenshotError;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Result alias used by [`TerminalManager`](crate::TerminalManager); the error
/// type defaults to [`TerminalError`].
pub type Result<T, E = TerminalError> = std::result::Result<T, E>;

/// Failure of a PTY or terminal operation.
#[derive(Debug, Error)]
pub enum TerminalError {
    /// A shell or command could not be started on the PTY: a process is
    /// already running, the PTY could not be opened, the working directory
    /// could not be entered, or the program could not be executed.
    #[error("Failed to spawn shell process: {0}")]
    Spawn(#[source] PtyError),

    /// Writing input or a paste to the PTY failed — in practice because the
    /// child has exited and the slave end is closed.
    #[error("Failed to write to PTY: {0}")]
    Write(#[source] PtyError),

    /// The PTY resize request failed — typically because the PTY is closed.
    #[error("Failed to resize PTY: {0}")]
    Resize(#[source] PtyError),

    /// The child process could not be killed, or no process was ever spawned.
    #[error("Failed to kill PTY process: {0}")]
    Kill(#[source] PtyError),

    /// Rendering or saving a terminal screenshot failed.
    #[error("Failed to save screenshot: {0}")]
    Screenshot(#[source] ScreenshotError),

    /// A session recording could not be written to disk.
    #[error("Failed to write recording to {}: {source}", path.display())]
    RecordingExport {
        /// Destination path of the export.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}

impl TerminalError {
    /// `map_err` adapter building [`TerminalError::RecordingExport`] for `path`.
    pub(crate) fn recording_export(path: &Path) -> impl FnOnce(std::io::Error) -> Self + '_ {
        move |source| Self::RecordingExport {
            path: path.to_path_buf(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TerminalError;
    use crate::TerminalManager;

    #[test]
    fn unspawned_pty_failures_are_typed() {
        let mut term = TerminalManager::new_with_scrollback(20, 5, 0).expect("terminal");
        assert!(matches!(term.kill(), Err(TerminalError::Kill(_))));

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("missing-dir").join("rec.cast");
        term.terminal().write().start_recording(None);
        let session = term.terminal().write().stop_recording().expect("session");
        match term.export_recording_to_file(&session, &path, "asciicast") {
            Err(TerminalError::RecordingExport { path: p, .. }) => assert_eq!(p, path),
            other => panic!("expected RecordingExport, got {other:?}"),
        }
    }
}

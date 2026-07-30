//! Bounded subprocess execution.
//!
//! `std::process::Command::output()` waits forever. Helper subprocesses invoked from
//! the event loop or an egui closure (`git rev-parse`, `tmux list-sessions`) must not
//! be able to freeze the UI when the tool they call hangs — a network-mounted `.git`,
//! a stale `index.lock`, or an unresponsive tmux server. These run the command with a
//! deadline and kill it when the deadline passes.

use std::io::Read;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// How often the parent re-checks whether the child has exited.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Captured result of a subprocess that exited within its deadline.
pub(crate) struct BoundedOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

/// Run `cmd` to completion, killing it if it outlives `timeout`.
///
/// Returns `Err` if the command cannot be spawned or exceeds the deadline; a
/// non-zero exit is reported through [`BoundedOutput::status`], as with
/// `Command::output`.
pub(crate) fn output_with_timeout(
    cmd: &mut Command,
    timeout: Duration,
) -> Result<BoundedOutput, String> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn: {e}"))?;

    // Drain both pipes on helper threads. Polling `try_wait` without reading would
    // deadlock as soon as the child fills a pipe buffer.
    let mut child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| "stdout pipe unavailable".to_string())?;
    let mut child_stderr = child
        .stderr
        .take()
        .ok_or_else(|| "stderr pipe unavailable".to_string())?;
    let stdout_reader = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = child_stdout.read_to_string(&mut buf);
        buf
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = child_stderr.read_to_string(&mut buf);
        buf
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("timed out after {:.1}s", timeout.as_secs_f64()));
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(e) => return Err(format!("failed while waiting: {e}")),
        }
    };

    Ok(BoundedOutput {
        status,
        stdout: stdout_reader
            .join()
            .map_err(|_| "stdout reader thread panicked".to_string())?,
        stderr: stderr_reader
            .join()
            .map_err(|_| "stderr reader thread panicked".to_string())?,
    })
}

/// Run `cmd` and return its trimmed stdout, or `Err` on spawn failure, non-zero
/// exit, or timeout.
pub(crate) fn stdout_with_timeout(cmd: &mut Command, timeout: Duration) -> Result<String, String> {
    let out = output_with_timeout(cmd, timeout)?;
    if !out.status.success() {
        return Err(format!("exited with {}", out.status));
    }
    Ok(out.stdout.trim().to_string())
}

// Unix-only: the fixtures shell out to `echo` and `sleep`.
#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn captures_stdout_within_deadline() {
        let mut cmd = Command::new("echo");
        cmd.arg("hello");
        let out = stdout_with_timeout(&mut cmd, Duration::from_secs(5)).expect("echo should run");
        assert_eq!(out, "hello");
    }

    #[test]
    fn kills_a_command_that_outlives_the_deadline() {
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        let start = Instant::now();
        let err = stdout_with_timeout(&mut cmd, Duration::from_millis(150))
            .expect_err("sleep 30 must not complete");
        assert!(err.contains("timed out"), "unexpected error: {err}");
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn reports_spawn_failure() {
        let mut cmd = Command::new("par-term-no-such-binary-xyz");
        let err = stdout_with_timeout(&mut cmd, Duration::from_secs(1))
            .expect_err("missing binary must fail");
        assert!(err.contains("failed to spawn"), "unexpected error: {err}");
    }
}

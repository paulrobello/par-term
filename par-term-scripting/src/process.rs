//! Single script subprocess management.
//!
//! [`ScriptProcess`] manages the lifecycle of a single script subprocess, providing
//! piped stdin/stdout/stderr communication. Stdout lines are parsed as JSON
//! [`ScriptCommand`] objects, and stderr lines are collected for error reporting.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use super::protocol::{ScriptCommand, ScriptEvent};

/// Liveness of a script process, polled each frame.
///
/// `Exited` is sticky: once the OS reports an exit, the outcome is remembered so
/// the supervisor observes exactly one running→exited transition rather than a
/// stream of identical exits every frame. [`ScriptProcess::poll_status`] reaps
/// the child on the first `Exited` and records the outcome here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptStatus {
    /// Still alive.
    Running,
    /// Exited. `success` mirrors [`std::process::ExitStatus::success`].
    Exited { success: bool },
}

/// Manages a single script subprocess with JSON-line communication.
///
/// The subprocess receives [`ScriptEvent`] objects serialized as JSON lines on stdin,
/// and emits [`ScriptCommand`] objects as JSON lines on stdout. Stderr is captured
/// separately for error reporting.
pub struct ScriptProcess {
    /// The child process handle, if still alive.
    child: Option<Child>,
    /// Writer to the child's stdin, if still open.
    stdin_writer: Option<std::process::ChildStdin>,
    /// Buffer of parsed commands read from the child's stdout.
    command_buffer: Arc<Mutex<Vec<ScriptCommand>>>,
    /// Buffer of error lines read from the child's stderr.
    error_buffer: Arc<Mutex<Vec<String>>>,
    /// Handle to the background thread reading stdout.
    _stdout_thread: Option<JoinHandle<()>>,
    /// Handle to the background thread reading stderr.
    _stderr_thread: Option<JoinHandle<()>>,
    /// Sticky exit outcome. `None` while running or before the first poll.
    exit: Option<ScriptStatus>,
}

impl ScriptProcess {
    /// Spawn a script subprocess with piped stdin/stdout/stderr.
    ///
    /// Starts background threads to read stdout (parsing JSON into [`ScriptCommand`])
    /// and stderr (collecting error lines).
    ///
    /// # Arguments
    /// * `command` - The command to execute (e.g., "python3").
    /// * `args` - Arguments to pass to the command.
    /// * `env_vars` - Additional environment variables to set for the subprocess.
    ///
    /// # Errors
    /// Returns an error string if the subprocess cannot be spawned.
    pub fn spawn(
        command: &str,
        args: &[&str],
        env_vars: &HashMap<String, String>,
    ) -> Result<Self, String> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(env_vars);

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn '{}': {}", command, e))?;

        let stdin_writer = child.stdin.take();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture stdout".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Failed to capture stderr".to_string())?;

        let command_buffer: Arc<Mutex<Vec<ScriptCommand>>> = Arc::new(Mutex::new(Vec::new()));
        let error_buffer: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

        // Stdout reader thread: parse JSON lines into ScriptCommand
        let cmd_buf = Arc::clone(&command_buffer);
        let stdout_thread = std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(text) => {
                        if text.is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<ScriptCommand>(&text) {
                            Ok(cmd) => {
                                let mut buf = cmd_buf.lock().unwrap_or_else(|e| {
                                    log::warn!("command_buffer mutex poisoned, recovering");
                                    e.into_inner()
                                });
                                buf.push(cmd);
                            }
                            Err(e) => {
                                log::warn!(
                                    "ScriptProcess: failed to parse stdout line as ScriptCommand: {}: {:?}",
                                    e,
                                    text
                                );
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("ScriptProcess: error reading stdout: {}", e);
                        break;
                    }
                }
            }
        });

        // Stderr reader thread: collect error lines
        let err_buf = Arc::clone(&error_buffer);
        let stderr_thread = std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(text) => {
                        if text.is_empty() {
                            continue;
                        }
                        let mut buf = err_buf.lock().unwrap_or_else(|e| {
                            log::warn!("error_buffer mutex poisoned, recovering");
                            e.into_inner()
                        });
                        buf.push(text);
                    }
                    Err(e) => {
                        log::warn!("ScriptProcess: error reading stderr: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(Self {
            child: Some(child),
            stdin_writer,
            command_buffer,
            error_buffer,
            _stdout_thread: Some(stdout_thread),
            _stderr_thread: Some(stderr_thread),
            exit: None,
        })
    }

    /// Poll the process liveness, classifying an exit as clean or failed.
    ///
    /// The first observed exit is sticky: [`ScriptStatus::Exited`] is recorded
    /// and the child handle is dropped, so later calls are cheap and the
    /// supervisor sees the transition exactly once. [`Self::is_running`] is
    /// defined in terms of this.
    pub fn poll_status(&mut self) -> ScriptStatus {
        if let Some(status) = self.exit {
            return status;
        }
        // Compute the outcome in a scope where `self.child` is borrowed, then
        // release that borrow before mutating `self` below.
        let polled = match self.child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(s)) => Some(ScriptStatus::Exited {
                    success: s.success(),
                }),
                Ok(None) => None,
                Err(_) => Some(ScriptStatus::Exited { success: false }),
            },
            // No handle and no recorded exit: treat as a clean exit so a stray
            // poll on an already-cleared slot does not loop.
            None => Some(ScriptStatus::Exited { success: true }),
        };
        match polled {
            Some(status) => {
                // Drop stdin (signals EOF to the reader threads) and release the
                // reaped child handle. The exit is now sticky in `self.exit`.
                self.stdin_writer.take();
                self.child.take();
                self.exit = Some(status);
                status
            }
            None => ScriptStatus::Running,
        }
    }

    /// Check if the child process is still alive.
    ///
    /// Returns `false` once the process has exited (cleanly or otherwise).
    pub fn is_running(&mut self) -> bool {
        matches!(self.poll_status(), ScriptStatus::Running)
    }

    /// Serialize a [`ScriptEvent`] to JSON and write it to the child's stdin as a line.
    ///
    /// # Errors
    /// Returns an error if the stdin writer is not available or if the write fails.
    pub fn send_event(&mut self, event: &ScriptEvent) -> Result<(), String> {
        let stdin = self
            .stdin_writer
            .as_mut()
            .ok_or_else(|| "stdin writer is not available".to_string())?;

        let json = serde_json::to_string(event)
            .map_err(|e| format!("Failed to serialize event: {}", e))?;

        writeln!(stdin, "{}", json).map_err(|e| format!("Failed to write to stdin: {}", e))?;

        stdin
            .flush()
            .map_err(|e| format!("Failed to flush stdin: {}", e))?;

        Ok(())
    }

    /// Drain pending commands from the command buffer.
    ///
    /// Returns all commands that have been parsed from the child's stdout since the
    /// last call to this method.
    pub fn read_commands(&self) -> Vec<ScriptCommand> {
        let mut buf = self.command_buffer.lock().unwrap_or_else(|e| {
            log::warn!("command_buffer mutex poisoned, recovering");
            e.into_inner()
        });
        buf.drain(..).collect()
    }

    /// Drain pending error lines from the error buffer.
    ///
    /// Returns all lines that have been read from the child's stderr since the
    /// last call to this method.
    pub fn read_errors(&self) -> Vec<String> {
        let mut buf = self.error_buffer.lock().unwrap_or_else(|e| {
            log::warn!("error_buffer mutex poisoned, recovering");
            e.into_inner()
        });
        buf.drain(..).collect()
    }

    /// Stop the subprocess.
    ///
    /// Drops the stdin writer (sending EOF to the child), kills the child process
    /// if it's still running, and waits for it to exit.
    pub fn stop(&mut self) {
        // Drop stdin to send EOF
        self.stdin_writer.take();

        if let Some(ref mut child) = self.child {
            // Try to kill the child process
            let _ = child.kill();
            // Wait for the child to exit
            let _ = child.wait();
        }
        self.child.take();
    }
}

impl Drop for ScriptProcess {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::Duration;

    fn no_env() -> HashMap<String, String> {
        HashMap::new()
    }

    #[test]
    fn poll_status_running_for_live_process() {
        let mut p = ScriptProcess::spawn("sleep", &["2"], &no_env()).expect("spawn sleep");
        assert_eq!(p.poll_status(), ScriptStatus::Running);
        p.stop();
    }

    #[test]
    fn poll_status_exited_clean_for_exit_zero() {
        let mut p = ScriptProcess::spawn("true", &[], &no_env()).expect("spawn true");
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(p.poll_status(), ScriptStatus::Exited { success: true });
        // sticky: a second poll reports the same outcome without re-querying the OS
        assert_eq!(p.poll_status(), ScriptStatus::Exited { success: true });
    }

    #[test]
    fn poll_status_exited_failure_for_nonzero_exit() {
        let mut p = ScriptProcess::spawn("false", &[], &no_env()).expect("spawn false");
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(p.poll_status(), ScriptStatus::Exited { success: false });
    }

    #[test]
    fn is_running_false_after_exit() {
        let mut p = ScriptProcess::spawn("true", &[], &no_env()).expect("spawn true");
        std::thread::sleep(Duration::from_millis(50));
        assert!(!p.is_running());
    }
}

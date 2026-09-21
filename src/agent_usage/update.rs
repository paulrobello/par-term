//! Optional update-command runner for the agent-usage store.
//!
//! The design's division of labor (omarchy's): par-term watches a records
//! directory and renders what appears there; keeping the records fresh is a
//! collector's job. When the user configures `agent_usage_update_command`,
//! this runner executes it through `sh -c` on the refresh interval and on
//! manual refresh — off the UI thread, once at a time, killed at a timeout.
//! The command comes from the user's own config (same trust level as
//! `custom_shell`); nothing is ever interpolated into it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// How long a run may take before it is killed.
const RUN_TIMEOUT: Duration = Duration::from_secs(60);

/// Poll granularity of the in-thread wait loop.
const WAIT_TICK: Duration = Duration::from_millis(100);

/// Runs the configured update command, at most one instance at a time.
pub(crate) struct UpdateRunner {
    /// The configured command; `None` is pure watch mode.
    command: Option<String>,
    /// Set by the worker thread when its run finishes; the owner clears it
    /// on the next visit. `None` means no run has been spawned since the
    /// last completion was reaped (or ever).
    done: Option<Arc<AtomicBool>>,
}

impl Default for UpdateRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateRunner {
    /// Build a runner with no command configured.
    pub(crate) fn new() -> Self {
        Self {
            command: None,
            done: None,
        }
    }

    /// Set (or clear) the command to run. Changes take effect on the next
    /// trigger; an in-flight run of the old command is left to finish.
    pub(crate) fn configure(&mut self, command: Option<String>) {
        self.command = command;
    }

    /// Spawn the configured command if one exists and no run is in flight.
    /// Returns whether a run was started.
    pub(crate) fn trigger_if_idle(&mut self) -> bool {
        // Reap a finished run first.
        if let Some(done) = &self.done
            && done.load(Ordering::Relaxed)
        {
            self.done = None;
        }
        let Some(command) = self.command.clone() else {
            return false;
        };
        if self.done.is_some() {
            return false; // still running
        }

        let done = Arc::new(AtomicBool::new(false));
        let worker_done = Arc::clone(&done);
        self.done = Some(done);
        std::thread::spawn(move || {
            run_command(&command);
            worker_done.store(true, Ordering::Relaxed);
        });
        true
    }
}

/// Execute one command under `sh -c`, logging the outcome. Runs on the
/// worker thread; kills the child at [`RUN_TIMEOUT`].
fn run_command(command: &str) {
    let spawned = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => {
            log::warn!("agent-usage update command failed to spawn: {e}");
            return;
        }
    };

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    log::info!("agent-usage update command finished");
                } else {
                    use std::io::Read;
                    let stderr = child
                        .stderr
                        .take()
                        .and_then(|mut s| {
                            let mut buf = String::new();
                            s.read_to_string(&mut buf).ok().map(|_| buf)
                        })
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "none".to_string());
                    log::warn!("agent-usage update command exited {status} (stderr: {stderr})");
                }
                return;
            }
            Ok(None) if started.elapsed() >= RUN_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                log::warn!(
                    "agent-usage update command killed after {}s: {command}",
                    RUN_TIMEOUT.as_secs()
                );
                return;
            }
            Ok(None) => std::thread::sleep(WAIT_TICK),
            Err(e) => {
                log::warn!("agent-usage update command wait failed: {e}");
                return;
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_command_configured_never_spawns() {
        let mut runner = UpdateRunner::new();
        assert!(!runner.trigger_if_idle());
    }

    #[test]
    fn fast_command_completes_and_runner_reaps() {
        let mut runner = UpdateRunner::new();
        runner.configure(Some("echo updated".to_string()));
        assert!(runner.trigger_if_idle(), "first run spawns");
        // The worker sets done when finished; wait for it so the assert
        // below cannot race the 100ms poll loop (echo finishes in ms).
        let done = runner.done.clone().expect("run in flight");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done.load(Ordering::Relaxed) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(done.load(Ordering::Relaxed), "echo run finishes");
        assert!(
            runner.trigger_if_idle(),
            "finished run is reaped and retriggerable"
        );
    }

    #[test]
    fn concurrent_trigger_while_busy_is_refused() {
        let mut runner = UpdateRunner::new();
        runner.configure(Some("sleep 1".to_string()));
        assert!(runner.trigger_if_idle(), "first run spawns");
        assert!(!runner.trigger_if_idle(), "second run refused while busy");
        let done = runner.done.clone().expect("run in flight");
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done.load(Ordering::Relaxed) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

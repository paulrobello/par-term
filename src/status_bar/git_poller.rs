//! Background git branch and status poller for the status bar.
//!
//! `GitBranchPoller` runs a background thread that periodically queries git
//! for the current branch, ahead/behind counts, and dirty status. The results
//! are surfaced to the status bar render loop via a shared `GitStatus`.

use parking_lot::Mutex;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Snapshot of git repository status.
#[derive(Debug, Clone, Default)]
pub struct GitStatus {
    /// Current branch name.
    pub branch: Option<String>,
    /// Commits ahead of upstream.
    pub ahead: u32,
    /// Commits behind upstream.
    pub behind: u32,
    /// Whether the working tree has uncommitted changes.
    pub dirty: bool,
}

/// Git branch poller that runs on a background thread.
pub(super) struct GitBranchPoller {
    /// Shared git status (read from render thread, written by poll thread).
    pub(super) status: Arc<Mutex<GitStatus>>,
    /// Current working directory to poll in.
    cwd: Arc<Mutex<Option<String>>>,
    /// Whether the poller is running.
    running: Arc<AtomicBool>,
    /// Handle to the polling thread.
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl GitBranchPoller {
    pub(super) fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(GitStatus::default())),
            cwd: Arc::new(Mutex::new(None)),
            running: Arc::new(AtomicBool::new(false)),
            thread: Mutex::new(None),
        }
    }

    /// Start the background polling thread.
    pub(super) fn start(&self, poll_interval_secs: f32) {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let status = Arc::clone(&self.status);
        let cwd = Arc::clone(&self.cwd);
        let running = Arc::clone(&self.running);
        let interval = Duration::from_secs_f32(poll_interval_secs.max(1.0));

        let handle = std::thread::Builder::new()
            .name("status-bar-git".into())
            .spawn(move || {
                while running.load(Ordering::SeqCst) {
                    let dir = cwd.lock().clone();
                    let result = dir.map(|d| poll_git_status(&d)).unwrap_or_default();
                    *status.lock() = result;
                    // Sleep in short increments so stop() returns quickly
                    let deadline = Instant::now() + interval;
                    while Instant::now() < deadline && running.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            });

        match handle {
            Ok(h) => *self.thread.lock() = Some(h),
            Err(e) => {
                // Thread spawn failed (e.g. OS out of resources); reset the
                // running flag so start() can be retried and degrade gracefully
                // without crashing the terminal session.
                self.running.store(false, Ordering::SeqCst);
                crate::debug_error!(
                    "SESSION_LOGGER",
                    "failed to spawn git branch poller thread: {:?}",
                    e
                );
            }
        }
    }

    /// Signal the background thread to stop without waiting for it to finish.
    pub(super) fn signal_stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Stop the background polling thread and wait for it to finish.
    pub(super) fn stop(&self) {
        self.signal_stop();
        if let Some(handle) = self.thread.lock().take() {
            let _ = handle.join();
        }
    }

    /// Update the working directory to poll in.
    pub(super) fn set_cwd(&self, new_cwd: Option<&str>) {
        *self.cwd.lock() = new_cwd.map(String::from);
    }

    /// Get the current git status snapshot.
    pub(super) fn status(&self) -> GitStatus {
        self.status.lock().clone()
    }

    pub(super) fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Drop for GitBranchPoller {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Poll git for branch name, ahead/behind counts, and dirty status.
pub(super) fn poll_git_status(dir: &str) -> GitStatus {
    // Get branch name
    let branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir)
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                let b = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if b.is_empty() { None } else { Some(b) }
            } else {
                None
            }
        });

    if branch.is_none() {
        return GitStatus::default();
    }

    // Get ahead/behind counts via rev-list
    let (ahead, behind) = Command::new("git")
        .args(["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])
        .current_dir(dir)
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                let parts: Vec<&str> = text.trim().split('\t').collect();
                if parts.len() == 2 {
                    let a = parts[0].parse::<u32>().unwrap_or(0);
                    let b = parts[1].parse::<u32>().unwrap_or(0);
                    Some((a, b))
                } else {
                    None
                }
            } else {
                // No upstream configured
                None
            }
        })
        .unwrap_or((0, 0));

    // Check dirty status (fast: just check if there are any changes)
    let dirty = Command::new("git")
        .args(["status", "--porcelain", "-uno"])
        .current_dir(dir)
        .output()
        .ok()
        .is_some_and(|out| out.status.success() && !out.stdout.is_empty());

    GitStatus {
        branch,
        ahead,
        behind,
        dirty,
    }
}

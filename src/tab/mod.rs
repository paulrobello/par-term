//! Tab management for multi-tab terminal support
//!
//! This module provides the core tab infrastructure including:
//! - `Tab`: Represents a single terminal session with its own state (supports split panes)
//! - `TabManager`: Coordinates multiple tabs within a window
//! - `TabId`: Unique identifier for each tab

mod activity_state;
mod constructors;
mod initial_text;
mod manager;
mod manager_nav;
mod pane_accessors;
mod pane_ops;
mod profile_state;
mod profile_tracking;
mod refresh_task;
mod scripting_state;
mod session_logging;
mod setup;
mod tmux_state;

pub(crate) use activity_state::TabActivityMonitor;
pub(crate) use profile_state::TabProfileState;
pub(crate) use scripting_state::TabScriptingState;
pub(crate) use tmux_state::TabTmuxState;

use crate::pane::PaneManager;
use crate::session_logger::SharedSessionLogger;
use crate::terminal::TerminalManager;
pub use manager::TabManager;
pub(crate) use setup::{
    apply_login_shell_flag, build_shell_env, configure_terminal_from_config, get_shell_command,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use tokio::runtime::Runtime;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

// Re-export TabId from par-term-config for shared access across subcrates
pub use par_term_config::TabId;

/// A single terminal tab with its own state (supports split panes).
///
/// # Mutex Strategy
///
/// `terminal` is behind a `tokio::sync::RwLock` because `TerminalManager` is
/// shared across async tasks (PTY reader, input sender, resize handler) and the
/// sync winit event loop.  See the global policy in [`crate`] (lib.rs) and the
/// locking table on the `terminal` field below.
///
/// `pane_manager` is owned directly (not behind a Mutex) because it is only ever
/// accessed from the sync winit event loop on the main thread.
///
/// For the complete threading model see `docs/MUTEX_PATTERNS.md`.
pub struct Tab {
    /// Unique identifier for this tab
    pub(crate) id: TabId,
    /// The terminal session for this tab.
    ///
    /// Uses `tokio::sync::RwLock` because `TerminalManager` is shared across async tasks
    /// (PTY reader, input sender, resize handler) and the winit event loop.
    ///
    /// ## Locking rules
    ///
    /// | Caller context | Correct access pattern | Notes |
    /// |----------------|------------------------|-------|
    /// | Async task (Read) | `terminal.read().await` | Async shared access |
    /// | Async task (Write) | `terminal.write().await` | Async exclusive access |
    /// | Sync event loop (Read) | `terminal.try_read()` | Non-blocking; skip if contended |
    /// | Sync event loop (Write) | `terminal.try_write()` | Non-blocking; skip if contended |
    /// | Sync user action (Write)| `terminal.blocking_write()` | OK for infrequent user-initiated ops |
    ///
    /// **Never call `blocking_write()` from within a Tokio worker thread** — it will
    /// deadlock because the blocking call cannot yield to the async scheduler.
    ///
    /// See the struct-level doc on [`Tab`] and `docs/MUTEX_PATTERNS.md` for the full
    /// threading model.
    pub(crate) terminal: Arc<RwLock<TerminalManager>>,
    /// Pane manager for split pane support.
    ///
    /// Always `Some` — initialised with a single primary pane at tab creation
    /// (R-32).  The primary pane shares `Tab::terminal`'s `Arc` so no extra
    /// shell process is spawned.  Additional panes are added on the first user
    /// split; the pane count transitions from 1 → 2.
    ///
    /// Not behind a Mutex — accessed only from the sync winit event loop on the main thread.
    pub(crate) pane_manager: Option<PaneManager>,
    /// Tab title (from OSC sequences or fallback)
    pub(crate) title: String,
    /// Async task for refresh polling
    pub(crate) refresh_task: Option<JoinHandle<()>>,
    /// Working directory when tab was created (for inheriting).
    /// Access via [`Tab::get_cwd`] rather than reading this field directly.
    pub(in crate::tab) working_directory: Option<String>,
    /// Custom tab color [R, G, B] (0-255), overrides config colors when set
    pub(crate) custom_color: Option<[u8; 3]>,
    /// Whether the tab has its default "Tab N" title (not set by OSC, CWD, or user)
    pub(crate) has_default_title: bool,
    /// Whether the user has manually named this tab (makes title static)
    pub(crate) user_named: bool,
    /// Activity monitoring: tab bar indicator, anti-idle, silence, and exit tracking (R-11)
    pub(crate) activity: TabActivityMonitor,
    /// Session logger for automatic session recording
    pub(crate) session_logger: SharedSessionLogger,
    /// Tmux gateway mode and pane identity state
    pub(crate) tmux: TabTmuxState,
    /// Last detected hostname for automatic profile switching (from OSC 7)
    pub(crate) detected_hostname: Option<String>,
    /// Last detected CWD for automatic profile switching (from OSC 7).
    /// Internal tracking state; access the current CWD via [`Tab::get_cwd`].
    pub(in crate::tab) detected_cwd: Option<String>,
    /// Custom icon set by user via context menu (takes precedence over profile_icon)
    pub(crate) custom_icon: Option<String>,
    /// Profile auto-switching state (hostname, directory, SSH)
    pub(crate) profile: TabProfileState,
    /// Scripting, coprocess, and trigger state
    pub(crate) scripting: TabScriptingState,
    /// Whether the terminal was on the alt screen last frame (for detecting transitions)
    pub(crate) was_alt_screen: bool,
    /// Whether this tab is the currently active (visible) tab.
    /// Used by the refresh task to dynamically choose polling interval.
    /// Managed exclusively within the `crate::tab` module.
    pub(in crate::tab) is_active: Arc<AtomicBool>,
    /// When true, Drop impl skips cleanup (terminal Arcs are dropped on background threads)
    pub(crate) shutdown_fast: bool,
    /// When true, this tab is hidden from the tab bar (e.g., tmux gateway tab while windows are active)
    pub(crate) is_hidden: bool,
    /// Last-known modifyOtherKeys level. Updated on every successful read of
    /// `terminal` from the input path; read as a fallback when `try_read()`
    /// fails. Lock contention with the renderer (`try_write` on every frame in
    /// release/LTO builds) was causing modifier-aware key encoding (notably
    /// CSI-u Shift+Enter under tmux) to silently fall back to defaults.
    pub(crate) cached_modify_other_keys_mode: AtomicU8,
    /// Last-known DECCKM (application cursor) state. See cache rationale on
    /// `cached_modify_other_keys_mode`.
    pub(crate) cached_application_cursor: AtomicBool,
    /// Last-known alt-screen state. See cache rationale on
    /// `cached_modify_other_keys_mode`.
    pub(crate) cached_alt_screen_active: AtomicBool,
    /// Last-known result of "is a `tmux*` process running under this tab's
    /// shell?", populated by [`crate::app::WindowState::shell_has_tmux_child`].
    /// Used to disambiguate Shift+Enter encoding when a `try_read` collision
    /// prevents a fresh process-tree query.
    pub(crate) cached_has_tmux_child: AtomicBool,
}

impl Tab {
    /// Read terminal mode flags with cache fallback.
    ///
    /// Tries to acquire a non-blocking read lock; on success, updates the
    /// cached atomics and returns the fresh values. On contention, returns
    /// the last successfully-read values from the cache.
    ///
    /// Returns `(modify_other_keys_mode, application_cursor, alt_screen_active)`.
    ///
    /// Used by the keyboard input path to keep modifier-aware encoding
    /// correct when the renderer's `try_write` collides with our `try_read`
    /// — see field-level docs on `cached_modify_other_keys_mode`.
    pub(crate) fn read_or_cached_modes(&self) -> (u8, bool, bool) {
        if let Ok(term) = self.terminal.try_read() {
            let m = term.modify_other_keys_mode();
            let a = term.application_cursor();
            let s = term.is_alt_screen_active();
            self.cached_modify_other_keys_mode
                .store(m, Ordering::Relaxed);
            self.cached_application_cursor.store(a, Ordering::Relaxed);
            self.cached_alt_screen_active.store(s, Ordering::Relaxed);
            (m, a, s)
        } else {
            (
                self.cached_modify_other_keys_mode.load(Ordering::Relaxed),
                self.cached_application_cursor.load(Ordering::Relaxed),
                self.cached_alt_screen_active.load(Ordering::Relaxed),
            )
        }
    }
}

/// Parameters that differ between `Tab::new()` and `Tab::new_from_profile()`.
///
/// Passed to [`Tab::new_internal`] after the caller has resolved its constructor-specific
/// values (shell command, working directory, tab title).
pub(super) struct TabInitParams {
    /// Unique tab identifier
    pub(super) id: TabId,
    /// Terminal title shown in the tab bar
    pub(super) title: String,
    /// True for auto-generated "Tab N" titles (not set by OSC, CWD, or user)
    pub(super) has_default_title: bool,
    /// True when the user (or profile `tab_name`) has explicitly named the tab
    pub(super) user_named: bool,
    /// Working directory to expose via `Tab::get_cwd`
    pub(super) working_directory: Option<String>,
    /// Used to schedule the initial-text send (if any) in `Tab::new()`
    pub(super) runtime: Option<Arc<Runtime>>,
}

impl Drop for Tab {
    fn drop(&mut self) {
        if self.shutdown_fast {
            log::info!("Fast-dropping tab {} (cleanup handled externally)", self.id);
            return;
        }

        log::info!("Dropping tab {}", self.id);

        // Stop session logging first (before terminal is killed)
        if let Some(ref mut logger) = *self.session_logger.lock() {
            match logger.stop() {
                Ok(path) => {
                    log::info!("Session log saved to: {:?}", path);
                }
                Err(e) => {
                    log::warn!("Failed to stop session logging: {}", e);
                }
            }
        }

        // abort() is non-blocking; no sleep needed after it.
        self.stop_refresh_task();

        // Kill the terminal
        if let Ok(mut term) = self.terminal.try_write()
            && term.is_running()
        {
            log::info!("Killing terminal for tab {}", self.id);
            let _ = term.kill();
        }
    }
}

impl Tab {
    /// Non-blocking read access to this tab's `TerminalManager`.
    ///
    /// Returns `None` on lock contention (expected: another async task holds it).
    /// Use this instead of the inline `if let Ok(term) = tab.terminal.try_read()` pattern
    /// (AUD-031).
    ///
    /// # try_lock rationale
    /// Called from the sync winit event loop. On contention, returns `None` so the
    /// caller can gracefully skip the operation and retry on the next frame.
    #[inline]
    pub(crate) fn try_with_terminal<R>(&self, f: impl FnOnce(&TerminalManager) -> R) -> Option<R> {
        // try_lock: intentional — called from the sync event loop; skip on contention.
        self.terminal.try_read().ok().map(|guard| f(&guard))
    }

    /// Non-blocking write access to this tab's `TerminalManager`.
    ///
    /// Returns `None` on lock contention (expected: another async task holds it).
    /// Use this instead of the inline `if let Ok(mut term) = tab.terminal.try_write()` pattern
    /// (AUD-031).
    ///
    /// # try_lock rationale
    /// Called from the sync winit event loop. On contention, returns `None` so the
    /// caller can gracefully skip the operation and retry on the next frame.
    #[inline]
    pub(crate) fn try_with_terminal_mut<R>(
        &self,
        f: impl FnOnce(&mut TerminalManager) -> R,
    ) -> Option<R> {
        // try_lock: intentional — called from the sync event loop; skip on contention.
        self.terminal
            .try_write()
            .ok()
            .map(|mut guard| f(&mut guard))
    }
}

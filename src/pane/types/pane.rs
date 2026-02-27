//! `Pane` — a single terminal pane with its own PTY session and display state.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::app::bell::BellState;
use crate::app::mouse::MouseState;
use crate::app::render_cache::RenderCache;
use crate::config::Config;
use crate::scroll_state::ScrollState;
use crate::session_logger::{SharedSessionLogger, create_shared_logger};
use crate::tab::{apply_login_shell_flag, build_shell_env, configure_terminal_from_config, get_shell_command};
use crate::terminal::TerminalManager;

use super::bounds::PaneBounds;
use super::common::{PaneBackground, PaneId, RestartState};

/// A single terminal pane with its own state
///
/// # Mutex Strategy
///
/// `terminal` uses `tokio::sync::Mutex` for the same reason as `Tab::terminal`:
/// `TerminalManager` is shared between the async PTY reader task and the sync winit
/// event loop.
///
/// Access rules:
/// - **From async tasks**: `terminal.lock().await`
/// - **From the sync event loop**: `terminal.try_lock()` for polling;
///   `terminal.blocking_lock()` for infrequent user-initiated operations only.
pub struct Pane {
    /// Unique identifier for this pane
    pub id: PaneId,
    /// The terminal session for this pane.
    ///
    /// Uses `tokio::sync::Mutex`. From sync contexts use `.try_lock()` for
    /// non-blocking access or `.blocking_lock()` for user-initiated operations.
    pub terminal: Arc<Mutex<TerminalManager>>,
    /// Scroll state for this pane
    pub scroll_state: ScrollState,
    /// Mouse state for this pane
    pub mouse: MouseState,
    /// Bell state for this pane
    pub bell: BellState,
    /// Render cache for this pane
    pub cache: RenderCache,
    /// Async task for refresh polling
    pub refresh_task: Option<JoinHandle<()>>,
    /// Working directory when pane was created
    pub working_directory: Option<String>,
    /// Last time terminal output (activity) was detected
    pub last_activity_time: std::time::Instant,
    /// Last terminal update generation seen
    pub last_seen_generation: u64,
    /// Last activity time for anti-idle keep-alive
    pub anti_idle_last_activity: std::time::Instant,
    /// Last terminal generation recorded for anti-idle tracking
    pub anti_idle_last_generation: u64,
    /// Whether silence notification has been sent for current idle period
    pub silence_notified: bool,
    /// Whether exit notification has been sent for this pane
    pub exit_notified: bool,
    /// Session logger for automatic session recording
    pub session_logger: SharedSessionLogger,
    /// Current bounds of this pane (updated on layout calculation)
    pub bounds: PaneBounds,
    /// Per-pane background settings (overrides global config if image_path is set)
    pub background: PaneBackground,
    /// State for shell restart behavior (None = shell running or closed normally)
    pub restart_state: Option<RestartState>,
    /// Whether the parent tab is active (shared with tab for refresh throttling)
    pub is_active: Arc<AtomicBool>,
    /// When true, Drop impl skips cleanup (terminal Arcs are dropped on background threads)
    pub(crate) shutdown_fast: bool,
}

impl Pane {
    /// Create a new pane with a terminal session
    pub fn new(
        id: PaneId,
        config: &Config,
        _runtime: Arc<Runtime>,
        working_directory: Option<String>,
    ) -> anyhow::Result<Self> {
        // Create terminal with scrollback from config
        let mut terminal = TerminalManager::new_with_scrollback(
            config.cols,
            config.rows,
            config.scrollback_lines,
        )?;

        // Apply common terminal configuration (theme, clipboard limits, cursor style, unicode)
        configure_terminal_from_config(&mut terminal, config);

        // Determine working directory
        let work_dir = working_directory
            .as_deref()
            .or(config.working_directory.as_deref());

        // Get shell command and apply login shell flag
        #[allow(unused_mut)] // mut is needed on Unix for login shell modification
        let (shell_cmd, mut shell_args) = get_shell_command(config);
        apply_login_shell_flag(&mut shell_args, config);

        let shell_args_deref = shell_args.as_deref();
        let shell_env = build_shell_env(config.shell_env.as_ref());
        terminal.spawn_custom_shell_with_dir(
            &shell_cmd,
            shell_args_deref,
            work_dir,
            shell_env.as_ref(),
        )?;

        // Create shared session logger
        let session_logger = create_shared_logger();

        let terminal = Arc::new(Mutex::new(terminal));

        Ok(Self {
            id,
            terminal,
            scroll_state: ScrollState::new(),
            mouse: MouseState::new(),
            bell: BellState::new(),
            cache: RenderCache::new(),
            refresh_task: None,
            working_directory: working_directory.or_else(|| config.working_directory.clone()),
            last_activity_time: std::time::Instant::now(),
            last_seen_generation: 0,
            anti_idle_last_activity: std::time::Instant::now(),
            anti_idle_last_generation: 0,
            silence_notified: false,
            exit_notified: false,
            session_logger,
            bounds: PaneBounds::default(),
            background: PaneBackground::new(),
            restart_state: None,
            is_active: Arc::new(AtomicBool::new(false)),
            shutdown_fast: false,
        })
    }

    /// Create a new pane for tmux integration (no shell spawned)
    ///
    /// This creates a terminal that receives output from tmux control mode
    /// rather than a local PTY.
    pub fn new_for_tmux(
        id: PaneId,
        config: &Config,
        _runtime: Arc<Runtime>,
    ) -> anyhow::Result<Self> {
        // Create terminal with scrollback from config but don't spawn a shell
        let mut terminal = TerminalManager::new_with_scrollback(
            config.cols,
            config.rows,
            config.scrollback_lines,
        )?;

        // Apply common terminal configuration (theme, clipboard limits, cursor style, unicode)
        configure_terminal_from_config(&mut terminal, config);

        // Don't spawn any shell - tmux provides the output
        // Create shared session logger
        let session_logger = create_shared_logger();

        let terminal = Arc::new(Mutex::new(terminal));

        Ok(Self {
            id,
            terminal,
            scroll_state: ScrollState::new(),
            mouse: MouseState::new(),
            bell: BellState::new(),
            cache: RenderCache::new(),
            refresh_task: None,
            working_directory: None,
            last_activity_time: std::time::Instant::now(),
            last_seen_generation: 0,
            anti_idle_last_activity: std::time::Instant::now(),
            anti_idle_last_generation: 0,
            silence_notified: false,
            exit_notified: false,
            session_logger,
            bounds: PaneBounds::default(),
            background: PaneBackground::new(),
            restart_state: None,
            is_active: Arc::new(AtomicBool::new(false)),
            shutdown_fast: false,
        })
    }

    /// Check if the visual bell is currently active
    pub fn is_bell_active(&self) -> bool {
        const FLASH_DURATION_MS: u128 = 150;
        if let Some(flash_start) = self.bell.visual_flash {
            flash_start.elapsed().as_millis() < FLASH_DURATION_MS
        } else {
            false
        }
    }

    /// Check if the terminal in this pane is still running
    pub fn is_running(&self) -> bool {
        if let Ok(term) = self.terminal.try_lock() {
            let running = term.is_running();
            if !running {
                crate::debug_info!(
                    "PANE_EXIT",
                    "Pane {} terminal detected as NOT running (shell exited)",
                    self.id
                );
            }
            running
        } else {
            true // Assume running if locked
        }
    }

    /// Get the current working directory of this pane's shell
    pub fn get_cwd(&self) -> Option<String> {
        if let Ok(term) = self.terminal.try_lock() {
            term.shell_integration_cwd()
        } else {
            self.working_directory.clone()
        }
    }

    /// Set per-pane background settings (overrides global config)
    pub fn set_background(&mut self, background: PaneBackground) {
        self.background = background;
    }

    /// Get per-pane background settings
    pub fn background(&self) -> &PaneBackground {
        &self.background
    }

    /// Set a per-pane background image (overrides global config)
    pub fn set_background_image(&mut self, path: Option<String>) {
        self.background.image_path = path;
    }

    /// Get the per-pane background image path (if set)
    pub fn get_background_image(&self) -> Option<&str> {
        self.background.image_path.as_deref()
    }

    /// Respawn the shell in this pane
    ///
    /// This resets the terminal state and spawns a new shell process.
    /// Used when shell_exit_action is one of the restart variants.
    pub fn respawn_shell(&mut self, config: &Config) -> anyhow::Result<()> {
        // Clear restart state
        self.restart_state = None;
        self.exit_notified = false;

        // Determine the shell command to use
        #[allow(unused_mut)]
        let (shell_cmd, mut shell_args) = if let Some(ref custom) = config.custom_shell {
            (custom.clone(), config.shell_args.clone())
        } else {
            #[cfg(target_os = "windows")]
            {
                ("powershell.exe".to_string(), None)
            }
            #[cfg(not(target_os = "windows"))]
            {
                (
                    std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string()),
                    None,
                )
            }
        };

        // On Unix-like systems, spawn as login shell if configured
        #[cfg(not(target_os = "windows"))]
        if config.login_shell {
            let args = shell_args.get_or_insert_with(Vec::new);
            if !args.iter().any(|a| a == "-l" || a == "--login") {
                args.insert(0, "-l".to_string());
            }
        }

        // Determine working directory (use current CWD if available, else config)
        let work_dir = self
            .get_cwd()
            .or_else(|| self.working_directory.clone())
            .or_else(|| config.working_directory.clone());

        let shell_args_deref = shell_args.as_deref();
        let shell_env = build_shell_env(config.shell_env.as_ref());

        // Respawn the shell
        if let Ok(mut term) = self.terminal.try_lock() {
            // Clear the screen before respawning (using VT escape sequence)
            // This clears screen and moves cursor to home position
            term.process_data(b"\x1b[2J\x1b[H");

            // Spawn new shell
            term.spawn_custom_shell_with_dir(
                &shell_cmd,
                shell_args_deref,
                work_dir.as_deref(),
                shell_env.as_ref(),
            )?;

            log::info!("Respawned shell in pane {}", self.id);
        }

        Ok(())
    }

    /// Write a restart prompt message to the terminal
    pub fn write_restart_prompt(&self) {
        if let Ok(term) = self.terminal.try_lock() {
            // Write the prompt message directly to terminal display
            let message = "\r\n[Process exited. Press Enter to restart...]\r\n";
            term.process_data(message.as_bytes());
        }
    }

    /// Get the title for this pane (from OSC or CWD)
    pub fn get_title(&self) -> String {
        if let Ok(term) = self.terminal.try_lock() {
            let osc_title = term.get_title();
            if !osc_title.is_empty() {
                return osc_title;
            }
            if let Some(cwd) = term.shell_integration_cwd() {
                // Abbreviate home directory to ~
                let abbreviated = if let Some(home) = dirs::home_dir() {
                    cwd.replace(&home.to_string_lossy().to_string(), "~")
                } else {
                    cwd
                };
                // Use just the last component for brevity
                if let Some(last) = abbreviated.rsplit('/').next()
                    && !last.is_empty()
                {
                    return last.to_string();
                }
                return abbreviated;
            }
        }
        format!("Pane {}", self.id)
    }

    /// Start the refresh polling task for this pane
    pub fn start_refresh_task(
        &mut self,
        runtime: Arc<Runtime>,
        window: Arc<winit::window::Window>,
        active_fps: u32,
        inactive_fps: u32,
    ) {
        let terminal_clone = Arc::clone(&self.terminal);
        let is_active = Arc::clone(&self.is_active);
        let active_interval_ms = (1000 / active_fps.max(1)) as u64;
        let inactive_interval_ms = (1000 / inactive_fps.max(1)) as u64;

        let handle = runtime.spawn(async move {
            let mut last_gen = 0u64;
            let mut idle_streak = 0u32;
            const MAX_INACTIVE_IDLE_INTERVAL_MS: u64 = 250;

            loop {
                let is_active_now = is_active.load(Ordering::Relaxed);
                // Keep the active tab responsive: only apply backoff to inactive tabs.
                let interval_ms = if is_active_now {
                    active_interval_ms
                } else if idle_streak > 0 {
                    (inactive_interval_ms << idle_streak.min(4)).min(MAX_INACTIVE_IDLE_INTERVAL_MS)
                } else {
                    inactive_interval_ms
                };
                tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)).await;

                let should_redraw = if let Ok(term) = terminal_clone.try_lock() {
                    let current_gen = term.update_generation();
                    if current_gen > last_gen {
                        last_gen = current_gen;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };

                if should_redraw {
                    idle_streak = 0;
                    window.request_redraw();
                } else if is_active_now {
                    idle_streak = 0;
                } else {
                    idle_streak = idle_streak.saturating_add(1);
                }
            }
        });

        self.refresh_task = Some(handle);
    }

    /// Stop the refresh polling task
    pub fn stop_refresh_task(&mut self) {
        if let Some(handle) = self.refresh_task.take() {
            handle.abort();
        }
    }

    /// Resize the terminal to match the pane bounds
    pub fn resize_terminal(&self, cols: usize, rows: usize) {
        if let Ok(mut term) = self.terminal.try_lock()
            && term.dimensions() != (cols, rows)
        {
            let _ = term.resize(cols, rows);
        }
    }

    /// Resize the terminal and update cell pixel dimensions.
    ///
    /// Unlike `resize_terminal`, this also calls `set_cell_dimensions` so that
    /// the core library tracks `scroll_offset_rows` in display-cell units rather
    /// than its internal default (2 px per row).  Must be called whenever the
    /// display cell size is known (e.g., on every layout pass).
    pub fn resize_terminal_with_cell_dims(
        &self,
        cols: usize,
        rows: usize,
        cell_width: u32,
        cell_height: u32,
    ) {
        if let Ok(mut term) = self.terminal.try_lock() {
            term.set_cell_dimensions(cell_width, cell_height);
            if term.dimensions() != (cols, rows) {
                let _ = term.resize(cols, rows);
            }
        }
    }
}

impl Drop for Pane {
    fn drop(&mut self) {
        if self.shutdown_fast {
            log::info!(
                "Fast-dropping pane {} (cleanup handled externally)",
                self.id
            );
            return;
        }

        log::info!("Dropping pane {}", self.id);

        // Stop session logging first
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

        self.stop_refresh_task();

        // Give the task time to abort
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Kill the terminal
        if let Ok(mut term) = self.terminal.try_lock()
            && term.is_running()
        {
            log::info!("Killing terminal for pane {}", self.id);
            let _ = term.kill();
        }
    }
}

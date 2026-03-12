//! `Pane` — a single terminal pane with its own PTY session and display state.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tokio::runtime::Runtime;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::pane::bell::BellState;
use crate::pane::mouse::MouseState;
use crate::pane::render_cache::RenderCache;
use crate::scroll_state::ScrollState;
use crate::session_logger::{SharedSessionLogger, create_shared_logger};
use crate::tab::{
    apply_login_shell_flag, build_shell_env, configure_terminal_from_config, get_shell_command,
};
use crate::terminal::TerminalManager;
use crate::ui_constants::VISUAL_BELL_FLASH_DURATION_MS;

use super::bounds::PaneBounds;
use super::common::{PaneBackground, PaneId, RestartState};

/// A single terminal pane with its own state
///
/// # RwLock Strategy
///
/// `terminal` uses `tokio::sync::RwLock` for the same reason as `Tab::terminal`:
/// `TerminalManager` is shared between the async PTY reader task and the sync winit
/// event loop.
///
/// Access rules:
/// - **From async tasks**: `terminal.read().await` or `terminal.write().await`
/// - **From the sync event loop**: `terminal.try_read()` or `terminal.try_write()` for polling;
///   `terminal.blocking_write()` for infrequent user-initiated operations only.
pub struct Pane {
    /// Unique identifier for this pane
    pub id: PaneId,
    /// The terminal session for this pane.
    ///
    /// Uses `tokio::sync::RwLock`. From sync contexts use `.try_read()` or `.try_write()` for
    /// non-blocking access or `.blocking_write()` for user-initiated operations.
    pub terminal: Arc<RwLock<TerminalManager>>,
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
            config.scrollback.scrollback_lines,
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

        let terminal = Arc::new(RwLock::new(terminal));

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

    /// Create a pane that launches `command args` instead of the configured login shell.
    ///
    /// Identical to `Pane::new()` except the PTY is started with the given command.
    /// All other fields (scroll state, cache, refresh task, etc.) are the same as `new()`.
    pub fn new_with_command(
        id: PaneId,
        config: &Config,
        _runtime: Arc<Runtime>,
        working_directory: Option<String>,
        command: String,
        args: Vec<String>,
    ) -> anyhow::Result<Self> {
        // Create terminal with scrollback from config
        let mut terminal = TerminalManager::new_with_scrollback(
            config.cols,
            config.rows,
            config.scrollback.scrollback_lines,
        )?;

        // Apply common terminal configuration (theme, clipboard limits, cursor style, unicode)
        configure_terminal_from_config(&mut terminal, config);

        // Determine working directory
        let work_dir = working_directory
            .as_deref()
            .or(config.working_directory.as_deref());

        // Spawn the caller-supplied command instead of the login shell
        let shell_env = build_shell_env(config.shell_env.as_ref());
        terminal.spawn_custom_shell_with_dir(
            &command,
            Some(args.as_slice()),
            work_dir,
            shell_env.as_ref(),
        )?;

        // Create shared session logger
        let session_logger = create_shared_logger();

        let terminal = Arc::new(RwLock::new(terminal));

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

    /// Create a primary pane that wraps an already-running terminal session.
    ///
    /// Unlike [`Pane::new`], this constructor does **not** spawn a new shell.
    /// It shares the caller's `Arc<RwLock<TerminalManager>>` directly, so the
    /// pane's terminal is the same session that `Tab::terminal` points to.
    ///
    /// Used by `Tab::new_internal` to always initialise a `PaneManager` with a
    /// single primary pane at tab creation, removing the need for tab-level
    /// `scroll_state`, `mouse`, `bell`, and `cache` fallback fields (R-32).
    ///
    /// # Arguments
    /// * `id` — Pane identifier (typically `1`)
    /// * `terminal` — Shared `Arc` cloned from the owning `Tab::terminal`
    /// * `working_directory` — Optional CWD exposed via [`Pane::get_cwd`]
    /// * `is_active` — Shared atomic flag cloned from the owning `Tab::is_active`
    pub fn new_wrapping_terminal(
        id: PaneId,
        terminal: Arc<RwLock<TerminalManager>>,
        working_directory: Option<String>,
        is_active: Arc<AtomicBool>,
    ) -> Self {
        let session_logger = create_shared_logger();

        Self {
            id,
            terminal,
            scroll_state: ScrollState::new(),
            mouse: MouseState::new(),
            bell: BellState::new(),
            cache: RenderCache::new(),
            refresh_task: None,
            working_directory,
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
            is_active,
            shutdown_fast: false,
        }
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
            config.scrollback.scrollback_lines,
        )?;

        // Apply common terminal configuration (theme, clipboard limits, cursor style, unicode)
        configure_terminal_from_config(&mut terminal, config);

        // Don't spawn any shell - tmux provides the output
        // Create shared session logger
        let session_logger = create_shared_logger();

        let terminal = Arc::new(RwLock::new(terminal));

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
        if let Some(flash_start) = self.bell.visual_flash {
            flash_start.elapsed().as_millis() < VISUAL_BELL_FLASH_DURATION_MS
        } else {
            false
        }
    }

    /// Check if the terminal in this pane is still running
    pub fn is_running(&self) -> bool {
        if let Ok(term) = self.terminal.try_write() {
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
        if let Ok(term) = self.terminal.try_write() {
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
        if let Ok(mut term) = self.terminal.try_write()
            && term.is_running()
        {
            log::info!("Killing terminal for pane {}", self.id);
            let _ = term.kill();
        }
    }
}

//! Tab management for multi-tab terminal support
//!
//! This module provides the core tab infrastructure including:
//! - `Tab`: Represents a single terminal session with its own state (supports split panes)
//! - `TabManager`: Coordinates multiple tabs within a window
//! - `TabId`: Unique identifier for each tab

mod initial_text;
mod manager;
mod pane_ops;
mod profile_tracking;
mod refresh_task;
mod session_logging;
mod setup;

use crate::app::bell::BellState;
use crate::app::mouse::MouseState;
use crate::app::render_cache::RenderCache;
use crate::config::Config;
use crate::pane::PaneManager;
use crate::prettifier::gutter::GutterManager;
use crate::prettifier::pipeline::PrettifierPipeline;
use crate::profile::Profile;
use crate::scroll_state::ScrollState;
use crate::session_logger::{SessionLogger, SharedSessionLogger, create_shared_logger};
use crate::tab::initial_text::build_initial_text_payload;
use crate::terminal::TerminalManager;
pub use manager::TabManager;
use par_term_emu_core_rust::coprocess::CoprocessId;
pub(crate) use setup::{
    apply_login_shell_flag, build_shell_env, configure_terminal_from_config, get_shell_command,
};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

// Re-export TabId from par-term-config for shared access across subcrates
pub use par_term_config::TabId;

/// A single terminal tab with its own state (supports split panes)
///
/// # Mutex Strategy
///
/// `terminal` uses `tokio::sync::Mutex` because `TerminalManager` is shared across async
/// tasks (PTY reader, input sender, resize handler) and the winit event loop.
///
/// Access rules:
/// - **From async tasks** (spawned with `runtime.spawn`): `terminal.lock().await`
/// - **From the sync winit event loop**: `terminal.try_lock()` for non-blocking polling;
///   `terminal.blocking_lock()` only for infrequent user-initiated operations
///   (e.g., start/stop coprocess, register scripting observer).
///
/// Never call `blocking_lock()` inside an async context — it will deadlock if called
/// from within a Tokio worker thread.
///
/// `pane_manager` is owned directly (not behind a Mutex) because it is only ever
/// accessed from the sync winit event loop on the main thread.
pub struct Tab {
    /// Unique identifier for this tab
    pub(crate) id: TabId,
    /// The terminal session for this tab.
    ///
    /// Uses `tokio::sync::Mutex` because `TerminalManager` is shared across async tasks
    /// (PTY reader, input sender, resize handler) and the winit event loop.
    ///
    /// ## Locking rules
    ///
    /// | Caller context | Correct access pattern | Notes |
    /// |----------------|------------------------|-------|
    /// | Async task (`runtime.spawn`) | `terminal.lock().await` | Standard async lock |
    /// | Sync winit event loop (polling) | `terminal.try_lock()` | Non-blocking; skip if contended |
    /// | Sync winit event loop (user action) | `terminal.blocking_lock()` | OK for infrequent user-initiated ops (start/stop coprocess, register observer) |
    ///
    /// **Never call `blocking_lock()` from within a Tokio worker thread** — it will
    /// deadlock because the blocking call cannot yield to the async scheduler.
    ///
    /// See the struct-level doc on [`Tab`] and `docs/MUTEX_PATTERNS.md` for the full
    /// threading model.
    pub(crate) terminal: Arc<Mutex<TerminalManager>>,
    /// Pane manager for split pane support.
    ///
    /// Not behind a Mutex — accessed only from the sync winit event loop on the main thread.
    pub(crate) pane_manager: Option<PaneManager>,
    /// Tab title (from OSC sequences or fallback)
    pub(crate) title: String,
    /// Whether this tab has unread activity since last viewed
    pub(crate) has_activity: bool,
    /// Scroll state for this tab.
    /// Legacy field: each pane has its own scroll state. Will be removed in a future version.
    pub(crate) scroll_state: ScrollState,
    /// Mouse state for this tab.
    /// Legacy field: each pane has its own mouse state. Will be removed in a future version.
    pub(crate) mouse: MouseState,
    /// Bell state for this tab.
    /// Legacy field: each pane has its own bell state. Will be removed in a future version.
    pub(crate) bell: BellState,
    /// Render cache for this tab.
    /// Legacy field: each pane has its own render cache. Will be removed in a future version.
    pub(crate) cache: RenderCache,
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
    /// Last time terminal output (activity) was detected
    pub(crate) last_activity_time: std::time::Instant,
    /// Last terminal update generation seen (to detect new output)
    pub(crate) last_seen_generation: u64,
    /// Last activity time for anti-idle keep-alive
    pub(crate) anti_idle_last_activity: std::time::Instant,
    /// Last terminal generation recorded for anti-idle tracking
    pub(crate) anti_idle_last_generation: u64,
    /// Whether silence notification has been sent for current idle period
    pub(crate) silence_notified: bool,
    /// Whether exit notification has been sent for this tab
    pub(crate) exit_notified: bool,
    /// Session logger for automatic session recording
    pub(crate) session_logger: SharedSessionLogger,
    /// Whether this tab is in tmux gateway mode
    pub(crate) tmux_gateway_active: bool,
    /// The tmux pane ID this tab represents (when in gateway mode)
    pub(crate) tmux_pane_id: Option<crate::tmux::TmuxPaneId>,
    /// Last detected hostname for automatic profile switching (from OSC 7)
    pub(crate) detected_hostname: Option<String>,
    /// Last detected CWD for automatic profile switching (from OSC 7).
    /// Internal tracking state; access the current CWD via [`Tab::get_cwd`].
    pub(in crate::tab) detected_cwd: Option<String>,
    /// Profile ID that was auto-applied based on hostname detection
    pub(crate) auto_applied_profile_id: Option<crate::profile::ProfileId>,
    /// Profile ID that was auto-applied based on directory pattern matching
    pub(crate) auto_applied_dir_profile_id: Option<crate::profile::ProfileId>,
    /// Icon from auto-applied profile (displayed in tab bar)
    pub(crate) profile_icon: Option<String>,
    /// Custom icon set by user via context menu (takes precedence over profile_icon)
    pub(crate) custom_icon: Option<String>,
    /// Original tab title saved before auto-profile override (restored when profile clears)
    pub(crate) pre_profile_title: Option<String>,
    /// Badge text override from auto-applied profile (overrides global badge_format)
    pub(crate) badge_override: Option<String>,
    /// Mapping from config index to coprocess ID (for UI tracking)
    pub(crate) coprocess_ids: Vec<Option<CoprocessId>>,
    /// Script manager for this tab
    pub(crate) script_manager: crate::scripting::manager::ScriptManager,
    /// Maps config index to ScriptId for running scripts
    pub(crate) script_ids: Vec<Option<crate::scripting::manager::ScriptId>>,
    /// Observer IDs registered with the terminal for script event forwarding
    pub(crate) script_observer_ids: Vec<Option<par_term_emu_core_rust::observer::ObserverId>>,
    /// Event forwarders (shared with observer registration)
    pub(crate) script_forwarders:
        Vec<Option<std::sync::Arc<crate::scripting::observer::ScriptEventForwarder>>>,
    /// Trigger-generated scrollbar marks (from MarkLine actions)
    pub(crate) trigger_marks: Vec<crate::scrollback_metadata::ScrollbackMark>,
    /// Security metadata: maps trigger_id -> require_user_action flag.
    /// When true, dangerous actions (RunCommand, SendText) from that trigger
    /// are suppressed when fired from passive terminal output.
    pub(crate) trigger_security: std::collections::HashMap<u64, bool>,
    /// Rate limiter for output-triggered dangerous actions.
    pub(crate) trigger_rate_limiter: par_term_config::TriggerRateLimiter,
    /// Prettifier pipeline for content detection and rendering (None if disabled)
    pub(crate) prettifier: Option<PrettifierPipeline>,
    /// Gutter manager for prettifier indicators
    pub(crate) gutter_manager: GutterManager,
    /// Whether the terminal was on the alt screen last frame (for detecting transitions)
    pub(crate) was_alt_screen: bool,
    /// Profile saved before SSH auto-switch (for revert on disconnect)
    pub(crate) pre_ssh_switch_profile: Option<crate::profile::ProfileId>,
    /// Whether current profile was auto-applied due to SSH hostname detection
    pub(crate) ssh_auto_switched: bool,
    /// Whether this tab is the currently active (visible) tab.
    /// Used by the refresh task to dynamically choose polling interval.
    /// Managed exclusively within the `crate::tab` module.
    pub(in crate::tab) is_active: Arc<AtomicBool>,
    /// When true, Drop impl skips cleanup (terminal Arcs are dropped on background threads)
    pub(crate) shutdown_fast: bool,
    /// When true, a deferred call to `set_tmux_control_mode(false)` is pending.
    ///
    /// Set when `handle_tmux_session_ended` could not acquire the terminal lock via
    /// `try_lock()`. The notification poll loop retries on each subsequent frame until
    /// the lock is available, ensuring the terminal parser exits tmux control mode even
    /// if the lock was transiently held at cleanup time.
    pub(crate) pending_tmux_mode_disable: bool,
}

impl Tab {
    /// Create a new tab with a terminal session
    ///
    /// # Arguments
    /// * `id` - Unique tab identifier
    /// * `tab_number` - Display number for the tab (1-indexed)
    /// * `config` - Terminal configuration
    /// * `runtime` - Tokio runtime for async operations
    /// * `working_directory` - Optional working directory to start in
    /// * `grid_size` - Optional (cols, rows) override. When provided, uses these
    ///   dimensions instead of config.cols/rows. This ensures the shell starts
    ///   with the correct dimensions when the renderer has already calculated
    ///   the grid size accounting for tab bar height.
    pub fn new(
        id: TabId,
        tab_number: usize,
        config: &Config,
        runtime: Arc<Runtime>,
        working_directory: Option<String>,
        grid_size: Option<(usize, usize)>,
    ) -> anyhow::Result<Self> {
        // Use provided grid size if available, otherwise fall back to config
        let (cols, rows) = grid_size.unwrap_or((config.cols, config.rows));

        // Create terminal with scrollback from config
        let mut terminal =
            TerminalManager::new_with_scrollback(cols, rows, config.scrollback_lines)?;

        // Apply common terminal configuration
        configure_terminal_from_config(&mut terminal, config);

        // Determine working directory:
        // 1. If explicitly provided (e.g., from tab_inherit_cwd), use that
        // 2. Otherwise, use the configured startup directory based on mode
        let effective_startup_dir = config.get_effective_startup_directory();
        let work_dir = working_directory
            .as_deref()
            .or(effective_startup_dir.as_deref());

        // Get shell command and apply login shell flag
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

        // Sync triggers from config into the core TriggerRegistry
        let trigger_security = terminal.sync_triggers(&config.triggers);

        // Auto-start configured coprocesses via the PtySession's built-in manager
        let mut coprocess_ids = Vec::with_capacity(config.coprocesses.len());
        for coproc_config in &config.coprocesses {
            if coproc_config.auto_start {
                let core_config = par_term_emu_core_rust::coprocess::CoprocessConfig {
                    command: coproc_config.command.clone(),
                    args: coproc_config.args.clone(),
                    cwd: None,
                    env: crate::terminal::coprocess_env(),
                    copy_terminal_output: coproc_config.copy_terminal_output,
                    restart_policy: coproc_config.restart_policy.to_core(),
                    restart_delay_ms: coproc_config.restart_delay_ms,
                };
                match terminal.start_coprocess(core_config) {
                    Ok(id) => {
                        log::info!(
                            "Auto-started coprocess '{}' (id={})",
                            coproc_config.name,
                            id
                        );
                        coprocess_ids.push(Some(id));
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to auto-start coprocess '{}': {}",
                            coproc_config.name,
                            e
                        );
                        coprocess_ids.push(None);
                    }
                }
            } else {
                coprocess_ids.push(None);
            }
        }

        // Create shared session logger
        let session_logger = create_shared_logger();

        // Set up session logging if enabled
        if config.auto_log_sessions {
            let logs_dir = config.logs_dir();
            let session_title = Some(format!(
                "Tab {} - {}",
                tab_number,
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            ));

            match SessionLogger::new(
                config.session_log_format,
                &logs_dir,
                (config.cols, config.rows),
                session_title,
            ) {
                Ok(mut logger) => {
                    logger.set_redact_passwords(config.session_log_redact_passwords);
                    if let Err(e) = logger.start() {
                        log::warn!("Failed to start session logging: {}", e);
                    } else {
                        log::info!("Session logging started: {:?}", logger.output_path());

                        // Set up output callback to record PTY output
                        let logger_clone = Arc::clone(&session_logger);
                        terminal.set_output_callback(move |data: &[u8]| {
                            if let Some(ref mut logger) = *logger_clone.lock() {
                                logger.record_output(data);
                            }
                        });

                        *session_logger.lock() = Some(logger);
                    }
                }
                Err(e) => {
                    log::warn!("Failed to create session logger: {}", e);
                }
            }
        }

        let terminal = Arc::new(Mutex::new(terminal));

        // Send initial text after optional delay
        if let Some(payload) =
            build_initial_text_payload(&config.initial_text, config.initial_text_send_newline)
        {
            let delay_ms = config.initial_text_delay_ms;
            let terminal_clone = Arc::clone(&terminal);
            runtime.spawn(async move {
                if delay_ms > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                }

                let term = terminal_clone.lock().await;
                if let Err(err) = term.write(&payload) {
                    log::warn!("Failed to send initial text: {}", err);
                }
            });
        }

        // Generate initial title based on current tab count, not unique ID
        let title = format!("Tab {}", tab_number);

        Ok(Self {
            id,
            terminal,
            pane_manager: None, // Created on first split
            title,
            has_activity: false,
            scroll_state: ScrollState::new(),
            mouse: MouseState::new(),
            bell: BellState::new(),
            cache: RenderCache::new(),
            refresh_task: None,
            working_directory: working_directory.or_else(|| config.working_directory.clone()),
            custom_color: None,
            has_default_title: true,
            user_named: false,
            last_activity_time: std::time::Instant::now(),
            last_seen_generation: 0,
            anti_idle_last_activity: std::time::Instant::now(),
            anti_idle_last_generation: 0,
            silence_notified: false,
            exit_notified: false,
            session_logger,
            tmux_gateway_active: false,
            tmux_pane_id: None,
            detected_hostname: None,
            detected_cwd: None,
            auto_applied_profile_id: None,
            auto_applied_dir_profile_id: None,
            profile_icon: None,
            custom_icon: None,
            pre_profile_title: None,
            badge_override: None,
            coprocess_ids,
            script_manager: crate::scripting::manager::ScriptManager::new(),
            script_ids: Vec::new(),
            script_observer_ids: Vec::new(),
            script_forwarders: Vec::new(),
            trigger_marks: Vec::new(),
            trigger_security,
            trigger_rate_limiter: par_term_config::TriggerRateLimiter::default(),
            prettifier: crate::prettifier::config_bridge::create_pipeline_from_config(
                config, cols, None,
            ),
            gutter_manager: GutterManager::new(),
            was_alt_screen: false,
            pre_ssh_switch_profile: None,
            ssh_auto_switched: false,
            is_active: Arc::new(AtomicBool::new(false)),
            shutdown_fast: false,
            pending_tmux_mode_disable: false,
        })
    }

    /// Create a new tab from a profile configuration
    ///
    /// The profile can override:
    /// - Working directory
    /// - Command and arguments (instead of default shell)
    /// - Tab name
    ///
    /// If a profile specifies a command, it always runs from the profile's working
    /// directory (or config default if unset).
    ///
    /// # Arguments
    /// * `id` - Unique tab identifier
    /// * `config` - Terminal configuration
    /// * `_runtime` - Tokio runtime (unused but kept for API consistency)
    /// * `profile` - Profile configuration to use
    /// * `grid_size` - Optional (cols, rows) override for initial terminal size
    pub fn new_from_profile(
        id: TabId,
        config: &Config,
        _runtime: Arc<Runtime>,
        profile: &Profile,
        grid_size: Option<(usize, usize)>,
    ) -> anyhow::Result<Self> {
        // Use provided grid size if available, otherwise fall back to config
        let (cols, rows) = grid_size.unwrap_or((config.cols, config.rows));

        // Create terminal with scrollback from config
        let mut terminal =
            TerminalManager::new_with_scrollback(cols, rows, config.scrollback_lines)?;

        // Apply common terminal configuration
        configure_terminal_from_config(&mut terminal, config);

        // Determine working directory: profile overrides config startup directory
        let effective_startup_dir = config.get_effective_startup_directory();
        let work_dir = profile
            .working_directory
            .as_deref()
            .or(effective_startup_dir.as_deref());

        // Determine command and args with priority:
        // 0. profile.ssh_host → build ssh command with user/port/identity args
        // 1. profile.command → use as-is (non-shell commands like tmux, ssh)
        // 2. profile.shell → use as shell, apply login_shell logic
        // 3. neither → fall back to global config shell / $SHELL
        let is_ssh_profile = profile.ssh_host.is_some();
        let (shell_cmd, mut shell_args) = if let Some(ssh_args) = profile.ssh_command_args() {
            ("ssh".to_string(), Some(ssh_args))
        } else if let Some(ref cmd) = profile.command {
            (cmd.clone(), profile.command_args.clone())
        } else if let Some(ref shell) = profile.shell {
            (shell.clone(), None)
        } else {
            get_shell_command(config)
        };

        // Apply login shell flag when using a shell (not a custom command or SSH profile).
        // Per-profile login_shell overrides global config.login_shell.
        if profile.command.is_none() && !is_ssh_profile {
            let use_login_shell = profile.login_shell.unwrap_or(config.login_shell);
            if use_login_shell {
                let args = shell_args.get_or_insert_with(Vec::new);
                #[cfg(not(target_os = "windows"))]
                if !args.iter().any(|a| a == "-l" || a == "--login") {
                    args.insert(0, "-l".to_string());
                }
            }
        }

        let shell_args_deref = shell_args.as_deref();
        let mut shell_env = build_shell_env(config.shell_env.as_ref());

        // When a profile specifies a shell, set the SHELL env var so child
        // processes (and $SHELL) reflect the selected shell, not the login shell.
        if profile.command.is_none()
            && let Some(ref shell_path) = profile.shell
            && let Some(ref mut env) = shell_env
        {
            env.insert("SHELL".to_string(), shell_path.clone());
        }

        terminal.spawn_custom_shell_with_dir(
            &shell_cmd,
            shell_args_deref,
            work_dir,
            shell_env.as_ref(),
        )?;

        // Sync triggers from config into the core TriggerRegistry
        let trigger_security = terminal.sync_triggers(&config.triggers);

        // Auto-start configured coprocesses via the PtySession's built-in manager
        let mut coprocess_ids = Vec::with_capacity(config.coprocesses.len());
        for coproc_config in &config.coprocesses {
            if coproc_config.auto_start {
                let core_config = par_term_emu_core_rust::coprocess::CoprocessConfig {
                    command: coproc_config.command.clone(),
                    args: coproc_config.args.clone(),
                    cwd: None,
                    env: crate::terminal::coprocess_env(),
                    copy_terminal_output: coproc_config.copy_terminal_output,
                    restart_policy: coproc_config.restart_policy.to_core(),
                    restart_delay_ms: coproc_config.restart_delay_ms,
                };
                match terminal.start_coprocess(core_config) {
                    Ok(id) => {
                        log::info!(
                            "Auto-started coprocess '{}' (id={})",
                            coproc_config.name,
                            id
                        );
                        coprocess_ids.push(Some(id));
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to auto-start coprocess '{}': {}",
                            coproc_config.name,
                            e
                        );
                        coprocess_ids.push(None);
                    }
                }
            } else {
                coprocess_ids.push(None);
            }
        }

        // Create shared session logger
        let session_logger = create_shared_logger();

        // Set up session logging if enabled
        if config.auto_log_sessions {
            let logs_dir = config.logs_dir();
            let session_title = Some(format!(
                "{} - {}",
                profile.name,
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            ));

            match SessionLogger::new(
                config.session_log_format,
                &logs_dir,
                (config.cols, config.rows),
                session_title,
            ) {
                Ok(mut logger) => {
                    logger.set_redact_passwords(config.session_log_redact_passwords);
                    if let Err(e) = logger.start() {
                        log::warn!("Failed to start session logging for profile: {}", e);
                    } else {
                        log::info!(
                            "Session logging started for profile '{}': {:?}",
                            profile.name,
                            logger.output_path()
                        );

                        // Set up output callback to record PTY output
                        let logger_clone = Arc::clone(&session_logger);
                        terminal.set_output_callback(move |data: &[u8]| {
                            if let Some(ref mut logger) = *logger_clone.lock() {
                                logger.record_output(data);
                            }
                        });

                        *session_logger.lock() = Some(logger);
                    }
                }
                Err(e) => {
                    log::warn!("Failed to create session logger for profile: {}", e);
                }
            }
        }

        let terminal = Arc::new(Mutex::new(terminal));

        // Generate title: use profile tab_name or profile name or default
        let title = profile
            .tab_name
            .clone()
            .unwrap_or_else(|| profile.name.clone());

        let working_directory = profile
            .working_directory
            .clone()
            .or_else(|| config.working_directory.clone());

        Ok(Self {
            id,
            terminal,
            pane_manager: None, // Created on first split
            title,
            has_activity: false,
            scroll_state: ScrollState::new(),
            mouse: MouseState::new(),
            bell: BellState::new(),
            cache: RenderCache::new(),
            refresh_task: None,
            working_directory,
            custom_color: None,
            has_default_title: false, // Profile-created tabs have explicit names
            user_named: profile.tab_name.is_some(),
            last_activity_time: std::time::Instant::now(),
            last_seen_generation: 0,
            anti_idle_last_activity: std::time::Instant::now(),
            anti_idle_last_generation: 0,
            silence_notified: false,
            exit_notified: false,
            session_logger,
            tmux_gateway_active: false,
            tmux_pane_id: None,
            detected_hostname: None,
            detected_cwd: None,
            auto_applied_profile_id: None,
            auto_applied_dir_profile_id: None,
            profile_icon: None,
            custom_icon: None,
            pre_profile_title: None,
            badge_override: None,
            coprocess_ids,
            script_manager: crate::scripting::manager::ScriptManager::new(),
            script_ids: Vec::new(),
            script_observer_ids: Vec::new(),
            script_forwarders: Vec::new(),
            trigger_marks: Vec::new(),
            trigger_security,
            trigger_rate_limiter: par_term_config::TriggerRateLimiter::default(),
            prettifier: crate::prettifier::config_bridge::create_pipeline_from_config(
                config, cols, None,
            ),
            gutter_manager: GutterManager::new(),
            was_alt_screen: false,
            pre_ssh_switch_profile: None,
            ssh_auto_switched: false,
            is_active: Arc::new(AtomicBool::new(false)),
            shutdown_fast: false,
            pending_tmux_mode_disable: false,
        })
    }
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

        self.stop_refresh_task();

        // Give the task time to abort
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Kill the terminal
        if let Ok(mut term) = self.terminal.try_lock()
            && term.is_running()
        {
            log::info!("Killing terminal for tab {}", self.id);
            let _ = term.kill();
        }
    }
}

impl Tab {
    /// Create a minimal stub tab for unit testing (no PTY, no runtime)
    #[cfg(test)]
    pub(crate) fn new_stub(id: TabId, tab_number: usize) -> Self {
        // Create a dummy TerminalManager without spawning a shell
        let terminal =
            TerminalManager::new_with_scrollback(80, 24, 100).expect("stub terminal creation");
        Self {
            id,
            terminal: Arc::new(Mutex::new(terminal)),
            pane_manager: None,
            title: format!("Tab {}", tab_number),
            has_activity: false,
            scroll_state: ScrollState::new(),
            mouse: MouseState::new(),
            bell: BellState::new(),
            cache: RenderCache::new(),
            refresh_task: None,
            working_directory: None,
            custom_color: None,
            has_default_title: true,
            user_named: false,
            last_activity_time: std::time::Instant::now(),
            last_seen_generation: 0,
            anti_idle_last_activity: std::time::Instant::now(),
            anti_idle_last_generation: 0,
            silence_notified: false,
            exit_notified: false,
            session_logger: create_shared_logger(),
            tmux_gateway_active: false,
            tmux_pane_id: None,
            detected_hostname: None,
            detected_cwd: None,
            auto_applied_profile_id: None,
            auto_applied_dir_profile_id: None,
            profile_icon: None,
            custom_icon: None,
            pre_profile_title: None,
            badge_override: None,
            coprocess_ids: Vec::new(),
            script_manager: crate::scripting::manager::ScriptManager::new(),
            script_ids: Vec::new(),
            script_observer_ids: Vec::new(),
            script_forwarders: Vec::new(),
            trigger_marks: Vec::new(),
            trigger_security: std::collections::HashMap::new(),
            trigger_rate_limiter: par_term_config::TriggerRateLimiter::default(),
            prettifier: None,
            gutter_manager: GutterManager::new(),
            was_alt_screen: false,
            pre_ssh_switch_profile: None,
            ssh_auto_switched: false,
            is_active: Arc::new(AtomicBool::new(false)),
            shutdown_fast: false,
            pending_tmux_mode_disable: false,
        }
    }
}

//! Tab management for multi-tab terminal support
//!
//! This module provides the core tab infrastructure including:
//! - `Tab`: Represents a single terminal session with its own state (supports split panes)
//! - `TabManager`: Coordinates multiple tabs within a window
//! - `TabId`: Unique identifier for each tab

mod initial_text;
mod manager;

pub use manager::TabManager;

use crate::app::bell::BellState;
use crate::app::mouse::MouseState;
use crate::app::render_cache::RenderCache;
use crate::config::Config;
use crate::pane::{NavigationDirection, PaneManager, SplitDirection};
use crate::profile::Profile;
use crate::scroll_state::ScrollState;
use crate::session_logger::{SessionLogger, SharedSessionLogger, create_shared_logger};
use crate::tab::initial_text::build_initial_text_payload;
use crate::terminal::TerminalManager;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// Configure a terminal with settings from config (theme, clipboard limits, cursor style, unicode)
fn configure_terminal_from_config(terminal: &mut TerminalManager, config: &Config) {
    // Set theme from config
    terminal.set_theme(config.load_theme());

    // Apply clipboard history limits from config
    terminal.set_max_clipboard_sync_events(config.clipboard_max_sync_events);
    terminal.set_max_clipboard_event_bytes(config.clipboard_max_event_bytes);

    // Set answerback string for ENQ response (if configured)
    if !config.answerback_string.is_empty() {
        terminal.set_answerback_string(Some(config.answerback_string.clone()));
    }

    // Apply Unicode width configuration
    let width_config =
        par_term_emu_core_rust::WidthConfig::new(config.unicode_version, config.ambiguous_width);
    terminal.set_width_config(width_config);

    // Initialize cursor style from config
    use crate::config::CursorStyle as ConfigCursorStyle;
    use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;
    let term_style = if config.cursor_blink {
        match config.cursor_style {
            ConfigCursorStyle::Block => TermCursorStyle::BlinkingBlock,
            ConfigCursorStyle::Underline => TermCursorStyle::BlinkingUnderline,
            ConfigCursorStyle::Beam => TermCursorStyle::BlinkingBar,
        }
    } else {
        match config.cursor_style {
            ConfigCursorStyle::Block => TermCursorStyle::SteadyBlock,
            ConfigCursorStyle::Underline => TermCursorStyle::SteadyUnderline,
            ConfigCursorStyle::Beam => TermCursorStyle::SteadyBar,
        }
    };
    terminal.set_cursor_style(term_style);
}

/// Get the platform-specific PATH separator
#[cfg(target_os = "windows")]
const PATH_SEPARATOR: char = ';';
#[cfg(not(target_os = "windows"))]
const PATH_SEPARATOR: char = ':';

/// Build environment variables with an augmented PATH
///
/// When launched from Finder on macOS (or similar on other platforms), the PATH may be minimal.
/// This function augments the PATH with common directories where user tools are installed.
fn build_shell_env(
    config_env: Option<&std::collections::HashMap<String, String>>,
) -> Option<std::collections::HashMap<String, String>> {
    // Get the current PATH
    let current_path = std::env::var("PATH").unwrap_or_default();

    // Build platform-specific extra paths
    let extra_paths = build_platform_extra_paths();

    // Filter to paths that exist and aren't already in PATH
    let new_paths: Vec<String> = extra_paths
        .into_iter()
        .filter(|p| !p.is_empty() && !current_path.contains(p) && std::path::Path::new(p).exists())
        .collect();

    // If nothing to add and no config env, return the config env as-is
    if new_paths.is_empty() && config_env.is_none() {
        return None;
    }

    // Build the augmented PATH using platform-specific separator
    let augmented_path = if new_paths.is_empty() {
        current_path
    } else {
        format!(
            "{}{}{}",
            new_paths.join(&PATH_SEPARATOR.to_string()),
            PATH_SEPARATOR,
            current_path
        )
    };

    // Start with config env or empty map
    let mut env = config_env.cloned().unwrap_or_default();
    env.insert("PATH".to_string(), augmented_path);

    Some(env)
}

/// Build the list of extra PATH directories for the current platform
#[cfg(target_os = "windows")]
fn build_platform_extra_paths() -> Vec<String> {
    let mut paths = Vec::new();

    if let Some(home) = dirs::home_dir() {
        // Cargo bin
        paths.push(
            home.join(".cargo")
                .join("bin")
                .to_string_lossy()
                .to_string(),
        );
        // Scoop
        paths.push(
            home.join("scoop")
                .join("shims")
                .to_string_lossy()
                .to_string(),
        );
        // Go bin
        paths.push(home.join("go").join("bin").to_string_lossy().to_string());
    }

    // Chocolatey
    paths.push(r"C:\ProgramData\chocolatey\bin".to_string());

    // Common program locations
    if let Some(local_app_data) = dirs::data_local_dir() {
        // Python (common location)
        paths.push(
            local_app_data
                .join("Programs")
                .join("Python")
                .join("Python312")
                .join("Scripts")
                .to_string_lossy()
                .to_string(),
        );
        paths.push(
            local_app_data
                .join("Programs")
                .join("Python")
                .join("Python311")
                .join("Scripts")
                .to_string_lossy()
                .to_string(),
        );
    }

    paths
}

/// Build the list of extra PATH directories for Unix platforms (macOS/Linux)
#[cfg(not(target_os = "windows"))]
fn build_platform_extra_paths() -> Vec<String> {
    let mut paths = Vec::new();

    if let Some(home) = dirs::home_dir() {
        // User's home .local/bin (common for pip, pipx, etc.)
        paths.push(
            home.join(".local")
                .join("bin")
                .to_string_lossy()
                .to_string(),
        );
        // Cargo bin
        paths.push(
            home.join(".cargo")
                .join("bin")
                .to_string_lossy()
                .to_string(),
        );
        // Go bin
        paths.push(home.join("go").join("bin").to_string_lossy().to_string());
        // Nix user profile
        paths.push(
            home.join(".nix-profile")
                .join("bin")
                .to_string_lossy()
                .to_string(),
        );
    }

    // Nix system profile
    paths.push("/nix/var/nix/profiles/default/bin".to_string());

    // macOS-specific paths
    #[cfg(target_os = "macos")]
    {
        // Homebrew on Apple Silicon
        paths.push("/opt/homebrew/bin".to_string());
        paths.push("/opt/homebrew/sbin".to_string());
        // Homebrew on Intel Mac
        paths.push("/usr/local/bin".to_string());
        paths.push("/usr/local/sbin".to_string());
        // MacPorts
        paths.push("/opt/local/bin".to_string());
    }

    // Linux-specific paths
    #[cfg(target_os = "linux")]
    {
        // Common system paths that might be missing
        paths.push("/usr/local/bin".to_string());
        // Snap
        paths.push("/snap/bin".to_string());
        // Flatpak exports
        if let Some(home) = dirs::home_dir() {
            paths.push(
                home.join(".local")
                    .join("share")
                    .join("flatpak")
                    .join("exports")
                    .join("bin")
                    .to_string_lossy()
                    .to_string(),
            );
        }
        paths.push("/var/lib/flatpak/exports/bin".to_string());
    }

    paths
}

/// Determine the shell command and arguments to use based on config
fn get_shell_command(config: &Config) -> (String, Option<Vec<String>>) {
    if let Some(ref custom) = config.custom_shell {
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
    }
}

/// Apply login shell flag if configured (Unix only)
#[cfg(not(target_os = "windows"))]
fn apply_login_shell_flag(shell_args: &mut Option<Vec<String>>, config: &Config) {
    if config.login_shell {
        let args = shell_args.get_or_insert_with(Vec::new);
        if !args.iter().any(|a| a == "-l" || a == "--login") {
            args.insert(0, "-l".to_string());
        }
    }
}

#[cfg(target_os = "windows")]
fn apply_login_shell_flag(_shell_args: &mut Option<Vec<String>>, _config: &Config) {
    // No-op on Windows
}

/// Unique identifier for a tab
pub type TabId = u64;

/// A single terminal tab with its own state (supports split panes)
pub struct Tab {
    /// Unique identifier for this tab
    pub id: TabId,
    /// The terminal session for this tab (legacy - use pane_manager for new code)
    pub terminal: Arc<Mutex<TerminalManager>>,
    /// Pane manager for split pane support
    pub pane_manager: Option<PaneManager>,
    /// Tab title (from OSC sequences or fallback)
    pub title: String,
    /// Whether this tab has unread activity since last viewed
    pub has_activity: bool,
    /// Scroll state for this tab (legacy - each pane has its own)
    pub scroll_state: ScrollState,
    /// Mouse state for this tab (legacy - each pane has its own)
    pub mouse: MouseState,
    /// Bell state for this tab (legacy - each pane has its own)
    pub bell: BellState,
    /// Render cache for this tab (legacy - each pane has its own)
    pub cache: RenderCache,
    /// Async task for refresh polling
    pub refresh_task: Option<JoinHandle<()>>,
    /// Working directory when tab was created (for inheriting)
    pub working_directory: Option<String>,
    /// Custom tab color [R, G, B] (0-255), overrides config colors when set
    pub custom_color: Option<[u8; 3]>,
    /// Whether the tab has its default "Tab N" title (not set by OSC, CWD, or user)
    pub has_default_title: bool,
    /// Last time terminal output (activity) was detected
    pub last_activity_time: std::time::Instant,
    /// Last terminal update generation seen (to detect new output)
    pub last_seen_generation: u64,
    /// Last activity time for anti-idle keep-alive
    pub anti_idle_last_activity: std::time::Instant,
    /// Last terminal generation recorded for anti-idle tracking
    pub anti_idle_last_generation: u64,
    /// Whether silence notification has been sent for current idle period
    pub silence_notified: bool,
    /// Whether exit notification has been sent for this tab
    pub exit_notified: bool,
    /// Session logger for automatic session recording
    pub session_logger: SharedSessionLogger,
    /// Whether this tab is in tmux gateway mode
    pub tmux_gateway_active: bool,
    /// The tmux pane ID this tab represents (when in gateway mode)
    pub tmux_pane_id: Option<crate::tmux::TmuxPaneId>,
}

impl Tab {
    /// Create a new tab with a terminal session
    pub fn new(
        id: TabId,
        tab_number: usize,
        config: &Config,
        runtime: Arc<Runtime>,
        working_directory: Option<String>,
    ) -> anyhow::Result<Self> {
        // Create terminal with scrollback from config
        let mut terminal = TerminalManager::new_with_scrollback(
            config.cols,
            config.rows,
            config.scrollback_lines,
        )?;

        // Apply common terminal configuration
        configure_terminal_from_config(&mut terminal, config);

        // Determine working directory
        let work_dir = working_directory
            .as_deref()
            .or(config.working_directory.as_deref());

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
            last_activity_time: std::time::Instant::now(),
            last_seen_generation: 0,
            anti_idle_last_activity: std::time::Instant::now(),
            anti_idle_last_generation: 0,
            silence_notified: false,
            exit_notified: false,
            session_logger,
            tmux_gateway_active: false,
            tmux_pane_id: None,
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
    pub fn new_from_profile(
        id: TabId,
        config: &Config,
        _runtime: Arc<Runtime>,
        profile: &Profile,
    ) -> anyhow::Result<Self> {
        // Create terminal with scrollback from config
        let mut terminal = TerminalManager::new_with_scrollback(
            config.cols,
            config.rows,
            config.scrollback_lines,
        )?;

        // Apply common terminal configuration
        configure_terminal_from_config(&mut terminal, config);

        // Determine working directory: profile overrides config
        let work_dir = profile
            .working_directory
            .as_deref()
            .or(config.working_directory.as_deref());

        // Determine command and args: profile command overrides config shell
        let (shell_cmd, mut shell_args) = if let Some(ref cmd) = profile.command {
            (cmd.clone(), profile.command_args.clone())
        } else {
            get_shell_command(config)
        };

        // Only apply login shell flag for default shell, not custom profile commands
        if profile.command.is_none() {
            apply_login_shell_flag(&mut shell_args, config);
        }

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
            last_activity_time: std::time::Instant::now(),
            last_seen_generation: 0,
            anti_idle_last_activity: std::time::Instant::now(),
            anti_idle_last_generation: 0,
            silence_notified: false,
            exit_notified: false,
            session_logger,
            tmux_gateway_active: false,
            tmux_pane_id: None,
        })
    }

    /// Check if the visual bell is currently active (within flash duration)
    pub fn is_bell_active(&self) -> bool {
        const FLASH_DURATION_MS: u128 = 150;
        if let Some(flash_start) = self.bell.visual_flash {
            flash_start.elapsed().as_millis() < FLASH_DURATION_MS
        } else {
            false
        }
    }

    /// Update tab title from terminal OSC sequences
    pub fn update_title(&mut self) {
        if let Ok(term) = self.terminal.try_lock() {
            let osc_title = term.get_title();
            if !osc_title.is_empty() {
                self.title = osc_title;
                self.has_default_title = false;
            } else if let Some(cwd) = term.shell_integration_cwd() {
                // Abbreviate home directory to ~
                let abbreviated = if let Some(home) = dirs::home_dir() {
                    cwd.replace(&home.to_string_lossy().to_string(), "~")
                } else {
                    cwd
                };
                // Use just the last component for brevity
                if let Some(last) = abbreviated.rsplit('/').next() {
                    if !last.is_empty() {
                        self.title = last.to_string();
                    } else {
                        self.title = abbreviated;
                    }
                } else {
                    self.title = abbreviated;
                }
                self.has_default_title = false;
            }
            // Otherwise keep the existing title (e.g., "Tab N")
        }
    }

    /// Set the tab's default title based on its position
    pub fn set_default_title(&mut self, tab_number: usize) {
        if self.has_default_title {
            self.title = format!("Tab {}", tab_number);
        }
    }

    /// Explicitly set the tab title (for tmux window names, etc.)
    ///
    /// This overrides any default title and marks the tab as having a custom title.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
        self.has_default_title = false;
    }

    /// Check if the terminal in this tab is still running
    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        if let Ok(term) = self.terminal.try_lock() {
            term.is_running()
        } else {
            true // Assume running if locked
        }
    }

    /// Get the current working directory of this tab's shell
    pub fn get_cwd(&self) -> Option<String> {
        if let Ok(term) = self.terminal.try_lock() {
            term.shell_integration_cwd()
        } else {
            self.working_directory.clone()
        }
    }

    /// Start the refresh polling task for this tab
    pub fn start_refresh_task(
        &mut self,
        runtime: Arc<Runtime>,
        window: Arc<winit::window::Window>,
        max_fps: u32,
    ) {
        let terminal_clone = Arc::clone(&self.terminal);
        let refresh_interval_ms = 1000 / max_fps.max(1);

        let handle = runtime.spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(
                refresh_interval_ms as u64,
            ));
            let mut last_gen = 0;

            loop {
                interval.tick().await;

                let should_redraw = if let Ok(term) = terminal_clone.try_lock() {
                    let current_gen = term.update_generation();
                    if current_gen > last_gen {
                        last_gen = current_gen;
                        true
                    } else {
                        term.has_updates()
                    }
                } else {
                    false
                };

                if should_redraw {
                    window.request_redraw();
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

    /// Set a custom color for this tab
    pub fn set_custom_color(&mut self, color: [u8; 3]) {
        self.custom_color = Some(color);
    }

    /// Clear the custom color for this tab (reverts to default config colors)
    pub fn clear_custom_color(&mut self) {
        self.custom_color = None;
    }

    /// Check if this tab has a custom color set
    #[allow(dead_code)]
    pub fn has_custom_color(&self) -> bool {
        self.custom_color.is_some()
    }

    /// Toggle session logging on/off.
    ///
    /// Returns `Ok(true)` if logging is now active, `Ok(false)` if stopped.
    /// If logging wasn't active and no logger exists, creates a new one.
    pub fn toggle_session_logging(&mut self, config: &Config) -> anyhow::Result<bool> {
        let mut logger_guard = self.session_logger.lock();

        if let Some(ref mut logger) = *logger_guard {
            // Logger exists - toggle based on current state
            if logger.is_active() {
                logger.stop()?;
                log::info!("Session logging stopped via hotkey");
                Ok(false)
            } else {
                logger.start()?;
                log::info!("Session logging started via hotkey");
                Ok(true)
            }
        } else {
            // No logger exists - create one and start it
            let logs_dir = config.logs_dir();
            if let Err(e) = std::fs::create_dir_all(&logs_dir) {
                log::warn!("Failed to create logs directory: {}", e);
                return Err(anyhow::anyhow!("Failed to create logs directory: {}", e));
            }

            // Get terminal dimensions
            let dimensions = if let Ok(term) = self.terminal.try_lock() {
                term.dimensions()
            } else {
                (80, 24) // fallback
            };

            let session_title = Some(format!(
                "{} - {}",
                self.title,
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
            ));

            let mut logger = SessionLogger::new(
                config.session_log_format,
                &logs_dir,
                dimensions,
                session_title,
            )?;

            logger.start()?;

            // Set up output callback to record PTY output
            let logger_clone = Arc::clone(&self.session_logger);
            if let Ok(term) = self.terminal.try_lock() {
                term.set_output_callback(move |data: &[u8]| {
                    if let Some(ref mut logger) = *logger_clone.lock() {
                        logger.record_output(data);
                    }
                });
            }

            *logger_guard = Some(logger);
            log::info!("Session logging created and started via hotkey");
            Ok(true)
        }
    }

    /// Check if session logging is currently active.
    pub fn is_session_logging_active(&self) -> bool {
        if let Some(ref logger) = *self.session_logger.lock() {
            logger.is_active()
        } else {
            false
        }
    }

    // ========================================================================
    // Split Pane Support
    // ========================================================================

    /// Check if this tab has multiple panes (split)
    pub fn has_multiple_panes(&self) -> bool {
        self.pane_manager
            .as_ref()
            .is_some_and(|pm| pm.has_multiple_panes())
    }

    /// Get the number of panes in this tab
    pub fn pane_count(&self) -> usize {
        self.pane_manager
            .as_ref()
            .map(|pm| pm.pane_count())
            .unwrap_or(1)
    }

    /// Split the current pane horizontally (panes stacked vertically)
    ///
    /// Returns the new pane ID if successful
    pub fn split_horizontal(
        &mut self,
        config: &Config,
        runtime: Arc<Runtime>,
    ) -> anyhow::Result<Option<crate::pane::PaneId>> {
        self.split(SplitDirection::Horizontal, config, runtime)
    }

    /// Split the current pane vertically (panes side by side)
    ///
    /// Returns the new pane ID if successful
    pub fn split_vertical(
        &mut self,
        config: &Config,
        runtime: Arc<Runtime>,
    ) -> anyhow::Result<Option<crate::pane::PaneId>> {
        self.split(SplitDirection::Vertical, config, runtime)
    }

    /// Split the focused pane in the given direction
    fn split(
        &mut self,
        direction: SplitDirection,
        config: &Config,
        runtime: Arc<Runtime>,
    ) -> anyhow::Result<Option<crate::pane::PaneId>> {
        // Check max panes limit
        if config.max_panes > 0 && self.pane_count() >= config.max_panes {
            log::warn!(
                "Cannot split: max panes limit ({}) reached",
                config.max_panes
            );
            return Ok(None);
        }

        // Initialize pane manager and create initial pane if needed
        let needs_initial_pane = self
            .pane_manager
            .as_ref()
            .map(|pm| pm.pane_count() == 0)
            .unwrap_or(true);

        if needs_initial_pane {
            // Create pane manager if it doesn't exist
            if self.pane_manager.is_none() {
                let mut pm = PaneManager::new();
                pm.set_divider_width(config.pane_divider_width.unwrap_or(2.0));
                self.pane_manager = Some(pm);
            }

            // Create initial pane with size calculated for AFTER the split
            // (since we know a split is about to happen)
            if let Some(ref mut pm) = self.pane_manager {
                pm.create_initial_pane_for_split(
                    direction,
                    config,
                    Arc::clone(&runtime),
                    self.working_directory.clone(),
                )?;
                log::info!(
                    "Created PaneManager for tab {} with initial pane on first split",
                    self.id
                );
            }
        }

        // Perform the split
        if let Some(ref mut pm) = self.pane_manager {
            let new_pane_id = pm.split(direction, config, Arc::clone(&runtime))?;
            if let Some(id) = new_pane_id {
                log::info!("Split tab {} {:?}, new pane {}", self.id, direction, id);
            }
            Ok(new_pane_id)
        } else {
            Ok(None)
        }
    }

    /// Close the focused pane
    ///
    /// Returns true if this was the last pane (tab should close)
    pub fn close_focused_pane(&mut self) -> bool {
        if let Some(ref mut pm) = self.pane_manager
            && let Some(focused_id) = pm.focused_pane_id()
        {
            let is_last = pm.close_pane(focused_id);
            if is_last {
                // Last pane closed, clear the pane manager
                self.pane_manager = None;
            }
            return is_last;
        }
        // No pane manager or no focused pane means single pane tab
        true
    }

    /// Check for exited panes and close them
    ///
    /// Returns (closed_pane_ids, tab_should_close) where:
    /// - `closed_pane_ids`: Vec of pane IDs that were closed
    /// - `tab_should_close`: true if all panes have exited (tab should close)
    pub fn close_exited_panes(&mut self) -> (Vec<crate::pane::PaneId>, bool) {
        let mut closed_panes = Vec::new();

        // Get IDs of panes whose shells have exited
        let exited_pane_ids: Vec<crate::pane::PaneId> = if let Some(ref pm) = self.pane_manager {
            let focused_id = pm.focused_pane_id();
            pm.all_panes()
                .iter()
                .filter_map(|pane| {
                    let is_running = pane.is_running();
                    crate::debug_info!(
                        "PANE_CHECK",
                        "Pane {} running={} focused={} bounds=({:.0},{:.0} {:.0}x{:.0})",
                        pane.id,
                        is_running,
                        focused_id == Some(pane.id),
                        pane.bounds.x,
                        pane.bounds.y,
                        pane.bounds.width,
                        pane.bounds.height
                    );
                    if !is_running {
                        Some(pane.id)
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        // Close each exited pane
        if let Some(ref mut pm) = self.pane_manager {
            for pane_id in exited_pane_ids {
                crate::debug_info!("PANE_CLOSE", "Closing pane {} - shell exited", pane_id);
                let is_last = pm.close_pane(pane_id);
                closed_panes.push(pane_id);

                if is_last {
                    // Last pane closed, clear the pane manager
                    self.pane_manager = None;
                    return (closed_panes, true);
                }
            }
        }

        (closed_panes, false)
    }

    /// Get the pane manager if split panes are enabled
    pub fn pane_manager(&self) -> Option<&PaneManager> {
        self.pane_manager.as_ref()
    }

    /// Get mutable access to the pane manager
    pub fn pane_manager_mut(&mut self) -> Option<&mut PaneManager> {
        self.pane_manager.as_mut()
    }

    /// Initialize the pane manager if not already present
    ///
    /// This is used for tmux integration where we need to create the pane manager
    /// before applying a layout.
    pub fn init_pane_manager(&mut self) {
        if self.pane_manager.is_none() {
            self.pane_manager = Some(PaneManager::new());
        }
    }

    /// Set the pane bounds and resize terminals
    ///
    /// This should be called before creating splits to ensure panes are sized correctly.
    /// If the pane manager doesn't exist yet, this creates it with the bounds set.
    pub fn set_pane_bounds(
        &mut self,
        bounds: crate::pane::PaneBounds,
        cell_width: f32,
        cell_height: f32,
    ) {
        self.set_pane_bounds_with_padding(bounds, cell_width, cell_height, 0.0);
    }

    /// Set the pane bounds and resize terminals with padding
    ///
    /// This should be called before creating splits to ensure panes are sized correctly.
    /// The padding parameter accounts for content inset from pane edges.
    pub fn set_pane_bounds_with_padding(
        &mut self,
        bounds: crate::pane::PaneBounds,
        cell_width: f32,
        cell_height: f32,
        padding: f32,
    ) {
        if self.pane_manager.is_none() {
            let mut pm = PaneManager::new();
            pm.set_bounds(bounds);
            self.pane_manager = Some(pm);
        } else if let Some(ref mut pm) = self.pane_manager {
            pm.set_bounds(bounds);
            pm.resize_all_terminals_with_padding(cell_width, cell_height, padding);
        }
    }

    /// Focus the pane at the given pixel coordinates
    ///
    /// Returns the ID of the newly focused pane, or None if no pane at that position
    pub fn focus_pane_at(&mut self, x: f32, y: f32) -> Option<crate::pane::PaneId> {
        if let Some(ref mut pm) = self.pane_manager {
            pm.focus_pane_at(x, y)
        } else {
            None
        }
    }

    /// Get the ID of the currently focused pane
    pub fn focused_pane_id(&self) -> Option<crate::pane::PaneId> {
        self.pane_manager
            .as_ref()
            .and_then(|pm| pm.focused_pane_id())
    }

    /// Check if a specific pane is focused
    pub fn is_pane_focused(&self, pane_id: crate::pane::PaneId) -> bool {
        self.focused_pane_id() == Some(pane_id)
    }

    /// Navigate to an adjacent pane
    pub fn navigate_pane(&mut self, direction: NavigationDirection) {
        if let Some(ref mut pm) = self.pane_manager {
            pm.navigate(direction);
        }
    }

    /// Check if a position is on a divider
    pub fn is_on_divider(&self, x: f32, y: f32) -> bool {
        self.pane_manager
            .as_ref()
            .is_some_and(|pm| pm.is_on_divider(x, y))
    }

    /// Find divider at position
    ///
    /// Returns the divider index if found
    pub fn find_divider_at(&self, x: f32, y: f32) -> Option<usize> {
        self.pane_manager
            .as_ref()
            .and_then(|pm| pm.find_divider_at(x, y, 3.0)) // 3px padding for easier grabbing
    }

    /// Get divider info by index
    pub fn get_divider(&self, index: usize) -> Option<crate::pane::DividerRect> {
        self.pane_manager
            .as_ref()
            .and_then(|pm| pm.get_divider(index))
    }

    /// Drag a divider to a new position
    pub fn drag_divider(&mut self, divider_index: usize, x: f32, y: f32) {
        if let Some(ref mut pm) = self.pane_manager {
            pm.drag_divider(divider_index, x, y);
        }
    }
}

impl Drop for Tab {
    fn drop(&mut self) {
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

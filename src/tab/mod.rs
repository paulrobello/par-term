//! Tab management for multi-tab terminal support
//!
//! This module provides the core tab infrastructure including:
//! - `Tab`: Represents a single terminal session with its own state
//! - `TabManager`: Coordinates multiple tabs within a window
//! - `TabId`: Unique identifier for each tab

mod initial_text;
mod manager;

pub use manager::TabManager;

use crate::app::bell::BellState;
use crate::app::mouse::MouseState;
use crate::app::render_cache::RenderCache;
use crate::config::Config;
use crate::profile::Profile;
use crate::scroll_state::ScrollState;
use crate::tab::initial_text::build_initial_text_payload;
use crate::terminal::TerminalManager;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// Configure a terminal with settings from config (theme, clipboard limits, cursor style)
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

/// A single terminal tab with its own state
pub struct Tab {
    /// Unique identifier for this tab
    pub id: TabId,
    /// The terminal session for this tab
    pub terminal: Arc<Mutex<TerminalManager>>,
    /// Tab title (from OSC sequences or fallback)
    pub title: String,
    /// Whether this tab has unread activity since last viewed
    pub has_activity: bool,
    /// Scroll state for this tab
    pub scroll_state: ScrollState,
    /// Mouse state for this tab
    pub mouse: MouseState,
    /// Bell state for this tab
    pub bell: BellState,
    /// Render cache for this tab
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
        let shell_env = config.shell_env.as_ref();
        terminal.spawn_custom_shell_with_dir(&shell_cmd, shell_args_deref, work_dir, shell_env)?;

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
        let shell_env = config.shell_env.as_ref();
        terminal.spawn_custom_shell_with_dir(&shell_cmd, shell_args_deref, work_dir, shell_env)?;

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
            silence_notified: false,
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
}

impl Drop for Tab {
    fn drop(&mut self) {
        log::info!("Dropping tab {}", self.id);
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

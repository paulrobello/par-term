//! Core types for the pane system
//!
//! This module defines the fundamental data structures for split panes:
//! - Binary tree structure for arbitrary nesting
//! - Per-pane state (terminal, scroll, mouse, etc.)
//! - Bounds calculation for rendering

use crate::app::bell::BellState;
use crate::app::mouse::MouseState;
use crate::app::render_cache::RenderCache;
use crate::config::Config;
use crate::scroll_state::ScrollState;
use crate::session_logger::{SharedSessionLogger, create_shared_logger};
use crate::tab::build_shell_env;
use crate::terminal::TerminalManager;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

// Re-export PaneId from par-term-config for shared access across subcrates
pub use par_term_config::PaneId;

/// State for shell restart behavior
#[derive(Debug, Clone)]
pub enum RestartState {
    /// Waiting for user to press Enter to restart
    AwaitingInput,
    /// Waiting for delay timer before restart
    AwaitingDelay(std::time::Instant),
}

/// Direction of a split
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SplitDirection {
    /// Panes are stacked vertically (split creates top/bottom panes)
    Horizontal,
    /// Panes are side by side (split creates left/right panes)
    Vertical,
}

/// Bounds of a pane in pixels
#[derive(Debug, Clone, Copy, Default)]
pub struct PaneBounds {
    /// X position in pixels from left edge of content area
    pub x: f32,
    /// Y position in pixels from top of content area (below tab bar)
    pub y: f32,
    /// Width in pixels
    pub width: f32,
    /// Height in pixels
    pub height: f32,
}

impl PaneBounds {
    /// Create new bounds
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Check if a point is inside these bounds
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    /// Get the center point of the bounds
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Calculate grid dimensions (cols, rows) given cell dimensions
    pub fn grid_size(&self, cell_width: f32, cell_height: f32) -> (usize, usize) {
        let cols = (self.width / cell_width).floor() as usize;
        let rows = (self.height / cell_height).floor() as usize;
        (cols.max(1), rows.max(1))
    }
}

// Re-export rendering types from par-term-config
pub use par_term_config::{DividerRect, PaneBackground};

/// A single terminal pane with its own state
pub struct Pane {
    /// Unique identifier for this pane
    pub id: PaneId,
    /// The terminal session for this pane
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
        let width_config = par_term_emu_core_rust::WidthConfig::new(
            config.unicode_version,
            config.ambiguous_width,
        );
        terminal.set_width_config(width_config);

        // Apply Unicode normalization form
        terminal.set_normalization_form(config.normalization_form);

        // Initialize cursor style from config
        {
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

        // Determine working directory
        let work_dir = working_directory
            .as_deref()
            .or(config.working_directory.as_deref());

        // Determine the shell command to use
        #[allow(unused_mut)] // mut is needed on Unix for login shell modification
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
        let width_config = par_term_emu_core_rust::WidthConfig::new(
            config.unicode_version,
            config.ambiguous_width,
        );
        terminal.set_width_config(width_config);

        // Apply Unicode normalization form
        terminal.set_normalization_form(config.normalization_form);

        // Initialize cursor style from config
        {
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

/// Tree node for pane layout
///
/// The pane tree is a binary tree where:
/// - Leaf nodes contain actual terminal panes
/// - Split nodes contain two children with a split direction and ratio
pub enum PaneNode {
    /// A leaf node containing a terminal pane
    Leaf(Box<Pane>),
    /// A split containing two child nodes
    Split {
        /// Direction of the split
        direction: SplitDirection,
        /// Split ratio (0.0 to 1.0) - position of divider
        /// For horizontal: ratio is height of first child / total height
        /// For vertical: ratio is width of first child / total width
        ratio: f32,
        /// First child (top for horizontal, left for vertical)
        first: Box<PaneNode>,
        /// Second child (bottom for horizontal, right for vertical)
        second: Box<PaneNode>,
    },
}

impl PaneNode {
    /// Create a new leaf node with a pane
    pub fn leaf(pane: Pane) -> Self {
        PaneNode::Leaf(Box::new(pane))
    }

    /// Create a new split node
    pub fn split(direction: SplitDirection, ratio: f32, first: PaneNode, second: PaneNode) -> Self {
        PaneNode::Split {
            direction,
            ratio: ratio.clamp(0.1, 0.9), // Enforce minimum pane size
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Check if this is a leaf node
    pub fn is_leaf(&self) -> bool {
        matches!(self, PaneNode::Leaf(_))
    }

    /// Get the pane if this is a leaf node
    pub fn as_pane(&self) -> Option<&Pane> {
        match self {
            PaneNode::Leaf(pane) => Some(pane),
            PaneNode::Split { .. } => None,
        }
    }

    /// Get mutable pane if this is a leaf node
    pub fn as_pane_mut(&mut self) -> Option<&mut Pane> {
        match self {
            PaneNode::Leaf(pane) => Some(pane),
            PaneNode::Split { .. } => None,
        }
    }

    /// Find a pane by ID (recursive)
    pub fn find_pane(&self, id: PaneId) -> Option<&Pane> {
        match self {
            PaneNode::Leaf(pane) => {
                if pane.id == id {
                    Some(pane)
                } else {
                    None
                }
            }
            PaneNode::Split { first, second, .. } => {
                first.find_pane(id).or_else(|| second.find_pane(id))
            }
        }
    }

    /// Find a mutable pane by ID (recursive)
    pub fn find_pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        match self {
            PaneNode::Leaf(pane) => {
                if pane.id == id {
                    Some(pane)
                } else {
                    None
                }
            }
            PaneNode::Split { first, second, .. } => first
                .find_pane_mut(id)
                .or_else(move || second.find_pane_mut(id)),
        }
    }

    /// Find the pane at a given pixel position
    pub fn find_pane_at(&self, x: f32, y: f32) -> Option<&Pane> {
        match self {
            PaneNode::Leaf(pane) => {
                if pane.bounds.contains(x, y) {
                    Some(pane)
                } else {
                    None
                }
            }
            PaneNode::Split { first, second, .. } => first
                .find_pane_at(x, y)
                .or_else(|| second.find_pane_at(x, y)),
        }
    }

    /// Get all pane IDs in this subtree
    pub fn all_pane_ids(&self) -> Vec<PaneId> {
        match self {
            PaneNode::Leaf(pane) => vec![pane.id],
            PaneNode::Split { first, second, .. } => {
                let mut ids = first.all_pane_ids();
                ids.extend(second.all_pane_ids());
                ids
            }
        }
    }

    /// Get all panes in this subtree
    pub fn all_panes(&self) -> Vec<&Pane> {
        match self {
            PaneNode::Leaf(pane) => vec![pane],
            PaneNode::Split { first, second, .. } => {
                let mut panes = first.all_panes();
                panes.extend(second.all_panes());
                panes
            }
        }
    }

    /// Get all mutable panes in this subtree
    pub fn all_panes_mut(&mut self) -> Vec<&mut Pane> {
        match self {
            PaneNode::Leaf(pane) => vec![pane],
            PaneNode::Split { first, second, .. } => {
                let mut panes = first.all_panes_mut();
                panes.extend(second.all_panes_mut());
                panes
            }
        }
    }

    /// Count total number of panes
    pub fn pane_count(&self) -> usize {
        match self {
            PaneNode::Leaf(_) => 1,
            PaneNode::Split { first, second, .. } => first.pane_count() + second.pane_count(),
        }
    }

    /// Calculate bounds for all panes given the total available area
    ///
    /// This recursively distributes space according to split ratios
    /// and updates each pane's bounds field.
    pub fn calculate_bounds(&mut self, bounds: PaneBounds, divider_width: f32) {
        match self {
            PaneNode::Leaf(pane) => {
                pane.bounds = bounds;
            }
            PaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let (first_bounds, second_bounds) = match direction {
                    SplitDirection::Horizontal => {
                        // Split vertically (panes stacked top/bottom)
                        let first_height = (bounds.height - divider_width) * *ratio;
                        let second_height = bounds.height - first_height - divider_width;
                        (
                            PaneBounds::new(bounds.x, bounds.y, bounds.width, first_height),
                            PaneBounds::new(
                                bounds.x,
                                bounds.y + first_height + divider_width,
                                bounds.width,
                                second_height,
                            ),
                        )
                    }
                    SplitDirection::Vertical => {
                        // Split horizontally (panes side by side)
                        let first_width = (bounds.width - divider_width) * *ratio;
                        let second_width = bounds.width - first_width - divider_width;
                        (
                            PaneBounds::new(bounds.x, bounds.y, first_width, bounds.height),
                            PaneBounds::new(
                                bounds.x + first_width + divider_width,
                                bounds.y,
                                second_width,
                                bounds.height,
                            ),
                        )
                    }
                };

                first.calculate_bounds(first_bounds, divider_width);
                second.calculate_bounds(second_bounds, divider_width);
            }
        }
    }

    /// Find the closest pane in a given direction from the focused pane
    ///
    /// Returns the pane ID of the closest pane in the specified direction,
    /// or None if there is no pane in that direction.
    pub fn find_pane_in_direction(
        &self,
        from_id: PaneId,
        direction: NavigationDirection,
    ) -> Option<PaneId> {
        // Get the bounds of the source pane
        let from_pane = self.find_pane(from_id)?;
        let from_center = from_pane.bounds.center();

        // Get all other panes
        let all_panes = self.all_panes();

        // Filter panes that are in the correct direction and find the closest
        let mut best: Option<(PaneId, f32)> = None;

        for pane in all_panes {
            if pane.id == from_id {
                continue;
            }

            let pane_center = pane.bounds.center();
            let is_in_direction = match direction {
                NavigationDirection::Left => pane_center.0 < from_center.0,
                NavigationDirection::Right => pane_center.0 > from_center.0,
                NavigationDirection::Up => pane_center.1 < from_center.1,
                NavigationDirection::Down => pane_center.1 > from_center.1,
            };

            if is_in_direction {
                // Calculate distance (Manhattan distance works well for grid-like layouts)
                let dx = (pane_center.0 - from_center.0).abs();
                let dy = (pane_center.1 - from_center.1).abs();

                // Weight the primary direction more heavily
                let distance = match direction {
                    NavigationDirection::Left | NavigationDirection::Right => dx + dy * 2.0,
                    NavigationDirection::Up | NavigationDirection::Down => dy + dx * 2.0,
                };

                if best.is_none_or(|(_, d)| distance < d) {
                    best = Some((pane.id, distance));
                }
            }
        }

        best.map(|(id, _)| id)
    }

    /// Collect all divider rectangles in the pane tree
    ///
    /// Returns a list of DividerRect structures that can be used for:
    /// - Rendering divider lines between panes
    /// - Hit testing for mouse drag resize
    pub fn collect_dividers(&self, bounds: PaneBounds, divider_width: f32) -> Vec<DividerRect> {
        let mut dividers = Vec::new();
        self.collect_dividers_recursive(bounds, divider_width, &mut dividers);
        dividers
    }

    /// Recursive helper for collecting dividers
    fn collect_dividers_recursive(
        &self,
        bounds: PaneBounds,
        divider_width: f32,
        dividers: &mut Vec<DividerRect>,
    ) {
        match self {
            PaneNode::Leaf(_) => {
                // Leaf nodes have no dividers
            }
            PaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                // Calculate divider position and child bounds
                let (first_bounds, divider, second_bounds) = match direction {
                    SplitDirection::Horizontal => {
                        // Horizontal split: panes stacked top/bottom, divider is horizontal line
                        let first_height = (bounds.height - divider_width) * *ratio;
                        let second_height = bounds.height - first_height - divider_width;
                        (
                            PaneBounds::new(bounds.x, bounds.y, bounds.width, first_height),
                            DividerRect::new(
                                bounds.x,
                                bounds.y + first_height,
                                bounds.width,
                                divider_width,
                                true, // is_horizontal
                            ),
                            PaneBounds::new(
                                bounds.x,
                                bounds.y + first_height + divider_width,
                                bounds.width,
                                second_height,
                            ),
                        )
                    }
                    SplitDirection::Vertical => {
                        // Vertical split: panes side by side, divider is vertical line
                        let first_width = (bounds.width - divider_width) * *ratio;
                        let second_width = bounds.width - first_width - divider_width;
                        (
                            PaneBounds::new(bounds.x, bounds.y, first_width, bounds.height),
                            DividerRect::new(
                                bounds.x + first_width,
                                bounds.y,
                                divider_width,
                                bounds.height,
                                false, // is_horizontal (it's vertical)
                            ),
                            PaneBounds::new(
                                bounds.x + first_width + divider_width,
                                bounds.y,
                                second_width,
                                bounds.height,
                            ),
                        )
                    }
                };

                // Add this divider
                dividers.push(divider);

                // Recurse into children
                first.collect_dividers_recursive(first_bounds, divider_width, dividers);
                second.collect_dividers_recursive(second_bounds, divider_width, dividers);
            }
        }
    }
}

/// Direction for pane navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationDirection {
    Left,
    Right,
    Up,
    Down,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pane_bounds_contains() {
        let bounds = PaneBounds::new(10.0, 20.0, 100.0, 50.0);

        // Inside
        assert!(bounds.contains(50.0, 40.0));
        assert!(bounds.contains(10.0, 20.0)); // Top-left corner

        // Outside
        assert!(!bounds.contains(5.0, 40.0)); // Left of bounds
        assert!(!bounds.contains(150.0, 40.0)); // Right of bounds
        assert!(!bounds.contains(50.0, 10.0)); // Above bounds
        assert!(!bounds.contains(50.0, 80.0)); // Below bounds
    }

    #[test]
    fn test_pane_bounds_grid_size() {
        let bounds = PaneBounds::new(0.0, 0.0, 800.0, 600.0);
        let (cols, rows) = bounds.grid_size(10.0, 20.0);
        assert_eq!(cols, 80);
        assert_eq!(rows, 30);
    }

    #[test]
    fn test_split_direction_clone() {
        let dir = SplitDirection::Horizontal;
        let cloned = dir;
        assert_eq!(dir, cloned);
    }
}

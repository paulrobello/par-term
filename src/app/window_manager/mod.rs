//! Multi-window manager for the terminal emulator
//!
//! This module contains `WindowManager`, which coordinates multiple terminal windows,
//! handles the native menu system, and manages shared resources.
//!
//! The implementation is split across sub-modules for clarity:
//! - `update_checker`   — periodic/forced update checks and desktop notifications
//! - `window_lifecycle` — window creation, destruction, positioning, session restore
//! - `menu_actions`     — native menu event dispatch
//! - `settings`         — settings window open/close and live config propagation
//! - `coprocess`        — coprocess start/stop and state sync to settings UI
//! - `scripting`        — script start/stop and state sync to settings UI
//! - `arrangements`     — save/restore/manage window arrangements

mod arrangements;
mod coprocess;
mod menu_actions;
mod scripting;
mod settings;
mod update_checker;
mod window_lifecycle;

use crate::app::window_state::WindowState;
use crate::arrangements::ArrangementManager;
use crate::cli::RuntimeOptions;
use crate::config::Config;
use crate::menu::MenuManager;
use crate::settings_window::SettingsWindow;
use crate::update_checker::{UpdateCheckResult, UpdateChecker};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::runtime::Runtime;
use winit::window::WindowId;

/// Manages multiple terminal windows and shared resources
pub struct WindowManager {
    /// Per-window state indexed by window ID
    pub(crate) windows: HashMap<WindowId, WindowState>,
    /// Native menu manager
    pub(crate) menu: Option<MenuManager>,
    /// Shared configuration (read at startup, each window gets a clone)
    pub(crate) config: Config,
    /// Shared async runtime
    pub(crate) runtime: Arc<Runtime>,
    /// Flag to indicate if app should exit
    pub(crate) should_exit: bool,
    /// Counter for generating unique window IDs during creation
    pending_window_count: usize,
    /// Separate settings window (if open)
    pub(crate) settings_window: Option<SettingsWindow>,
    /// Runtime options from CLI
    pub(crate) runtime_options: RuntimeOptions,
    /// When the app started (for timing-based CLI options)
    pub(crate) start_time: Option<Instant>,
    /// Whether the command has been sent
    pub(crate) command_sent: bool,
    /// Whether the screenshot has been taken
    pub(crate) screenshot_taken: bool,
    /// Update checker for checking GitHub releases
    pub(crate) update_checker: UpdateChecker,
    /// Time of next scheduled update check
    pub(crate) next_update_check: Option<Instant>,
    /// Last update check result (for display in settings)
    pub(crate) last_update_result: Option<UpdateCheckResult>,
    /// Saved window arrangement manager
    pub(crate) arrangement_manager: ArrangementManager,
    /// Whether auto-restore has been attempted this session
    pub(crate) auto_restore_done: bool,
    /// Dynamic profile manager for fetching remote profiles
    pub(crate) dynamic_profile_manager: crate::profile::DynamicProfileManager,
}

impl WindowManager {
    /// Create a new window manager
    pub fn new(config: Config, runtime: Arc<Runtime>, runtime_options: RuntimeOptions) -> Self {
        // Load saved arrangements
        let arrangement_manager = match crate::arrangements::storage::load_arrangements() {
            Ok(manager) => manager,
            Err(e) => {
                log::warn!("Failed to load arrangements: {}", e);
                ArrangementManager::new()
            }
        };

        let mut dynamic_profile_manager = crate::profile::DynamicProfileManager::new();
        if !config.dynamic_profile_sources.is_empty() {
            dynamic_profile_manager.start(&config.dynamic_profile_sources, &runtime);
        }

        Self {
            windows: HashMap::new(),
            menu: None,
            config,
            runtime,
            should_exit: false,
            pending_window_count: 0,
            settings_window: None,
            runtime_options,
            start_time: None,
            command_sent: false,
            screenshot_taken: false,
            update_checker: UpdateChecker::new(env!("CARGO_PKG_VERSION")),
            next_update_check: None,
            last_update_result: None,
            arrangement_manager,
            auto_restore_done: false,
            dynamic_profile_manager,
        }
    }

    /// Get the ID of the currently focused window.
    /// Returns the window with `is_focused == true`, or falls back to the first window if none is focused.
    pub fn get_focused_window_id(&self) -> Option<WindowId> {
        // Find the window that has focus
        for (window_id, window_state) in &self.windows {
            if window_state.is_focused {
                return Some(*window_id);
            }
        }
        // Fallback: return the first window if no window claims focus
        self.windows.keys().next().copied()
    }

    /// Get mutable reference to a window's state
    pub fn get_window_mut(&mut self, window_id: WindowId) -> Option<&mut WindowState> {
        self.windows.get_mut(&window_id)
    }

    /// Get reference to a window's state
    pub fn get_window(&self, window_id: WindowId) -> Option<&WindowState> {
        self.windows.get(&window_id)
    }
}

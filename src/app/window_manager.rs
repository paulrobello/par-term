//! Multi-window manager for the terminal emulator
//!
//! This module contains `WindowManager`, which coordinates multiple terminal windows,
//! handles the native menu system, and manages shared resources.

use crate::app::window_state::WindowState;
use crate::arrangements::{self, ArrangementId, ArrangementManager};
use crate::cli::RuntimeOptions;
use crate::config::{Config, resolve_shader_config};
use crate::menu::{MenuAction, MenuManager};
use crate::settings_window::{SettingsWindow, SettingsWindowAction};
use crate::update_checker::{UpdateCheckResult, UpdateChecker};

/// Convert a main-crate UpdateCheckResult to the settings-ui crate's type.
fn to_settings_update_result(result: &UpdateCheckResult) -> crate::settings_ui::UpdateCheckResult {
    match result {
        UpdateCheckResult::UpToDate => crate::settings_ui::UpdateCheckResult::UpToDate,
        UpdateCheckResult::UpdateAvailable(info) => {
            crate::settings_ui::UpdateCheckResult::UpdateAvailable(
                crate::settings_ui::UpdateCheckInfo {
                    version: info.version.clone(),
                    release_notes: info.release_notes.clone(),
                    release_url: info.release_url.clone(),
                    published_at: info.published_at.clone(),
                },
            )
        }
        UpdateCheckResult::Disabled => crate::settings_ui::UpdateCheckResult::Disabled,
        UpdateCheckResult::Skipped => crate::settings_ui::UpdateCheckResult::Skipped,
        UpdateCheckResult::Error(e) => crate::settings_ui::UpdateCheckResult::Error(e.clone()),
    }
}

/// Extract the available version string from an update result (None if not available).
fn update_available_version(result: &UpdateCheckResult) -> Option<String> {
    match result {
        UpdateCheckResult::UpdateAvailable(info) => Some(
            info.version
                .strip_prefix('v')
                .unwrap_or(&info.version)
                .to_string(),
        ),
        _ => None,
    }
}
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::runtime::Runtime;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
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
        let arrangement_manager = match arrangements::storage::load_arrangements() {
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
        // This can happen briefly during window creation/destruction
        self.windows.keys().next().copied()
    }

    /// Check and handle timing-based CLI options (exit-after, screenshot, command)
    pub fn check_cli_timers(&mut self) {
        let Some(start_time) = self.start_time else {
            return;
        };

        let elapsed = start_time.elapsed().as_secs_f64();

        // Send command after 1 second delay
        if !self.command_sent
            && elapsed >= 1.0
            && let Some(cmd) = self.runtime_options.command_to_send.clone()
        {
            self.send_command_to_shell(&cmd);
            self.command_sent = true;
        }

        // Take screenshot if requested (after exit_after - 1 second, or after 2 seconds if no exit_after)
        if !self.screenshot_taken && self.runtime_options.screenshot.is_some() {
            let screenshot_time = self
                .runtime_options
                .exit_after
                .map(|t| t - 1.0)
                .unwrap_or(2.0);
            if elapsed >= screenshot_time {
                self.take_screenshot();
                self.screenshot_taken = true;
            }
        }

        // Exit after specified time
        if let Some(exit_after) = self.runtime_options.exit_after
            && elapsed >= exit_after
        {
            log::info!("Exit-after timer expired ({:.1}s), exiting", exit_after);
            self.should_exit = true;
        }
    }

    /// Check for updates (called periodically from about_to_wait)
    pub fn check_for_updates(&mut self) {
        use crate::update_checker::current_timestamp;
        use std::time::Duration;

        let now = Instant::now();

        // Schedule initial check shortly after startup (5 seconds delay)
        if self.next_update_check.is_none() {
            self.next_update_check = Some(now + Duration::from_secs(5));
            return;
        }

        // Check if it's time for scheduled check
        if let Some(next_check) = self.next_update_check
            && now >= next_check
        {
            // Perform the check
            let (result, should_save) = self.update_checker.check_now(&self.config, false);

            // Log the result and notify if appropriate
            let mut config_changed = should_save;
            match &result {
                UpdateCheckResult::UpdateAvailable(info) => {
                    let version_str = info
                        .version
                        .strip_prefix('v')
                        .unwrap_or(&info.version)
                        .to_string();

                    log::info!(
                        "Update available: {} (current: {})",
                        version_str,
                        env!("CARGO_PKG_VERSION")
                    );

                    // Only notify if we haven't already notified about this version
                    let already_notified = self
                        .config
                        .last_notified_version
                        .as_ref()
                        .is_some_and(|v| v == &version_str);

                    if !already_notified {
                        self.notify_update_available(info);
                        self.config.last_notified_version = Some(version_str);
                        config_changed = true;
                    }
                }
                UpdateCheckResult::UpToDate => {
                    log::info!("par-term is up to date ({})", env!("CARGO_PKG_VERSION"));
                }
                UpdateCheckResult::Error(e) => {
                    log::warn!("Update check failed: {}", e);
                }
                UpdateCheckResult::Disabled | UpdateCheckResult::Skipped => {
                    // Silent
                }
            }

            self.last_update_result = Some(result);

            // Sync update version to status bar widgets
            let version = self
                .last_update_result
                .as_ref()
                .and_then(update_available_version);
            let result_clone = self.last_update_result.clone();
            for ws in self.windows.values_mut() {
                ws.status_bar_ui.update_available_version = version.clone();
                ws.last_update_result = result_clone.clone();
            }

            // Save config with updated timestamp if check was successful
            if config_changed {
                self.config.last_update_check = Some(current_timestamp());
                if let Err(e) = self.config.save() {
                    log::warn!("Failed to save config after update check: {}", e);
                }
            }

            // Schedule next check based on frequency
            self.next_update_check = self
                .config
                .update_check_frequency
                .as_seconds()
                .map(|secs| now + Duration::from_secs(secs));
        }
    }

    /// Show desktop notification when update is available
    fn notify_update_available(&self, info: &crate::update_checker::UpdateInfo) {
        let version_str = info.version.strip_prefix('v').unwrap_or(&info.version);
        let current = env!("CARGO_PKG_VERSION");
        let summary = format!("par-term v{} Available", version_str);
        let body = format!(
            "You have v{}. Check Settings > Advanced > Updates.",
            current
        );

        #[cfg(not(target_os = "macos"))]
        {
            use notify_rust::Notification;
            let _ = Notification::new()
                .summary(&summary)
                .body(&body)
                .appname("par-term")
                .timeout(notify_rust::Timeout::Milliseconds(8000))
                .show();
        }

        #[cfg(target_os = "macos")]
        {
            let script = format!(
                r#"display notification "{}" with title "{}""#,
                body.replace('"', r#"\""#),
                summary.replace('"', r#"\""#),
            );
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .spawn();
        }
    }

    /// Force an immediate update check (triggered from UI)
    pub fn force_update_check(&mut self) {
        use crate::update_checker::current_timestamp;

        let (result, should_save) = self.update_checker.check_now(&self.config, true);

        // Log the result
        match &result {
            UpdateCheckResult::UpdateAvailable(info) => {
                log::info!(
                    "Update available: {} (current: {})",
                    info.version,
                    env!("CARGO_PKG_VERSION")
                );
            }
            UpdateCheckResult::UpToDate => {
                log::info!("par-term is up to date ({})", env!("CARGO_PKG_VERSION"));
            }
            UpdateCheckResult::Error(e) => {
                log::warn!("Update check failed: {}", e);
            }
            _ => {}
        }

        self.last_update_result = Some(result);

        // Sync update version and full result to status bar widgets and update dialog
        let version = self
            .last_update_result
            .as_ref()
            .and_then(update_available_version);
        let result_clone = self.last_update_result.clone();
        for ws in self.windows.values_mut() {
            ws.status_bar_ui.update_available_version = version.clone();
            ws.last_update_result = result_clone.clone();
        }

        // Save config with updated timestamp
        if should_save {
            self.config.last_update_check = Some(current_timestamp());
            if let Err(e) = self.config.save() {
                log::warn!("Failed to save config after update check: {}", e);
            }
        }
    }

    /// Force an update check and sync the result to the settings window.
    pub fn force_update_check_for_settings(&mut self) {
        self.force_update_check();
        // Sync the result to the settings window
        if let Some(settings_window) = &mut self.settings_window {
            settings_window.settings_ui.last_update_result = self
                .last_update_result
                .as_ref()
                .map(to_settings_update_result);
            settings_window.request_redraw();
        }
    }

    /// Detect the installation type and convert to the settings-ui enum.
    fn detect_installation_type(&self) -> par_term_settings_ui::InstallationType {
        let install = crate::self_updater::detect_installation();
        match install {
            crate::self_updater::InstallationType::Homebrew => {
                par_term_settings_ui::InstallationType::Homebrew
            }
            crate::self_updater::InstallationType::CargoInstall => {
                par_term_settings_ui::InstallationType::CargoInstall
            }
            crate::self_updater::InstallationType::MacOSBundle => {
                par_term_settings_ui::InstallationType::MacOSBundle
            }
            crate::self_updater::InstallationType::StandaloneBinary => {
                par_term_settings_ui::InstallationType::StandaloneBinary
            }
        }
    }

    /// Send a command to the shell
    fn send_command_to_shell(&mut self, cmd: &str) {
        // Send to the first window's active tab
        if let Some(window_state) = self.windows.values_mut().next()
            && let Some(tab) = window_state.tab_manager.active_tab_mut()
        {
            // Send the command followed by Enter
            let cmd_with_enter = format!("{}\n", cmd);
            if let Ok(term) = tab.terminal.try_lock() {
                if let Err(e) = term.write(cmd_with_enter.as_bytes()) {
                    log::error!("Failed to send command to shell: {}", e);
                } else {
                    log::info!("Sent command to shell: {}", cmd);
                }
            }
        }
    }

    /// Take a screenshot
    fn take_screenshot(&mut self) {
        log::info!("Taking screenshot...");

        // Determine output path
        let output_path = match &self.runtime_options.screenshot {
            Some(path) if !path.as_os_str().is_empty() => {
                log::info!("Screenshot path specified: {:?}", path);
                path.clone()
            }
            _ => {
                // Generate timestamped filename
                let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                let path = PathBuf::from(format!("par-term-{}.png", timestamp));
                log::info!("Using auto-generated screenshot path: {:?}", path);
                path
            }
        };

        // Get the first window and take screenshot
        if let Some(window_state) = self.windows.values_mut().next() {
            if let Some(renderer) = &mut window_state.renderer {
                log::info!("Capturing screenshot from renderer...");
                match renderer.take_screenshot() {
                    Ok(image_data) => {
                        log::info!(
                            "Screenshot captured: {}x{} pixels",
                            image_data.width(),
                            image_data.height()
                        );
                        // Save the image
                        if let Err(e) = image_data.save(&output_path) {
                            log::error!("Failed to save screenshot to {:?}: {}", output_path, e);
                        } else {
                            log::info!("Screenshot saved to {:?}", output_path);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to take screenshot: {}", e);
                    }
                }
            } else {
                log::warn!("No renderer available for screenshot");
            }
        } else {
            log::warn!("No window available for screenshot");
        }
    }

    /// Create a new window with a fresh terminal session
    pub fn create_window(&mut self, event_loop: &ActiveEventLoop) {
        use crate::config::WindowType;
        use crate::font_metrics::window_size_from_config;
        use winit::window::Window;

        // Reload config from disk to pick up any changes made by other windows
        // (e.g., integration versions saved after completing onboarding).
        // This ensures new windows don't show stale prompts.
        if let Ok(fresh_config) = Config::load() {
            self.config = fresh_config;
        }

        // Calculate window size from cols/rows BEFORE window creation.
        // This ensures the window opens at the exact correct size with no visible resize.
        // We use scale_factor=1.0 here since we don't have the actual display scale yet;
        // the window will be resized correctly once we know the actual scale factor.
        // Fallback to reasonable defaults (800x600) if font metrics calculation fails.
        let (width, height) = window_size_from_config(&self.config, 1.0).unwrap_or((800, 600));

        // Build window title, optionally including window number
        let window_number = self.windows.len() + 1;
        let title = if self.config.show_window_number {
            format!("{} [{}]", self.config.window_title, window_number)
        } else {
            self.config.window_title.clone()
        };

        let mut window_attrs = Window::default_attributes()
            .with_title(&title)
            .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            .with_decorations(self.config.window_decorations);

        // Lock window size if requested (prevent resize)
        if self.config.lock_window_size {
            window_attrs = window_attrs.with_resizable(false);
            log::info!("Window size locked (resizing disabled)");
        }

        // Start in fullscreen if window_type is Fullscreen
        if self.config.window_type == WindowType::Fullscreen {
            window_attrs =
                window_attrs.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            log::info!("Window starting in fullscreen mode");
        }

        // Load and set the application icon
        let icon_bytes = include_bytes!("../../assets/icon.png");
        if let Ok(icon_image) = image::load_from_memory(icon_bytes) {
            let rgba = icon_image.to_rgba8();
            let (width, height) = rgba.dimensions();
            if let Ok(icon) = winit::window::Icon::from_rgba(rgba.into_raw(), width, height) {
                window_attrs = window_attrs.with_window_icon(Some(icon));
                log::info!("Window icon set ({}x{})", width, height);
            } else {
                log::warn!("Failed to create window icon from RGBA data");
            }
        } else {
            log::warn!("Failed to load embedded icon image");
        }

        // Set window always-on-top if requested
        if self.config.window_always_on_top {
            window_attrs = window_attrs.with_window_level(winit::window::WindowLevel::AlwaysOnTop);
            log::info!("Window always-on-top enabled");
        }

        // Always enable window transparency support for runtime opacity changes
        window_attrs = window_attrs.with_transparent(true);
        log::info!(
            "Window transparency enabled (opacity: {})",
            self.config.window_opacity
        );

        match event_loop.create_window(window_attrs) {
            Ok(window) => {
                let window_id = window.id();
                let mut window_state =
                    WindowState::new(self.config.clone(), Arc::clone(&self.runtime));
                // Set window index for title formatting (window_number calculated earlier)
                window_state.window_index = window_number;

                // Initialize async components using the shared runtime
                let runtime = Arc::clone(&self.runtime);
                if let Err(e) = runtime.block_on(window_state.initialize_async(window, None)) {
                    log::error!("Failed to initialize window: {}", e);
                    return;
                }

                // Initialize menu for the first window (macOS global menu) or per-window (Windows/Linux)
                if self.menu.is_none() {
                    match MenuManager::new() {
                        Ok(menu) => {
                            // Attach menu to window (platform-specific)
                            if let Some(win) = &window_state.window
                                && let Err(e) = menu.init_for_window(win)
                            {
                                log::warn!("Failed to initialize menu for window: {}", e);
                            }
                            self.menu = Some(menu);
                        }
                        Err(e) => {
                            log::warn!("Failed to create menu: {}", e);
                        }
                    }
                } else if let Some(menu) = &self.menu
                    && let Some(win) = &window_state.window
                    && let Err(e) = menu.init_for_window(win)
                {
                    // For additional windows on Windows/Linux, attach menu
                    log::warn!("Failed to initialize menu for window: {}", e);
                }

                // Apply target monitor and edge positioning after window creation
                if let Some(win) = &window_state.window {
                    self.apply_window_positioning(win, event_loop);
                }

                // Handle tmux auto-attach on first window only
                if self.windows.is_empty()
                    && window_state.config.tmux_enabled
                    && window_state.config.tmux_auto_attach
                {
                    let session_name = window_state.config.tmux_auto_attach_session.clone();

                    // Use gateway mode: writes tmux commands to existing PTY
                    if let Some(ref name) = session_name {
                        if !name.is_empty() {
                            log::info!(
                                "tmux auto-attach: attempting to attach to session '{}' via gateway",
                                name
                            );
                            match window_state.attach_tmux_gateway(name) {
                                Ok(()) => {
                                    log::info!(
                                        "tmux auto-attach: gateway initiated for session '{}'",
                                        name
                                    );
                                }
                                Err(e) => {
                                    log::warn!(
                                        "tmux auto-attach: failed to attach to '{}': {} - continuing without tmux",
                                        name,
                                        e
                                    );
                                    // Continue without tmux - don't fail startup
                                }
                            }
                        } else {
                            // Empty string means create new session
                            log::info!(
                                "tmux auto-attach: no session specified, creating new session via gateway"
                            );
                            if let Err(e) = window_state.initiate_tmux_gateway(None) {
                                log::warn!(
                                    "tmux auto-attach: failed to create new session: {} - continuing without tmux",
                                    e
                                );
                            }
                        }
                    } else {
                        // None means create new session
                        log::info!(
                            "tmux auto-attach: no session specified, creating new session via gateway"
                        );
                        if let Err(e) = window_state.initiate_tmux_gateway(None) {
                            log::warn!(
                                "tmux auto-attach: failed to create new session: {} - continuing without tmux",
                                e
                            );
                        }
                    }
                }

                self.windows.insert(window_id, window_state);
                self.pending_window_count += 1;

                // Sync existing update state to new window's status bar and dialog
                let update_version = self
                    .last_update_result
                    .as_ref()
                    .and_then(update_available_version);
                let update_result_clone = self.last_update_result.clone();
                let install_type = self.detect_installation_type();
                if let Some(ws) = self.windows.get_mut(&window_id) {
                    ws.status_bar_ui.update_available_version = update_version;
                    ws.last_update_result = update_result_clone;
                    ws.installation_type = install_type;
                }

                // Set start time on first window creation (for CLI timers)
                if self.start_time.is_none() {
                    self.start_time = Some(Instant::now());
                }

                log::info!(
                    "Created new window {:?} (total: {})",
                    window_id,
                    self.windows.len()
                );
            }
            Err(e) => {
                log::error!("Failed to create window: {}", e);
            }
        }
    }

    /// Apply window positioning based on config (target monitor and edge anchoring)
    fn apply_window_positioning(
        &self,
        window: &std::sync::Arc<winit::window::Window>,
        event_loop: &ActiveEventLoop,
    ) {
        use crate::config::WindowType;

        // Get list of available monitors
        let monitors: Vec<_> = event_loop.available_monitors().collect();
        if monitors.is_empty() {
            log::warn!("No monitors available for window positioning");
            return;
        }

        // Select target monitor (default to primary/first)
        let monitor = if let Some(index) = self.config.target_monitor {
            monitors
                .get(index)
                .cloned()
                .or_else(|| monitors.first().cloned())
        } else {
            event_loop
                .primary_monitor()
                .or_else(|| monitors.first().cloned())
        };

        let Some(monitor) = monitor else {
            log::warn!("Could not determine target monitor");
            return;
        };

        let monitor_pos = monitor.position();
        let monitor_size = monitor.size();
        let window_size = window.outer_size();

        // Apply edge positioning if configured
        match self.config.window_type {
            WindowType::EdgeTop => {
                // Position at top of screen, spanning full width
                window.set_outer_position(winit::dpi::PhysicalPosition::new(
                    monitor_pos.x,
                    monitor_pos.y,
                ));
                let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
                    monitor_size.width,
                    window_size.height,
                ));
                log::info!("Window positioned at top edge of monitor");
            }
            WindowType::EdgeBottom => {
                // Position at bottom of screen, spanning full width
                let y = monitor_pos.y + monitor_size.height as i32 - window_size.height as i32;
                window.set_outer_position(winit::dpi::PhysicalPosition::new(monitor_pos.x, y));
                let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
                    monitor_size.width,
                    window_size.height,
                ));
                log::info!("Window positioned at bottom edge of monitor");
            }
            WindowType::EdgeLeft => {
                // Position at left of screen, spanning full height
                window.set_outer_position(winit::dpi::PhysicalPosition::new(
                    monitor_pos.x,
                    monitor_pos.y,
                ));
                let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
                    window_size.width,
                    monitor_size.height,
                ));
                log::info!("Window positioned at left edge of monitor");
            }
            WindowType::EdgeRight => {
                // Position at right of screen, spanning full height
                let x = monitor_pos.x + monitor_size.width as i32 - window_size.width as i32;
                window.set_outer_position(winit::dpi::PhysicalPosition::new(x, monitor_pos.y));
                let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
                    window_size.width,
                    monitor_size.height,
                ));
                log::info!("Window positioned at right edge of monitor");
            }
            WindowType::Normal | WindowType::Fullscreen => {
                // For normal/fullscreen, just position on target monitor if specified
                if self.config.target_monitor.is_some() {
                    // Center window on target monitor
                    let x =
                        monitor_pos.x + (monitor_size.width as i32 - window_size.width as i32) / 2;
                    let y = monitor_pos.y
                        + (monitor_size.height as i32 - window_size.height as i32) / 2;
                    window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
                    log::info!(
                        "Window centered on monitor {} at ({}, {})",
                        self.config.target_monitor.unwrap_or(0),
                        x,
                        y
                    );
                }
            }
        }

        // Move window to target macOS Space if configured (macOS only, no-op on other platforms)
        if let Some(space) = self.config.target_space
            && let Err(e) = crate::macos_space::move_window_to_space(window, space)
        {
            log::warn!("Failed to move window to Space {}: {}", space, e);
        }
    }

    /// Close a specific window
    pub fn close_window(&mut self, window_id: WindowId) {
        // Save session state before removing the last window (while data is still available).
        // Capture happens synchronously (fast, in-memory), disk write is on a background thread.
        if self.config.restore_session
            && self.windows.len() == 1
            && self.windows.contains_key(&window_id)
        {
            self.save_session_state_background();
        }

        if let Some(window_state) = self.windows.remove(&window_id) {
            log::info!(
                "Closing window {:?} (remaining: {})",
                window_id,
                self.windows.len()
            );
            // Hide window immediately for instant visual feedback
            if let Some(ref window) = window_state.window {
                window.set_visible(false);
            }
            // WindowState's Drop impl handles cleanup
            drop(window_state);
        }

        // Exit app when last window closes
        if self.windows.is_empty() {
            log::info!("Last window closed, exiting application");
            // Close settings window FIRST before marking exit
            // This ensures settings window resources are cleaned up before app teardown
            if self.settings_window.is_some() {
                log::info!("Closing settings window before exit");
                self.close_settings_window();
            }
            self.should_exit = true;
        }
    }

    /// Save session state on a background thread to avoid blocking the main thread.
    /// Captures state synchronously (fast, in-memory) then spawns disk I/O.
    fn save_session_state_background(&self) {
        let state = crate::session::capture::capture_session(&self.windows);
        let _ = std::thread::Builder::new()
            .name("session-save".into())
            .spawn(move || {
                if let Err(e) = crate::session::storage::save_session(&state) {
                    log::error!("Failed to save session state: {}", e);
                }
            });
    }

    /// Restore windows from the last saved session
    ///
    /// Returns true if session was successfully restored, false otherwise.
    pub fn restore_session(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let session = match crate::session::storage::load_session() {
            Ok(Some(session)) => session,
            Ok(None) => {
                log::info!("No saved session found, creating default window");
                return false;
            }
            Err(e) => {
                log::warn!(
                    "Failed to load session state: {}, creating default window",
                    e
                );
                return false;
            }
        };

        if session.windows.is_empty() {
            log::info!("Saved session has no windows, creating default window");
            return false;
        }

        log::info!(
            "Restoring session ({} windows) saved at {}",
            session.windows.len(),
            session.saved_at
        );

        for session_window in &session.windows {
            // Validate CWDs for tabs
            let tab_cwds: Vec<Option<String>> = session_window
                .tabs
                .iter()
                .map(|tab| crate::session::restore::validate_cwd(&tab.cwd))
                .collect();

            let created_window_id = self.create_window_with_overrides(
                event_loop,
                session_window.position,
                session_window.size,
                &tab_cwds,
                session_window.active_tab_index,
            );

            // Restore pane layouts, user titles, custom colors, and icons
            if let Some(window_id) = created_window_id
                && let Some(window_state) = self.windows.get_mut(&window_id)
            {
                let tabs = window_state.tab_manager.tabs_mut();
                for (tab_idx, session_tab) in session_window.tabs.iter().enumerate() {
                    if let Some(ref layout) = session_tab.pane_layout
                        && let Some(tab) = tabs.get_mut(tab_idx)
                    {
                        tab.restore_pane_layout(layout, &self.config, Arc::clone(&self.runtime));
                    }
                }

                // Restore user titles, custom colors, and icons
                for (tab_idx, session_tab) in session_window.tabs.iter().enumerate() {
                    if let Some(tab) = tabs.get_mut(tab_idx) {
                        if let Some(ref user_title) = session_tab.user_title {
                            tab.title = user_title.clone();
                            tab.user_named = true;
                            tab.has_default_title = false;
                        }
                        if let Some(color) = session_tab.custom_color {
                            tab.set_custom_color(color);
                        }
                        if let Some(ref icon) = session_tab.custom_icon {
                            tab.custom_icon = Some(icon.clone());
                        }
                    }
                }
            }
        }

        // Clear the saved session file after successful restore
        if let Err(e) = crate::session::storage::clear_session() {
            log::warn!("Failed to clear session file after restore: {}", e);
        }

        // If no windows were created (shouldn't happen), fall back
        if self.windows.is_empty() {
            log::warn!("Session restore created no windows, creating default");
            return false;
        }

        true
    }

    /// Get mutable reference to a window's state
    pub fn get_window_mut(&mut self, window_id: WindowId) -> Option<&mut WindowState> {
        self.windows.get_mut(&window_id)
    }

    /// Get reference to a window's state
    pub fn get_window(&self, window_id: WindowId) -> Option<&WindowState> {
        self.windows.get(&window_id)
    }

    /// Handle a menu action
    pub fn handle_menu_action(
        &mut self,
        action: MenuAction,
        event_loop: &ActiveEventLoop,
        focused_window: Option<WindowId>,
    ) {
        match action {
            MenuAction::NewWindow => {
                self.create_window(event_loop);
            }
            MenuAction::CloseWindow => {
                // Smart close: close tab if multiple tabs, close window if single tab
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && window_state.close_current_tab()
                {
                    // Last tab closed, close the window
                    self.close_window(window_id);
                }
            }
            MenuAction::NewTab => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.new_tab();
                }
            }
            MenuAction::CloseTab => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && window_state.close_current_tab()
                {
                    // Last tab closed, close the window
                    self.close_window(window_id);
                }
            }
            MenuAction::NextTab => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.next_tab();
                }
            }
            MenuAction::PreviousTab => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.prev_tab();
                }
            }
            MenuAction::SwitchToTab(index) => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.switch_to_tab_index(index);
                }
            }
            MenuAction::MoveTabLeft => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.move_tab_left();
                }
            }
            MenuAction::MoveTabRight => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.move_tab_right();
                }
            }
            MenuAction::DuplicateTab => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.duplicate_tab();
                }
            }
            MenuAction::Quit => {
                // Close all windows
                let window_ids: Vec<_> = self.windows.keys().copied().collect();
                for window_id in window_ids {
                    self.close_window(window_id);
                }
            }
            MenuAction::Copy => {
                // If settings window is focused, inject copy event into egui
                if let Some(sw) = &self.settings_window
                    && sw.is_focused()
                {
                    if let Some(sw) = &mut self.settings_window {
                        sw.inject_event(egui::Event::Copy);
                    }
                    return;
                }
                // If an egui overlay (profile modal, search, etc.) is active, inject into main egui
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && window_state.has_egui_text_overlay_visible()
                {
                    window_state.pending_egui_events.push(egui::Event::Copy);
                    return;
                }
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && let Some(text) = window_state.get_selected_text_for_copy()
                {
                    if let Err(e) = window_state.input_handler.copy_to_clipboard(&text) {
                        log::error!("Failed to copy to clipboard: {}", e);
                    } else {
                        // Sync to tmux paste buffer if connected
                        window_state.sync_clipboard_to_tmux(&text);
                    }
                }
            }
            MenuAction::Paste => {
                // If settings window is focused, inject paste into its egui context
                // (macOS menu accelerator intercepts Cmd+V before egui sees it)
                if let Some(sw) = &self.settings_window
                    && sw.is_focused()
                {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        if let Ok(text) = clipboard.get_text() {
                            if let Some(sw) = &mut self.settings_window {
                                sw.inject_paste(text);
                            }
                            return;
                        }
                        // Clipboard has no text — check for image below.
                        // Don't return early so the image-paste forwarding
                        // code can send Ctrl+V to the terminal.
                        if clipboard.get_image().is_err() {
                            // Neither text nor image — nothing to paste
                            return;
                        }
                    } else {
                        return;
                    }
                }
                // If an egui overlay (profile modal, search, etc.) is active, inject into main egui
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && window_state.has_egui_text_overlay_visible()
                {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        if let Ok(text) = clipboard.get_text() {
                            window_state
                                .pending_egui_events
                                .push(egui::Event::Paste(text));
                            return;
                        }
                        // Clipboard has no text — fall through to check for image
                        // so it can be forwarded to the terminal
                        if clipboard.get_image().is_err() {
                            return;
                        }
                    } else {
                        return;
                    }
                }
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    if let Some(text) = window_state.input_handler.paste_from_clipboard() {
                        window_state.paste_text(&text);
                    } else if window_state.input_handler.clipboard_has_image() {
                        // Clipboard has an image but no text — forward as Ctrl+V (0x16) so
                        // image-aware child processes (e.g., Claude Code) can handle image paste
                        if let Some(tab) = window_state.tab_manager.active_tab() {
                            let terminal_clone = Arc::clone(&tab.terminal);
                            window_state.runtime.spawn(async move {
                                let term = terminal_clone.lock().await;
                                let _ = term.write(b"\x16");
                            });
                        }
                    }
                }
            }
            MenuAction::SelectAll => {
                // If settings window is focused, inject select-all into egui
                if let Some(sw) = &self.settings_window
                    && sw.is_focused()
                {
                    if let Some(sw) = &mut self.settings_window {
                        // egui has no dedicated SelectAll event; use Cmd+A key event
                        sw.inject_event(egui::Event::Key {
                            key: egui::Key::A,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::COMMAND,
                        });
                    }
                    return;
                }
                // If an egui overlay is active, inject select-all into main egui
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && window_state.has_egui_text_overlay_visible()
                {
                    window_state.pending_egui_events.push(egui::Event::Key {
                        key: egui::Key::A,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::COMMAND,
                    });
                    return;
                }
                // Not implemented for terminal - would select all visible text
                log::debug!("SelectAll menu action (not implemented for terminal)");
            }
            MenuAction::ClearScrollback => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    // Clear scrollback in active tab
                    let cleared = if let Some(tab) = window_state.tab_manager.active_tab_mut() {
                        if let Ok(mut term) = tab.terminal.try_lock() {
                            term.clear_scrollback();
                            term.clear_scrollback_metadata();
                            tab.cache.scrollback_len = 0;
                            tab.trigger_marks.clear();
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if cleared {
                        window_state.set_scroll_target(0);
                        log::info!("Cleared scrollback buffer");
                    }
                }
            }
            MenuAction::ClipboardHistory => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.overlay_ui.clipboard_history_ui.toggle();
                    window_state.needs_redraw = true;
                }
            }
            MenuAction::ToggleFullscreen => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && let Some(window) = &window_state.window
                {
                    window_state.is_fullscreen = !window_state.is_fullscreen;
                    if window_state.is_fullscreen {
                        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                    } else {
                        window.set_fullscreen(None);
                    }
                }
            }
            MenuAction::MaximizeVertically => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && let Some(window) = &window_state.window
                {
                    // Get current monitor to determine screen height
                    if let Some(monitor) = window.current_monitor() {
                        let monitor_pos = monitor.position();
                        let monitor_size = monitor.size();
                        let window_pos = window.outer_position().unwrap_or_default();
                        let window_size = window.outer_size();

                        // Set window to span full height while keeping current X position and width
                        window.set_outer_position(winit::dpi::PhysicalPosition::new(
                            window_pos.x,
                            monitor_pos.y,
                        ));
                        let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
                            window_size.width,
                            monitor_size.height,
                        ));
                        log::info!(
                            "Window maximized vertically to {} pixels",
                            monitor_size.height
                        );
                    }
                }
            }
            MenuAction::IncreaseFontSize => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.config.font_size = (window_state.config.font_size + 1.0).min(72.0);
                    window_state.pending_font_rebuild = true;
                    if let Some(window) = &window_state.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::DecreaseFontSize => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.config.font_size = (window_state.config.font_size - 1.0).max(6.0);
                    window_state.pending_font_rebuild = true;
                    if let Some(window) = &window_state.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::ResetFontSize => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.config.font_size = 14.0;
                    window_state.pending_font_rebuild = true;
                    if let Some(window) = &window_state.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::ToggleFpsOverlay => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.debug.show_fps_overlay = !window_state.debug.show_fps_overlay;
                    if let Some(window) = &window_state.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::OpenSettings => {
                self.open_settings_window(event_loop);
            }
            MenuAction::Minimize => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get(&window_id)
                    && let Some(window) = &window_state.window
                {
                    window.set_minimized(true);
                }
            }
            MenuAction::Zoom => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get(&window_id)
                    && let Some(window) = &window_state.window
                {
                    window.set_maximized(!window.is_maximized());
                }
            }
            MenuAction::ShowHelp => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.overlay_ui.help_ui.toggle();
                    if let Some(window) = &window_state.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::About => {
                log::info!("About par-term v{}", env!("CARGO_PKG_VERSION"));
                // Could show an about dialog here
            }
            MenuAction::ToggleBackgroundShader => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.toggle_background_shader();
                }
            }
            MenuAction::ToggleCursorShader => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.toggle_cursor_shader();
                }
            }
            MenuAction::ReloadConfig => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.reload_config();
                }
            }
            MenuAction::ManageProfiles => {
                self.open_settings_window(event_loop);
                if let Some(sw) = &mut self.settings_window {
                    sw.settings_ui
                        .set_selected_tab(crate::settings_ui::sidebar::SettingsTab::Profiles);
                }
            }
            MenuAction::ToggleProfileDrawer => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.toggle_profile_drawer();
                }
            }
            MenuAction::OpenProfile(profile_id) => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.open_profile(profile_id);
                }
            }
            MenuAction::SaveArrangement => {
                // Open settings window to the Arrangements tab
                self.open_settings_window(event_loop);
                if let Some(sw) = &mut self.settings_window {
                    sw.settings_ui
                        .set_selected_tab(crate::settings_ui::sidebar::SettingsTab::Arrangements);
                }
            }
            MenuAction::InstallShellIntegrationRemote => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state
                        .overlay_ui
                        .remote_shell_install_ui
                        .show_dialog();
                    window_state.needs_redraw = true;
                }
            }
        }
    }

    /// Process any pending menu events
    pub fn process_menu_events(
        &mut self,
        event_loop: &ActiveEventLoop,
        focused_window: Option<WindowId>,
    ) {
        if let Some(menu) = &self.menu {
            // Collect actions to avoid borrow conflicts
            let actions: Vec<_> = menu.poll_events().collect();
            for action in actions {
                self.handle_menu_action(action, event_loop, focused_window);
            }
        }
    }

    /// Open the settings window (or focus if already open)
    pub fn open_settings_window(&mut self, event_loop: &ActiveEventLoop) {
        // If already open, bring to front and focus
        if let Some(settings_window) = &self.settings_window {
            settings_window.focus();
            return;
        }

        // Create new settings window using shared runtime
        let config = self.config.clone();
        let runtime = Arc::clone(&self.runtime);

        // Get supported vsync modes from the first window's renderer
        let supported_vsync_modes: Vec<crate::config::VsyncMode> = self
            .windows
            .values()
            .next()
            .and_then(|ws| ws.renderer.as_ref())
            .map(|renderer| {
                [
                    crate::config::VsyncMode::Immediate,
                    crate::config::VsyncMode::Mailbox,
                    crate::config::VsyncMode::Fifo,
                ]
                .into_iter()
                .filter(|mode| renderer.is_vsync_mode_supported(*mode))
                .collect()
            })
            .unwrap_or_else(|| vec![crate::config::VsyncMode::Fifo]); // Fifo always supported

        match runtime.block_on(SettingsWindow::new(
            event_loop,
            config,
            supported_vsync_modes,
        )) {
            Ok(mut settings_window) => {
                log::info!("Opened settings window {:?}", settings_window.window_id());
                // Set app version from main crate (env! expands to the correct version here)
                settings_window.settings_ui.app_version = env!("CARGO_PKG_VERSION");
                // Wire up shell integration fn pointers
                settings_window
                    .settings_ui
                    .shell_integration_detected_shell_fn =
                    Some(crate::shell_integration_installer::detected_shell);
                settings_window
                    .settings_ui
                    .shell_integration_is_installed_fn =
                    Some(crate::shell_integration_installer::is_installed);
                // Sync last update check result to settings UI
                settings_window.settings_ui.last_update_result = self
                    .last_update_result
                    .as_ref()
                    .map(to_settings_update_result);
                // Sync profiles from first window's profile manager
                let profiles = self
                    .windows
                    .values()
                    .next()
                    .map(|ws| ws.overlay_ui.profile_manager.to_vec())
                    .unwrap_or_default();
                settings_window.settings_ui.sync_profiles(profiles);
                // Sync available agents from first window's discovered agents
                if let Some(ws) = self.windows.values().next() {
                    settings_window.settings_ui.available_agent_ids = ws
                        .agent_state
                        .available_agents
                        .iter()
                        .map(|a| (a.identity.clone(), a.name.clone()))
                        .collect();
                }
                self.settings_window = Some(settings_window);
                // Sync arrangement data to settings UI
                self.sync_arrangements_to_settings();
            }
            Err(e) => {
                log::error!("Failed to create settings window: {}", e);
            }
        }
    }

    /// Close the settings window
    pub fn close_settings_window(&mut self) {
        if let Some(settings_window) = self.settings_window.take() {
            // Persist collapsed section states AND current live-preview config.
            //
            // The settings window sends ApplyConfig every frame, updating both
            // `self.config` and all `window_state.config` with live-preview values.
            // We save `self.config` (not loading from disk) so the config file matches
            // the in-memory state. Previously, loading from disk and saving collapsed
            // sections triggered the config file watcher, which reloaded stale disk
            // values and reverted any live-preview changes (like tab_inactive_outline_only).
            let collapsed = settings_window.settings_ui.collapsed_sections_snapshot();
            if !collapsed.is_empty() || !self.config.collapsed_settings_sections.is_empty() {
                self.config.collapsed_settings_sections = collapsed.clone();
                for window_state in self.windows.values_mut() {
                    window_state.config.collapsed_settings_sections = collapsed.clone();
                }
            }
            // Save the in-memory config which includes both collapsed sections and
            // any live-preview changes from the settings window.
            if let Err(e) = self.config.save() {
                log::error!("Failed to persist config on settings window close: {}", e);
            }
            log::info!("Closed settings window");
        }
    }

    /// Check if a window ID belongs to the settings window
    pub fn is_settings_window(&self, window_id: WindowId) -> bool {
        self.settings_window
            .as_ref()
            .is_some_and(|sw| sw.window_id() == window_id)
    }

    /// Handle an event for the settings window
    pub fn handle_settings_window_event(
        &mut self,
        event: WindowEvent,
    ) -> Option<SettingsWindowAction> {
        if let Some(settings_window) = &mut self.settings_window {
            let action = settings_window.handle_window_event(event);

            // Handle close action
            if settings_window.should_close() {
                self.close_settings_window();
                return Some(SettingsWindowAction::Close);
            }

            return Some(action);
        }
        None
    }

    /// Apply config changes from settings window to all terminal windows
    pub fn apply_config_to_windows(&mut self, config: &Config) {
        use crate::app::config_updates::ConfigChanges;

        // Apply log level change immediately
        crate::debug::set_log_level(config.log_level.to_level_filter());

        // Track shader errors for the standalone settings window
        // Option<Option<String>>: None = no change attempted, Some(None) = success, Some(Some(err)) = error
        let mut last_shader_result: Option<Option<String>> = None;
        let mut last_cursor_shader_result: Option<Option<String>> = None;
        let mut ai_agent_list_changed = false;

        for window_state in self.windows.values_mut() {
            // Detect what changed
            let changes = ConfigChanges::detect(&window_state.config, config);

            // Update the config
            window_state.config = config.clone();

            if changes.ai_inspector_custom_agents {
                window_state.refresh_available_agents();
                ai_agent_list_changed = true;
            }

            // Rebuild keybinding registry if keybindings changed
            if changes.keybindings {
                window_state.keybinding_registry =
                    crate::keybindings::KeybindingRegistry::from_config(&config.keybindings);
                log::info!(
                    "Keybinding registry rebuilt with {} bindings",
                    config.keybindings.len()
                );
            }

            // Sync AI Inspector auto-approve / YOLO mode to connected agent
            if changes.ai_inspector_auto_approve
                && let Some(agent) = &window_state.agent_state.agent
            {
                let agent = agent.clone();
                let auto_approve = config.ai_inspector_auto_approve;
                let mode = if auto_approve {
                    "bypassPermissions"
                } else {
                    "default"
                }
                .to_string();
                window_state.runtime.spawn(async move {
                    let agent = agent.lock().await;
                    agent
                        .auto_approve
                        .store(auto_approve, std::sync::atomic::Ordering::Relaxed);
                    if let Err(e) = agent.set_mode(&mode).await {
                        log::error!("ACP: failed to set mode '{mode}': {e}");
                    }
                });
            }

            // Apply changes to renderer and collect any shader errors
            let (shader_result, cursor_result) = if let Some(renderer) = &mut window_state.renderer
            {
                // Update opacity
                renderer.update_opacity(config.window_opacity);

                // Update transparency mode if changed
                if changes.transparency_mode {
                    renderer.set_transparency_affects_only_default_background(
                        config.transparency_affects_only_default_background,
                    );
                    window_state.needs_redraw = true;
                }

                // Update text opacity mode if changed
                if changes.keep_text_opaque {
                    renderer.set_keep_text_opaque(config.keep_text_opaque);
                    window_state.needs_redraw = true;
                }

                if changes.link_underline_style {
                    renderer.set_link_underline_style(config.link_underline_style);
                    window_state.needs_redraw = true;
                }

                // Update vsync mode if changed
                if changes.vsync_mode {
                    let (actual_mode, _changed) = renderer.update_vsync_mode(config.vsync_mode);
                    // If the actual mode differs, update config
                    if actual_mode != config.vsync_mode {
                        window_state.config.vsync_mode = actual_mode;
                        log::warn!(
                            "Vsync mode {:?} is not supported. Using {:?} instead.",
                            config.vsync_mode,
                            actual_mode
                        );
                    }
                }

                // Update scrollbar appearance
                renderer.update_scrollbar_appearance(
                    config.scrollbar_width,
                    config.scrollbar_thumb_color,
                    config.scrollbar_track_color,
                );

                // Update cursor color
                if changes.cursor_color {
                    renderer.update_cursor_color(config.cursor_color);
                }

                // Update cursor text color
                if changes.cursor_text_color {
                    renderer.update_cursor_text_color(config.cursor_text_color);
                }

                // Update cursor style and blink for all tabs
                if changes.cursor_style || changes.cursor_blink {
                    use crate::config::CursorStyle as ConfigCursorStyle;
                    use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;

                    let term_style = if config.cursor_blink {
                        match config.cursor_style {
                            ConfigCursorStyle::Block => TermCursorStyle::BlinkingBlock,
                            ConfigCursorStyle::Beam => TermCursorStyle::BlinkingBar,
                            ConfigCursorStyle::Underline => TermCursorStyle::BlinkingUnderline,
                        }
                    } else {
                        match config.cursor_style {
                            ConfigCursorStyle::Block => TermCursorStyle::SteadyBlock,
                            ConfigCursorStyle::Beam => TermCursorStyle::SteadyBar,
                            ConfigCursorStyle::Underline => TermCursorStyle::SteadyUnderline,
                        }
                    };

                    for tab in window_state.tab_manager.tabs_mut() {
                        if let Ok(mut term) = tab.terminal.try_lock() {
                            term.set_cursor_style(term_style);
                        }
                        tab.cache.cells = None; // Invalidate cache to redraw cursor
                    }
                    window_state.needs_redraw = true;
                }

                // Apply cursor enhancement changes
                if changes.cursor_enhancements {
                    renderer.update_cursor_guide(
                        config.cursor_guide_enabled,
                        config.cursor_guide_color,
                    );
                    renderer.update_cursor_shadow(
                        config.cursor_shadow_enabled,
                        config.cursor_shadow_color,
                        config.cursor_shadow_offset,
                        config.cursor_shadow_blur,
                    );
                    renderer.update_cursor_boost(config.cursor_boost, config.cursor_boost_color);
                    renderer.update_unfocused_cursor_style(config.unfocused_cursor_style);
                    window_state.needs_redraw = true;
                }

                // Apply command separator changes
                if changes.command_separator {
                    renderer.update_command_separator(
                        config.command_separator_enabled,
                        config.command_separator_thickness,
                        config.command_separator_opacity,
                        config.command_separator_exit_color,
                        config.command_separator_color,
                    );
                    window_state.needs_redraw = true;
                }

                // Apply background changes (mode, color, or image)
                if changes.any_bg_change() {
                    // Expand tilde in path
                    let expanded_path = config.background_image.as_ref().map(|p| {
                        if let Some(rest) = p.strip_prefix("~/")
                            && let Some(home) = dirs::home_dir()
                        {
                            return home.join(rest).to_string_lossy().to_string();
                        }
                        p.clone()
                    });
                    renderer.set_background(
                        config.background_mode,
                        config.background_color,
                        expanded_path.as_deref(),
                        config.background_image_mode,
                        config.background_image_opacity,
                        config.background_image_enabled,
                    );
                    window_state.needs_redraw = true;
                }

                // Apply per-pane background changes to existing panes
                if changes.pane_backgrounds {
                    // Pre-load all pane background textures into the renderer cache
                    for pb_config in &config.pane_backgrounds {
                        if let Err(e) = renderer.load_pane_background(&pb_config.image) {
                            log::error!(
                                "Failed to load pane {} background '{}': {}",
                                pb_config.index,
                                pb_config.image,
                                e
                            );
                        }
                    }

                    for tab in window_state.tab_manager.tabs_mut() {
                        if let Some(pm) = tab.pane_manager_mut() {
                            let panes = pm.all_panes_mut();
                            for (index, pane) in panes.into_iter().enumerate() {
                                if let Some((image_path, mode, opacity, darken)) =
                                    config.get_pane_background(index)
                                {
                                    let bg = crate::pane::PaneBackground {
                                        image_path: Some(image_path),
                                        mode,
                                        opacity,
                                        darken,
                                    };
                                    pane.set_background(bg);
                                } else {
                                    // Clear pane background if no longer configured
                                    pane.set_background(crate::pane::PaneBackground::new());
                                }
                            }
                        }
                    }
                    renderer.mark_dirty();
                    window_state.needs_redraw = true;
                }

                // Apply inline image settings changes
                if changes.image_scaling_mode {
                    renderer.update_image_scaling_mode(config.image_scaling_mode);
                    window_state.needs_redraw = true;
                }
                if changes.image_preserve_aspect_ratio {
                    renderer.update_image_preserve_aspect_ratio(config.image_preserve_aspect_ratio);
                    window_state.needs_redraw = true;
                }

                // Apply theme changes
                if changes.theme
                    && let Some(tab) = window_state.tab_manager.active_tab()
                    && let Ok(mut term) = tab.terminal.try_lock()
                {
                    term.set_theme(config.load_theme());
                }

                // Update ENQ answerback string across all tabs when changed
                if changes.answerback_string {
                    let answerback = if config.answerback_string.is_empty() {
                        None
                    } else {
                        Some(config.answerback_string.clone())
                    };
                    for tab in window_state.tab_manager.tabs_mut() {
                        if let Ok(term) = tab.terminal.try_lock() {
                            term.set_answerback_string(answerback.clone());
                        }
                    }
                }

                // Apply Unicode width settings
                if changes.unicode_width {
                    let width_config = par_term_emu_core_rust::WidthConfig::new(
                        config.unicode_version,
                        config.ambiguous_width,
                    );
                    for tab in window_state.tab_manager.tabs_mut() {
                        if let Ok(term) = tab.terminal.try_lock() {
                            term.set_width_config(width_config);
                        }
                    }
                }

                // Apply Unicode normalization form
                if changes.normalization_form {
                    for tab in window_state.tab_manager.tabs_mut() {
                        if let Ok(term) = tab.terminal.try_lock() {
                            term.set_normalization_form(config.normalization_form);
                        }
                    }
                }

                // Resolve per-shader settings (user override -> metadata defaults -> global)
                // This is computed once and used for both shader enable and background-as-channel0
                let shader_override = config
                    .custom_shader
                    .as_ref()
                    .and_then(|name| config.shader_configs.get(name));
                // Get shader metadata from cache for full 3-tier resolution
                let metadata = config.custom_shader.as_ref().and_then(|name| {
                    window_state
                        .shader_state
                        .shader_metadata_cache
                        .get(name)
                        .cloned()
                });
                let resolved = resolve_shader_config(shader_override, metadata.as_ref(), config);

                // Apply shader changes - track if change was attempted and result
                // Option<Option<String>>: None = no change attempted, Some(None) = success, Some(Some(err)) = error
                let shader_result =
                    if changes.any_shader_change() || changes.shader_per_shader_config {
                        log::info!(
                            "SETTINGS: applying shader change: {:?} -> {:?}",
                            window_state.config.custom_shader,
                            config.custom_shader
                        );
                        Some(
                            renderer
                                .set_custom_shader_enabled(
                                    config.custom_shader_enabled,
                                    config.custom_shader.as_deref(),
                                    config.window_opacity,
                                    config.custom_shader_animation,
                                    resolved.animation_speed,
                                    resolved.full_content,
                                    resolved.brightness,
                                    &resolved.channel_paths(),
                                    resolved.cubemap_path().map(|p| p.as_path()),
                                )
                                .err(),
                        )
                    } else {
                        None // No change attempted
                    };

                // Apply use_background_as_channel0 setting
                // This needs to be applied after the shader is loaded but before it renders
                // Include any_shader_change() to ensure the setting is applied when a new shader is loaded
                if changes.any_shader_change()
                    || changes.shader_use_background_as_channel0
                    || changes.any_bg_change()
                    || changes.shader_per_shader_config
                {
                    renderer.update_background_as_channel0_with_mode(
                        resolved.use_background_as_channel0,
                        config.background_mode,
                        config.background_color,
                    );
                }

                // Apply cursor shader changes
                let cursor_result = if changes.any_cursor_shader_toggle() {
                    Some(
                        renderer
                            .set_cursor_shader_enabled(
                                config.cursor_shader_enabled,
                                config.cursor_shader.as_deref(),
                                config.window_opacity,
                                config.cursor_shader_animation,
                                config.cursor_shader_animation_speed,
                            )
                            .err(),
                    )
                } else {
                    None // No change attempted
                };

                (shader_result, cursor_result)
            } else {
                (None, None)
            };

            // Track shader errors for propagation to standalone settings window
            // shader_result: None = no change attempted, Some(None) = success, Some(Some(err)) = error
            if let Some(result) = shader_result {
                last_shader_result = Some(result);
            }
            if let Some(result) = cursor_result {
                last_cursor_shader_result = Some(result);
            }

            // Apply font rendering changes that can update live
            if changes.font_rendering {
                if let Some(renderer) = &mut window_state.renderer {
                    let mut updated = false;
                    updated |= renderer.update_font_antialias(config.font_antialias);
                    updated |= renderer.update_font_hinting(config.font_hinting);
                    updated |= renderer.update_font_thin_strokes(config.font_thin_strokes);
                    updated |= renderer.update_minimum_contrast(config.minimum_contrast);
                    if updated {
                        window_state.needs_redraw = true;
                    }
                } else {
                    window_state.pending_font_rebuild = true;
                }
            }

            // Apply window-related changes
            if let Some(window) = &window_state.window {
                // Update window title (handles both title change and show_window_number toggle)
                // Note: config is already updated at this point (line 985)
                if changes.window_title || changes.show_window_number {
                    let title = window_state.format_title(&window_state.config.window_title);
                    window.set_title(&title);
                }
                if changes.window_decorations {
                    window.set_decorations(config.window_decorations);
                }
                if changes.lock_window_size {
                    window.set_resizable(!config.lock_window_size);
                    log::info!("Window resizable set to: {}", !config.lock_window_size);
                }
                window.set_window_level(if config.window_always_on_top {
                    winit::window::WindowLevel::AlwaysOnTop
                } else {
                    winit::window::WindowLevel::Normal
                });

                // Apply blur changes (macOS only)
                #[cfg(target_os = "macos")]
                if changes.blur {
                    let blur_radius = if config.blur_enabled && config.window_opacity < 1.0 {
                        config.blur_radius
                    } else {
                        0 // Disable blur when not enabled or fully opaque
                    };
                    if let Err(e) = crate::macos_blur::set_window_blur(window, blur_radius) {
                        log::warn!("Failed to set window blur: {}", e);
                    }
                }

                window.request_redraw();
            }

            // Apply window padding changes live without full renderer rebuild
            if changes.padding
                && let Some(renderer) = &mut window_state.renderer
            {
                if let Some((new_cols, new_rows)) =
                    renderer.update_window_padding(config.window_padding)
                {
                    let cell_width = renderer.cell_width();
                    let cell_height = renderer.cell_height();
                    let width_px = (new_cols as f32 * cell_width) as usize;
                    let height_px = (new_rows as f32 * cell_height) as usize;

                    for tab in window_state.tab_manager.tabs_mut() {
                        if let Ok(mut term) = tab.terminal.try_lock() {
                            term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                            let _ =
                                term.resize_with_pixels(new_cols, new_rows, width_px, height_px);
                        }
                        tab.cache.cells = None;
                    }
                }
                window_state.needs_redraw = true;
            }

            // Queue font rebuild if needed
            if changes.font {
                window_state.pending_font_rebuild = true;
            }

            // Reinitialize shader watcher if shader paths changed
            if changes.needs_watcher_reinit() {
                window_state.reinit_shader_watcher();
            }

            // Restart refresh tasks when max_fps or inactive_tab_fps changes
            if (changes.max_fps || changes.inactive_tab_fps)
                && let Some(window) = &window_state.window
            {
                for tab in window_state.tab_manager.tabs_mut() {
                    tab.stop_refresh_task();
                    tab.start_refresh_task(
                        Arc::clone(&window_state.runtime),
                        Arc::clone(window),
                        config.max_fps,
                        config.inactive_tab_fps,
                    );
                }
                log::info!("Restarted refresh tasks with max_fps={}", config.max_fps);
            }

            // Update badge state if badge settings changed
            if changes.badge {
                window_state.badge_state.update_config(config);
                window_state.badge_state.mark_dirty();
            }

            // Sync status bar monitor state after config changes
            window_state.status_bar_ui.sync_monitor_state(config);

            // Update pane divider settings on all tabs with pane managers
            // Scale from logical pixels (config) to physical pixels for layout calculations
            let dpi_scale = window_state
                .renderer
                .as_ref()
                .map(|r| r.scale_factor())
                .unwrap_or(1.0);
            let divider_width = config.pane_divider_width.unwrap_or(2.0) * dpi_scale;
            for tab in window_state.tab_manager.tabs_mut() {
                if let Some(pm) = tab.pane_manager_mut() {
                    pm.set_divider_width(divider_width);
                    pm.set_divider_hit_width(config.pane_divider_hit_width * dpi_scale);
                }
            }

            // Resync triggers from config into core registry for all tabs
            for tab in window_state.tab_manager.tabs_mut() {
                if let Ok(term) = tab.terminal.try_lock() {
                    tab.trigger_security = term.sync_triggers(&config.triggers);
                }
            }

            // Rebuild prettifier pipelines for all tabs when config changes.
            if changes.prettifier_changed {
                for tab in window_state.tab_manager.tabs_mut() {
                    tab.prettifier = crate::prettifier::config_bridge::create_pipeline_from_config(
                        config,
                        config.cols,
                        None,
                    );
                }
            }

            // Invalidate cache
            if let Some(tab) = window_state.tab_manager.active_tab_mut() {
                tab.cache.cells = None;
            }
            window_state.needs_redraw = true;
        }

        if ai_agent_list_changed
            && let Some(sw) = &mut self.settings_window
            && let Some(ws) = self.windows.values().next()
        {
            sw.settings_ui.available_agent_ids = ws
                .agent_state
                .available_agents
                .iter()
                .map(|a| (a.identity.clone(), a.name.clone()))
                .collect();
        }

        // Restart dynamic profile manager if sources changed
        let dynamic_sources_changed =
            self.config.dynamic_profile_sources != config.dynamic_profile_sources;

        // Also update the shared config
        self.config = config.clone();

        // Restart dynamic profile manager with new sources if they changed
        if dynamic_sources_changed {
            self.dynamic_profile_manager.stop();
            if !config.dynamic_profile_sources.is_empty() {
                self.dynamic_profile_manager
                    .start(&config.dynamic_profile_sources, &self.runtime);
            }
            log::info!(
                "Dynamic profile manager restarted with {} sources",
                config.dynamic_profile_sources.len()
            );
        }

        // Update standalone settings window with shader errors only when a change was attempted
        if let Some(settings_window) = &mut self.settings_window {
            if let Some(result) = last_shader_result {
                settings_window.set_shader_error(result);
            }
            if let Some(result) = last_cursor_shader_result {
                settings_window.set_cursor_shader_error(result);
            }
        }
    }

    /// Apply shader changes from settings window editor
    pub fn apply_shader_from_editor(&mut self, source: &str) -> Result<(), String> {
        let mut last_error = None;

        for window_state in self.windows.values_mut() {
            if let Some(renderer) = &mut window_state.renderer {
                match renderer.reload_shader_from_source(source) {
                    Ok(()) => {
                        window_state.needs_redraw = true;
                        if let Some(window) = &window_state.window {
                            window.request_redraw();
                        }
                    }
                    Err(e) => {
                        last_error = Some(format!("{:#}", e));
                    }
                }
            }
        }

        // Update settings window with error status
        if let Some(settings_window) = &mut self.settings_window {
            if let Some(ref err) = last_error {
                settings_window.set_shader_error(Some(err.clone()));
            } else {
                settings_window.clear_shader_error();
            }
        }

        last_error.map_or(Ok(()), Err)
    }

    /// Apply cursor shader changes from settings window editor
    pub fn apply_cursor_shader_from_editor(&mut self, source: &str) -> Result<(), String> {
        let mut last_error = None;

        for window_state in self.windows.values_mut() {
            if let Some(renderer) = &mut window_state.renderer {
                match renderer.reload_cursor_shader_from_source(source) {
                    Ok(()) => {
                        window_state.needs_redraw = true;
                        if let Some(window) = &window_state.window {
                            window.request_redraw();
                        }
                    }
                    Err(e) => {
                        last_error = Some(format!("{:#}", e));
                    }
                }
            }
        }

        // Update settings window with error status
        if let Some(settings_window) = &mut self.settings_window {
            if let Some(ref err) = last_error {
                settings_window.set_cursor_shader_error(Some(err.clone()));
            } else {
                settings_window.clear_cursor_shader_error();
            }
        }

        last_error.map_or(Ok(()), Err)
    }

    /// Request redraw for settings window
    pub fn request_settings_redraw(&self) {
        if let Some(settings_window) = &self.settings_window {
            settings_window.request_redraw();
        }
    }

    /// Start a coprocess by config index on the focused window's active tab.
    pub fn start_coprocess(&mut self, config_index: usize) {
        log::debug!("start_coprocess called with index {}", config_index);
        let focused = self.get_focused_window_id();
        if let Some(window_id) = focused
            && let Some(ws) = self.windows.get_mut(&window_id)
            && let Some(tab) = ws.tab_manager.active_tab_mut()
        {
            if config_index >= ws.config.coprocesses.len() {
                log::warn!("Coprocess config index {} out of range", config_index);
                return;
            }
            let coproc_config = &ws.config.coprocesses[config_index];
            let core_config = par_term_emu_core_rust::coprocess::CoprocessConfig {
                command: coproc_config.command.clone(),
                args: coproc_config.args.clone(),
                cwd: None,
                env: crate::terminal::coprocess_env(),
                copy_terminal_output: coproc_config.copy_terminal_output,
                restart_policy: coproc_config.restart_policy.to_core(),
                restart_delay_ms: coproc_config.restart_delay_ms,
            };
            // Use blocking_lock since this is an infrequent user-initiated operation
            let term = tab.terminal.blocking_lock();
            match term.start_coprocess(core_config) {
                Ok(id) => {
                    log::info!("Started coprocess '{}' (id={})", coproc_config.name, id);
                    // Ensure coprocess_ids vec is large enough
                    while tab.coprocess_ids.len() <= config_index {
                        tab.coprocess_ids.push(None);
                    }
                    tab.coprocess_ids[config_index] = Some(id);
                }
                Err(e) => {
                    let err_msg = format!("Failed to start: {}", e);
                    log::error!("Failed to start coprocess '{}': {}", coproc_config.name, e);
                    // Show error in settings UI
                    if let Some(sw) = &mut self.settings_window {
                        let errors = &mut sw.settings_ui.coprocess_errors;
                        while errors.len() <= config_index {
                            errors.push(String::new());
                        }
                        errors[config_index] = err_msg;
                        sw.request_redraw();
                    }
                    return;
                }
            }
            drop(term);
            // Update running state in settings window
            self.sync_coprocess_running_state();
        } else {
            log::warn!("start_coprocess: no focused window or active tab found");
        }
    }

    /// Stop a coprocess by config index on the focused window's active tab.
    pub fn stop_coprocess(&mut self, config_index: usize) {
        log::debug!("stop_coprocess called with index {}", config_index);
        let focused = self.get_focused_window_id();
        if let Some(window_id) = focused
            && let Some(ws) = self.windows.get_mut(&window_id)
            && let Some(tab) = ws.tab_manager.active_tab_mut()
        {
            if let Some(Some(id)) = tab.coprocess_ids.get(config_index).copied() {
                // Use blocking_lock since this is an infrequent user-initiated operation
                let term = tab.terminal.blocking_lock();
                if let Err(e) = term.stop_coprocess(id) {
                    log::error!("Failed to stop coprocess at index {}: {}", config_index, e);
                } else {
                    log::info!("Stopped coprocess at index {} (id={})", config_index, id);
                }
                drop(term);
                tab.coprocess_ids[config_index] = None;
            }
            // Update running state in settings window
            self.sync_coprocess_running_state();
        }
    }

    /// Maximum number of output lines kept per coprocess in the UI.
    const COPROCESS_OUTPUT_MAX_LINES: usize = 200;

    /// Sync coprocess running state to the settings window.
    pub fn sync_coprocess_running_state(&mut self) {
        let focused = self.get_focused_window_id();
        let (running_state, error_state, new_output): (Vec<bool>, Vec<String>, Vec<Vec<String>>) =
            if let Some(window_id) = focused
                && let Some(ws) = self.windows.get(&window_id)
                && let Some(tab) = ws.tab_manager.active_tab()
            {
                if let Ok(term) = tab.terminal.try_lock() {
                    let mut running = Vec::new();
                    let mut errors = Vec::new();
                    let mut output = Vec::new();
                    for (i, _) in ws.config.coprocesses.iter().enumerate() {
                        let has_id = tab.coprocess_ids.get(i).and_then(|opt| opt.as_ref());
                        let is_running =
                            has_id.is_some_and(|id| term.coprocess_status(*id).unwrap_or(false));
                        // If coprocess has an id but is not running, check stderr.
                        // If no id (never started or start failed), preserve existing error.
                        let err_text = if let Some(id) = has_id {
                            if is_running {
                                String::new()
                            } else {
                                term.read_coprocess_errors(*id)
                                    .unwrap_or_default()
                                    .join("\n")
                            }
                        } else if let Some(sw) = &self.settings_window
                            && let Some(existing) = sw.settings_ui.coprocess_errors.get(i)
                            && !existing.is_empty()
                        {
                            existing.clone()
                        } else {
                            String::new()
                        };
                        // Drain stdout buffer from the core
                        let lines = if let Some(id) = has_id {
                            term.read_from_coprocess(*id).unwrap_or_default()
                        } else {
                            Vec::new()
                        };
                        running.push(is_running);
                        errors.push(err_text);
                        output.push(lines);
                    }
                    (running, errors, output)
                } else {
                    (Vec::new(), Vec::new(), Vec::new())
                }
            } else {
                (Vec::new(), Vec::new(), Vec::new())
            };
        if let Some(sw) = &mut self.settings_window {
            let running_changed = sw.settings_ui.coprocess_running != running_state;
            let errors_changed = sw.settings_ui.coprocess_errors != error_state;
            let has_new_output = new_output.iter().any(|lines| !lines.is_empty());

            // Ensure output/expanded vecs are the right size
            let count = running_state.len();
            sw.settings_ui.coprocess_output.resize_with(count, Vec::new);
            sw.settings_ui
                .coprocess_output_expanded
                .resize(count, false);

            // Append new output lines, capping at max
            for (i, lines) in new_output.into_iter().enumerate() {
                if !lines.is_empty() {
                    let buf = &mut sw.settings_ui.coprocess_output[i];
                    buf.extend(lines);
                    let overflow = buf.len().saturating_sub(Self::COPROCESS_OUTPUT_MAX_LINES);
                    if overflow > 0 {
                        buf.drain(..overflow);
                    }
                }
            }

            if running_changed || errors_changed || has_new_output {
                sw.settings_ui.coprocess_running = running_state;
                sw.settings_ui.coprocess_errors = error_state;
                sw.request_redraw();
            }
        }
    }

    // ========================================================================
    // Script Methods
    // ========================================================================

    /// Start a script by config index on the focused window's active tab.
    pub fn start_script(&mut self, config_index: usize) {
        crate::debug_info!(
            "SCRIPT",
            "start_script called with config_index={}",
            config_index
        );
        let focused = self.get_focused_window_id();
        if let Some(window_id) = focused
            && let Some(ws) = self.windows.get_mut(&window_id)
            && let Some(tab) = ws.tab_manager.active_tab_mut()
        {
            crate::debug_info!(
                "SCRIPT",
                "start_script: ws.config.scripts.len()={}, tab.script_ids.len()={}",
                ws.config.scripts.len(),
                tab.script_ids.len()
            );
            if config_index >= ws.config.scripts.len() {
                crate::debug_error!(
                    "SCRIPT",
                    "Script config index {} out of range (scripts.len={})",
                    config_index,
                    ws.config.scripts.len()
                );
                return;
            }
            let script_config = &ws.config.scripts[config_index];
            crate::debug_info!(
                "SCRIPT",
                "start_script: found config name='{}' path='{}' enabled={} args={:?}",
                script_config.name,
                script_config.script_path,
                script_config.enabled,
                script_config.args
            );
            if !script_config.enabled {
                crate::debug_info!(
                    "SCRIPT",
                    "Script '{}' is disabled, not starting",
                    script_config.name
                );
                return;
            }

            // Build subscription filter from config
            let subscription_filter = if script_config.subscriptions.is_empty() {
                None
            } else {
                Some(
                    script_config
                        .subscriptions
                        .iter()
                        .cloned()
                        .collect::<std::collections::HashSet<String>>(),
                )
            };

            // Create the event forwarder and register it as an observer
            let forwarder = std::sync::Arc::new(
                crate::scripting::observer::ScriptEventForwarder::new(subscription_filter),
            );

            // Register observer with terminal (user-initiated, use blocking_lock)
            let observer_id = {
                let term = tab.terminal.blocking_lock();
                term.add_observer(forwarder.clone())
            };

            // Start the script process
            crate::debug_info!("SCRIPT", "start_script: spawning process...");
            match tab.script_manager.start_script(script_config) {
                Ok(script_id) => {
                    crate::debug_info!(
                        "SCRIPT",
                        "start_script: SUCCESS script_id={} observer_id={:?}",
                        script_id,
                        observer_id
                    );

                    // Ensure vecs are large enough
                    while tab.script_ids.len() <= config_index {
                        tab.script_ids.push(None);
                    }
                    while tab.script_observer_ids.len() <= config_index {
                        tab.script_observer_ids.push(None);
                    }
                    while tab.script_forwarders.len() <= config_index {
                        tab.script_forwarders.push(None);
                    }

                    tab.script_ids[config_index] = Some(script_id);
                    tab.script_observer_ids[config_index] = Some(observer_id);
                    tab.script_forwarders[config_index] = Some(forwarder);
                }
                Err(e) => {
                    let err_msg = format!("Failed to start: {}", e);
                    crate::debug_error!(
                        "SCRIPT",
                        "start_script: FAILED to start '{}': {}",
                        script_config.name,
                        e
                    );

                    // Remove observer since script failed to start
                    let term = tab.terminal.blocking_lock();
                    term.remove_observer(observer_id);
                    drop(term);

                    // Show error in settings UI
                    if let Some(sw) = &mut self.settings_window {
                        let errors = &mut sw.settings_ui.script_errors;
                        while errors.len() <= config_index {
                            errors.push(String::new());
                        }
                        errors[config_index] = err_msg;
                        sw.request_redraw();
                    }
                    return;
                }
            }
            // Update running state in settings window
            self.sync_script_running_state();
        } else {
            crate::debug_error!(
                "SCRIPT",
                "start_script: no focused window or active tab found"
            );
        }
    }

    /// Stop a script by config index on the focused window's active tab.
    pub fn stop_script(&mut self, config_index: usize) {
        log::debug!("stop_script called with index {}", config_index);
        let focused = self.get_focused_window_id();
        if let Some(window_id) = focused
            && let Some(ws) = self.windows.get_mut(&window_id)
            && let Some(tab) = ws.tab_manager.active_tab_mut()
        {
            // Stop the script process
            if let Some(Some(script_id)) = tab.script_ids.get(config_index).copied() {
                tab.script_manager.stop_script(script_id);
                log::info!(
                    "Stopped script at index {} (id={})",
                    config_index,
                    script_id
                );
            }

            // Remove observer from terminal
            if let Some(Some(observer_id)) = tab.script_observer_ids.get(config_index).copied() {
                let term = tab.terminal.blocking_lock();
                term.remove_observer(observer_id);
                drop(term);
            }

            // Clear tracking state
            if let Some(slot) = tab.script_ids.get_mut(config_index) {
                *slot = None;
            }
            if let Some(slot) = tab.script_observer_ids.get_mut(config_index) {
                *slot = None;
            }
            if let Some(slot) = tab.script_forwarders.get_mut(config_index) {
                *slot = None;
            }

            // Update running state in settings window
            self.sync_script_running_state();
        }
    }

    /// Maximum number of output lines kept per script in the UI.
    const SCRIPT_OUTPUT_MAX_LINES: usize = 200;

    /// Sync script running state to the settings window.
    ///
    /// Drains events from forwarders, sends them to scripts, reads commands
    /// and errors back, and updates the settings UI state.
    pub fn sync_script_running_state(&mut self) {
        let focused = self.get_focused_window_id();

        // Collect state from the active tab
        #[allow(clippy::type_complexity)]
        let (running_state, error_state, new_output, panel_state): (
            Vec<bool>,
            Vec<String>,
            Vec<Vec<String>>,
            Vec<Option<(String, String)>>,
        ) = if let Some(window_id) = focused
            && let Some(ws) = self.windows.get_mut(&window_id)
            && let Some(tab) = ws.tab_manager.active_tab_mut()
        {
            let script_count = ws.config.scripts.len();
            let mut running = Vec::with_capacity(script_count);
            let mut errors = Vec::with_capacity(script_count);
            let mut output = Vec::with_capacity(script_count);
            let mut panels = Vec::with_capacity(script_count);

            for i in 0..script_count {
                let has_script_id = tab.script_ids.get(i).and_then(|opt| *opt);
                let is_running = has_script_id.is_some_and(|id| tab.script_manager.is_running(id));

                // Drain events from forwarder and send to script
                if is_running && let Some(Some(forwarder)) = tab.script_forwarders.get(i) {
                    let events = forwarder.drain_events();
                    if let Some(script_id) = has_script_id {
                        for event in &events {
                            let _ = tab.script_manager.send_event(script_id, event);
                        }
                    }
                }

                // Read commands from script and process them
                let mut log_lines = Vec::new();
                let mut panel_val = tab
                    .script_manager
                    .get_panel(has_script_id.unwrap_or(0))
                    .cloned();

                if let Some(script_id) = has_script_id {
                    let commands = tab.script_manager.read_commands(script_id);
                    for cmd in commands {
                        match cmd {
                            crate::scripting::protocol::ScriptCommand::Log { level, message } => {
                                log_lines.push(format!("[{}] {}", level, message));
                            }
                            crate::scripting::protocol::ScriptCommand::SetPanel {
                                title,
                                content,
                            } => {
                                tab.script_manager.set_panel(
                                    script_id,
                                    title.clone(),
                                    content.clone(),
                                );
                                panel_val = Some((title, content));
                            }
                            crate::scripting::protocol::ScriptCommand::ClearPanel {} => {
                                tab.script_manager.clear_panel(script_id);
                                panel_val = None;
                            }
                            // TODO: Implement WriteText, Notify, SetBadge, SetVariable,
                            // RunCommand, ChangeConfig — these require proper access to the
                            // terminal and config systems. Will be completed after the basic
                            // infrastructure is working.
                            _ => {
                                log::debug!("Script command not yet implemented: {:?}", cmd);
                            }
                        }
                    }
                }

                // Read errors from script
                let err_text = if let Some(script_id) = has_script_id {
                    if is_running {
                        // Drain any stderr lines even while running
                        let err_lines = tab.script_manager.read_errors(script_id);
                        if !err_lines.is_empty() {
                            err_lines.join("\n")
                        } else {
                            String::new()
                        }
                    } else {
                        let err_lines = tab.script_manager.read_errors(script_id);
                        err_lines.join("\n")
                    }
                } else if let Some(sw) = &self.settings_window
                    && let Some(existing) = sw.settings_ui.script_errors.get(i)
                    && !existing.is_empty()
                {
                    existing.clone()
                } else {
                    String::new()
                };

                running.push(is_running);
                errors.push(err_text);
                output.push(log_lines);
                panels.push(panel_val);
            }

            (running, errors, output, panels)
        } else {
            (Vec::new(), Vec::new(), Vec::new(), Vec::new())
        };

        // Update settings window state
        if let Some(sw) = &mut self.settings_window {
            let running_changed = sw.settings_ui.script_running != running_state;
            let errors_changed = sw.settings_ui.script_errors != error_state;
            let has_new_output = new_output.iter().any(|lines| !lines.is_empty());
            let panels_changed = sw.settings_ui.script_panels != panel_state;

            if running_changed || errors_changed {
                crate::debug_info!(
                    "SCRIPT",
                    "sync: state change - running={:?} errors_changed={}",
                    running_state,
                    errors_changed
                );
            }

            let count = running_state.len();
            sw.settings_ui.script_output.resize_with(count, Vec::new);
            sw.settings_ui.script_output_expanded.resize(count, false);
            sw.settings_ui.script_panels.resize_with(count, || None);

            // Append new output lines, capping at max
            for (i, lines) in new_output.into_iter().enumerate() {
                if !lines.is_empty() {
                    let buf = &mut sw.settings_ui.script_output[i];
                    buf.extend(lines);
                    let overflow = buf.len().saturating_sub(Self::SCRIPT_OUTPUT_MAX_LINES);
                    if overflow > 0 {
                        buf.drain(..overflow);
                    }
                }
            }

            if running_changed || errors_changed || has_new_output || panels_changed {
                sw.settings_ui.script_running = running_state;
                sw.settings_ui.script_errors = error_state;
                sw.settings_ui.script_panels = panel_state;
                sw.request_redraw();
            }
        }
    }

    // ========================================================================
    // Window Arrangement Methods
    // ========================================================================

    /// Create a new window with specific position and size overrides.
    ///
    /// Unlike `create_window()`, this skips `apply_window_positioning()` and
    /// places the window at the exact specified position and size.
    /// Additional tabs (beyond the first) are created with the given CWDs.
    pub fn create_window_with_overrides(
        &mut self,
        event_loop: &ActiveEventLoop,
        position: (i32, i32),
        size: (u32, u32),
        tab_cwds: &[Option<String>],
        active_tab_index: usize,
    ) -> Option<WindowId> {
        use winit::window::Window;

        // Reload config from disk to pick up any changes
        if let Ok(fresh_config) = Config::load() {
            self.config = fresh_config;
        }

        // Build window title
        let window_number = self.windows.len() + 1;
        let title = if self.config.show_window_number {
            format!("{} [{}]", self.config.window_title, window_number)
        } else {
            self.config.window_title.clone()
        };

        // position and size are in logical pixels (scale-factor-independent).
        // Using LogicalPosition/LogicalSize lets winit apply the correct
        // per-monitor DPI conversion, which is critical for mixed-DPI
        // multi-monitor setups (e.g. Retina laptop + 1x external display).
        let mut window_attrs = Window::default_attributes()
            .with_title(&title)
            .with_inner_size(winit::dpi::LogicalSize::new(size.0 as f64, size.1 as f64))
            .with_position(winit::dpi::LogicalPosition::new(
                position.0 as f64,
                position.1 as f64,
            ))
            .with_decorations(self.config.window_decorations);

        if self.config.lock_window_size {
            window_attrs = window_attrs.with_resizable(false);
        }

        // Load and set the application icon
        let icon_bytes = include_bytes!("../../assets/icon.png");
        if let Ok(icon_image) = image::load_from_memory(icon_bytes) {
            let rgba = icon_image.to_rgba8();
            let (w, h) = rgba.dimensions();
            if let Ok(icon) = winit::window::Icon::from_rgba(rgba.into_raw(), w, h) {
                window_attrs = window_attrs.with_window_icon(Some(icon));
            }
        }

        if self.config.window_always_on_top {
            window_attrs = window_attrs.with_window_level(winit::window::WindowLevel::AlwaysOnTop);
        }

        window_attrs = window_attrs.with_transparent(true);

        match event_loop.create_window(window_attrs) {
            Ok(window) => {
                let window_id = window.id();
                let mut window_state =
                    WindowState::new(self.config.clone(), Arc::clone(&self.runtime));
                window_state.window_index = window_number;

                // Extract the first tab's CWD to pass during initialization
                // (the shell must be spawned in the correct directory)
                let first_tab_cwd = tab_cwds.first().and_then(|c| c.clone());

                let runtime = Arc::clone(&self.runtime);
                if let Err(e) =
                    runtime.block_on(window_state.initialize_async(window, first_tab_cwd))
                {
                    log::error!("Failed to initialize arranged window: {}", e);
                    return None;
                }

                // Initialize menu for first window or attach to additional
                if self.menu.is_none() {
                    match MenuManager::new() {
                        Ok(menu) => {
                            if let Some(win) = &window_state.window
                                && let Err(e) = menu.init_for_window(win)
                            {
                                log::warn!("Failed to initialize menu: {}", e);
                            }
                            self.menu = Some(menu);
                        }
                        Err(e) => {
                            log::warn!("Failed to create menu: {}", e);
                        }
                    }
                } else if let Some(menu) = &self.menu
                    && let Some(win) = &window_state.window
                    && let Err(e) = menu.init_for_window(win)
                {
                    log::warn!("Failed to initialize menu for window: {}", e);
                }

                // Set the position explicitly (in case the WM overrode it).
                // Use LogicalPosition so the per-monitor scale conversion is
                // applied correctly (matches with_position above).
                if let Some(win) = &window_state.window {
                    win.set_outer_position(winit::dpi::LogicalPosition::new(
                        position.0 as f64,
                        position.1 as f64,
                    ));
                }

                // Create remaining tabs (first tab was already created with CWD)
                let grid_size = window_state.renderer.as_ref().map(|r| r.grid_size());
                for cwd in tab_cwds.iter().skip(1) {
                    if let Err(e) = window_state.tab_manager.new_tab_with_cwd(
                        &self.config,
                        Arc::clone(&self.runtime),
                        cwd.clone(),
                        grid_size,
                    ) {
                        log::warn!("Failed to create tab in arranged window: {}", e);
                    }
                }

                // Switch to the saved active tab (switch_to_index is 1-based)
                window_state
                    .tab_manager
                    .switch_to_index(active_tab_index + 1);

                // Start refresh tasks for all tabs
                if let Some(win) = &window_state.window {
                    for tab in window_state.tab_manager.tabs_mut() {
                        tab.start_refresh_task(
                            Arc::clone(&self.runtime),
                            Arc::clone(win),
                            self.config.max_fps,
                            self.config.inactive_tab_fps,
                        );
                    }
                }

                self.windows.insert(window_id, window_state);
                self.pending_window_count += 1;

                // Sync existing update state to new window's status bar and dialog
                let update_version = self
                    .last_update_result
                    .as_ref()
                    .and_then(update_available_version);
                let update_result_clone = self.last_update_result.clone();
                let install_type = self.detect_installation_type();
                if let Some(ws) = self.windows.get_mut(&window_id) {
                    ws.status_bar_ui.update_available_version = update_version;
                    ws.last_update_result = update_result_clone;
                    ws.installation_type = install_type;
                }

                if self.start_time.is_none() {
                    self.start_time = Some(Instant::now());
                }

                log::info!(
                    "Created arranged window {:?} at ({}, {}) size {}x{} with {} tabs",
                    window_id,
                    position.0,
                    position.1,
                    size.0,
                    size.1,
                    tab_cwds.len().max(1),
                );

                Some(window_id)
            }
            Err(e) => {
                log::error!("Failed to create arranged window: {}", e);
                None
            }
        }
    }

    /// Save the current window layout as an arrangement
    pub fn save_arrangement(&mut self, name: String, event_loop: &ActiveEventLoop) {
        // Remove existing arrangement with the same name (case-insensitive) to allow overwrite
        if let Some(existing) = self.arrangement_manager.find_by_name(&name) {
            let existing_id = existing.id;
            self.arrangement_manager.remove(&existing_id);
            log::info!("Overwriting existing arrangement '{}'", name);
        }

        let arrangement =
            arrangements::capture::capture_arrangement(name.clone(), &self.windows, event_loop);
        log::info!(
            "Saved arrangement '{}' with {} windows",
            name,
            arrangement.windows.len()
        );
        self.arrangement_manager.add(arrangement);
        if let Err(e) = arrangements::storage::save_arrangements(&self.arrangement_manager) {
            log::error!("Failed to save arrangements: {}", e);
        }
        self.sync_arrangements_to_settings();
    }

    /// Restore a saved arrangement by ID.
    ///
    /// Closes all existing windows and creates new ones according to the arrangement.
    pub fn restore_arrangement(&mut self, id: ArrangementId, event_loop: &ActiveEventLoop) {
        let arrangement = match self.arrangement_manager.get(&id) {
            Some(a) => a.clone(),
            None => {
                log::error!("Arrangement not found: {}", id);
                return;
            }
        };

        log::info!(
            "Restoring arrangement '{}' ({} windows)",
            arrangement.name,
            arrangement.windows.len()
        );

        // Close all existing windows
        let window_ids: Vec<WindowId> = self.windows.keys().copied().collect();
        for window_id in window_ids {
            if let Some(window_state) = self.windows.remove(&window_id) {
                drop(window_state);
            }
        }

        // Build monitor mapping
        let available_monitors: Vec<_> = event_loop.available_monitors().collect();
        let monitor_mapping = arrangements::restore::build_monitor_mapping(
            &arrangement.monitor_layout,
            &available_monitors,
        );

        // Create windows from arrangement
        for (i, window_snapshot) in arrangement.windows.iter().enumerate() {
            let Some((x, y, w, h)) = arrangements::restore::compute_restore_position(
                window_snapshot,
                &monitor_mapping,
                &available_monitors,
            ) else {
                log::warn!("Could not compute position for window {} in arrangement", i);
                continue;
            };

            let tab_cwds = arrangements::restore::tab_cwds(&arrangement, i);
            let created_window_id = self.create_window_with_overrides(
                event_loop,
                (x, y),
                (w, h),
                &tab_cwds,
                window_snapshot.active_tab_index,
            );

            // Restore user titles, custom colors, and icons from arrangement
            if let Some(window_id) = created_window_id
                && let Some(window_state) = self.windows.get_mut(&window_id)
            {
                let tabs = window_state.tab_manager.tabs_mut();
                for (tab_idx, snapshot) in window_snapshot.tabs.iter().enumerate() {
                    if let Some(tab) = tabs.get_mut(tab_idx) {
                        if let Some(ref user_title) = snapshot.user_title {
                            tab.title = user_title.clone();
                            tab.user_named = true;
                            tab.has_default_title = false;
                        }
                        if let Some(color) = snapshot.custom_color {
                            tab.set_custom_color(color);
                        }
                        if let Some(ref icon) = snapshot.custom_icon {
                            tab.custom_icon = Some(icon.clone());
                        }
                    }
                }
            }
        }

        // If no windows were created (e.g., empty arrangement), create one default window
        if self.windows.is_empty() {
            log::warn!("Arrangement had no restorable windows, creating default window");
            self.create_window(event_loop);
        }
    }

    /// Restore an arrangement by name (for auto-restore and keybinding actions)
    pub fn restore_arrangement_by_name(
        &mut self,
        name: &str,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        if let Some(arrangement) = self.arrangement_manager.find_by_name(name) {
            let id = arrangement.id;
            self.restore_arrangement(id, event_loop);
            true
        } else {
            log::warn!("Arrangement not found by name: {}", name);
            false
        }
    }

    /// Delete an arrangement by ID
    pub fn delete_arrangement(&mut self, id: ArrangementId) {
        if let Some(removed) = self.arrangement_manager.remove(&id) {
            log::info!("Deleted arrangement '{}'", removed.name);
            if let Err(e) = arrangements::storage::save_arrangements(&self.arrangement_manager) {
                log::error!("Failed to save arrangements after delete: {}", e);
            }
            self.sync_arrangements_to_settings();
        }
    }

    /// Rename an arrangement by ID
    pub fn rename_arrangement(&mut self, id: ArrangementId, new_name: String) {
        if let Some(arrangement) = self.arrangement_manager.get_mut(&id) {
            log::info!(
                "Renamed arrangement '{}' -> '{}'",
                arrangement.name,
                new_name
            );
            arrangement.name = new_name;
            if let Err(e) = arrangements::storage::save_arrangements(&self.arrangement_manager) {
                log::error!("Failed to save arrangements after rename: {}", e);
            }
            self.sync_arrangements_to_settings();
        }
    }

    /// Move an arrangement up in the order
    pub fn move_arrangement_up(&mut self, id: ArrangementId) {
        self.arrangement_manager.move_up(&id);
        if let Err(e) = arrangements::storage::save_arrangements(&self.arrangement_manager) {
            log::error!("Failed to save arrangements after reorder: {}", e);
        }
        self.sync_arrangements_to_settings();
    }

    /// Move an arrangement down in the order
    pub fn move_arrangement_down(&mut self, id: ArrangementId) {
        self.arrangement_manager.move_down(&id);
        if let Err(e) = arrangements::storage::save_arrangements(&self.arrangement_manager) {
            log::error!("Failed to save arrangements after reorder: {}", e);
        }
        self.sync_arrangements_to_settings();
    }

    /// Sync arrangement manager data to the settings window (for UI display)
    pub fn sync_arrangements_to_settings(&mut self) {
        if let Some(sw) = &mut self.settings_window {
            sw.settings_ui.arrangement_manager = self.arrangement_manager.clone();
        }
    }

    pub fn send_test_notification(&self) {
        log::info!("Sending test notification");

        #[cfg(not(target_os = "macos"))]
        {
            use notify_rust::Notification;
            if let Err(e) = Notification::new()
                .summary("par-term Test Notification")
                .body("If you see this, notifications are working!")
                .timeout(notify_rust::Timeout::Milliseconds(5000))
                .show()
            {
                log::warn!("Failed to send test notification: {}", e);
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS notifications via osascript
            let script = r#"display notification "If you see this, notifications are working!" with title "par-term Test Notification""#;

            if let Err(e) = std::process::Command::new("osascript")
                .arg("-e")
                .arg(script)
                .output()
            {
                log::warn!("Failed to send macOS test notification: {}", e);
            }
        }
    }
}

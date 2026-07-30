//! Session save/restore and arranged-window creation for WindowManager.
//!
//! Contains:
//! - `save_session_state_background`: capture in-memory session state and
//!   write it to disk on a background thread.
//! - `publish_crash_snapshot`: hand a known-good session snapshot to the panic
//!   boundary so a crash does not take every window's tabs with it.
//! - `restore_session`: load a saved session and recreate its windows.
//! - `create_window_with_overrides`: create a window at an exact position/size,
//!   bypassing the normal `apply_window_positioning()` logic (used during session
//!   restore and arrangement recall).

use std::sync::Arc;
use std::time::Instant;

use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::app::window_state::WindowState;
use crate::config::Config;
use crate::menu::MenuManager;

use super::WindowManager;
use super::update_checker::update_available_version;

/// How often [`WindowManager::publish_crash_snapshot`] re-captures the session
/// for the panic boundary.
///
/// This is the worst-case staleness of a crash-recovered session. Session state
/// only changes when a tab opens or closes, a shell changes directory, or a
/// window moves, so five seconds costs at most one such change — against losing
/// every tab, which is the alternative. The capture itself is cheap: it uses
/// `try_read` on each terminal and falls back to the cached working directory
/// rather than blocking the event loop.
const CRASH_SNAPSHOT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

impl WindowManager {
    /// Save session state on a background thread to avoid blocking the main thread.
    /// Captures state synchronously (fast, in-memory) then spawns disk I/O.
    pub(super) fn save_session_state_background(&self) {
        let state = crate::session::capture::capture_session(&self.windows);
        let _ = std::thread::Builder::new()
            .name("session-save".into())
            .spawn(move || {
                if let Err(e) = crate::session::storage::save_session(&state) {
                    log::error!("Failed to save session state: {}", e);
                }
            });
    }

    /// Hand the panic boundary a known-good snapshot of the current session.
    ///
    /// Call from the event loop, at a point where no par-term structure is
    /// mid-update. The panic hook can only write what has been published here:
    /// it cannot reach the live `WindowManager`, and serializing live state from
    /// a thread that has just panicked mid-mutation would be unsound anyway. See
    /// `crate::session::crash_guard` for the full rationale.
    ///
    /// Throttled to [`CRASH_SNAPSHOT_INTERVAL`], so this is two atomic loads on
    /// the overwhelming majority of event-loop iterations.
    ///
    /// Called from `WindowManager::about_to_wait`. That is the right place
    /// because it runs *between* events, when no window event is part-way
    /// through mutating a `WindowState` — which is the whole premise of
    /// snapshotting rather than serializing live state. The hook can only
    /// write what has been published, so removing that call makes the panic
    /// boundary inert.
    pub fn publish_crash_snapshot(&self) {
        // Gate on the same setting as the normal session save. A user who has
        // turned session restore off should not have a crash file appear in
        // their config directory, and nothing would consume it if it did.
        if !self.config.load().restore_session {
            return;
        }
        // Checked before the throttle, not after: a capture that would yield
        // nothing must not consume the interval, and this is the cheaper test
        // anyway. Never let a transient empty capture — startup before the first
        // window is registered, or teardown after the last one is removed —
        // replace a good snapshot with one that restores nothing.
        if self.windows.is_empty() {
            return;
        }
        if !crate::session::crash_guard::snapshot_is_due(CRASH_SNAPSHOT_INTERVAL) {
            return;
        }

        let state = crate::session::capture::capture_session(&self.windows);
        // `capture_session` also skips a `WindowState` whose window is not yet
        // created, so the map being non-empty is not on its own enough.
        if state.windows.is_empty() {
            return;
        }
        crate::session::crash_guard::publish(&state);
    }

    /// Restore windows from the last saved session.
    ///
    /// Prefers a crash session file when one is present: the normal session file
    /// is only written on a clean exit, so after a panic it is stale or missing
    /// entirely, while the crash file holds what was open seconds before the
    /// panic. Either file is consumed on use.
    ///
    /// Returns true if session was successfully restored, false otherwise.
    pub fn restore_session(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let recovered_from_crash = crate::session::crash_guard::take_crash_session();
        let restored_after_crash = recovered_from_crash.is_some();

        let session = match recovered_from_crash {
            Some(session) => {
                // The panic report from the run that produced this file is in
                // the rotated log (`<log>.1`), not the live one — `debug.rs`
                // rolls the previous log aside on open rather than truncating.
                log::warn!(
                    "Recovering session from a crash snapshot ({} windows) taken at {}. \
                     The previous run ended in a panic.",
                    session.windows.len(),
                    session.saved_at
                );
                session
            }
            None => match crate::session::storage::load_session() {
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
            },
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
            // When a tmux session is saved, the visible tabs are tmux display tabs that
            // will be re-created by the tmux session on reconnect.  Pass only a single
            // empty tab CWD so create_window_with_overrides spawns just the gateway
            // shell; the real tmux tabs arrive via layout-change notifications.
            let tab_cwds: Vec<Option<String>> = if session_window.tmux_session_name.is_some() {
                vec![None]
            } else {
                session_window
                    .tabs
                    .iter()
                    .map(|tab| crate::session::restore::validate_cwd(&tab.snapshot.cwd))
                    .collect()
            };

            let created_window_id = self.create_window_with_overrides(
                event_loop,
                session_window.position,
                session_window.size,
                &tab_cwds,
                session_window.active_tab_index,
            );

            if let Some(window_id) = created_window_id
                && let Some(window_state) = self.windows.get_mut(&window_id)
            {
                // Auto-reconnect tmux session if one was active at save time
                if let Some(ref session_name) = session_window.tmux_session_name
                    && window_state.config.load().tmux_enabled
                    && !session_name.is_empty()
                {
                    if let Err(e) = window_state.initiate_tmux_gateway(Some(session_name)) {
                        log::warn!("Session restore: tmux auto-connect failed: {}", e);
                    }
                } else {
                    // Non-tmux window: restore pane layouts, user titles, custom colors, icons
                    let tabs = window_state.tab_manager.tabs_mut();
                    for (tab_idx, session_tab) in session_window.tabs.iter().enumerate() {
                        if let Some(ref layout) = session_tab.pane_layout
                            && let Some(tab) = tabs.get_mut(tab_idx)
                            && matches!(layout, crate::session::SessionPaneNode::Split { .. })
                        {
                            tab.restore_pane_layout(
                                layout,
                                &self.config.load(),
                                Arc::clone(&self.runtime),
                            );
                        }
                    }
                    for (tab_idx, session_tab) in session_window.tabs.iter().enumerate() {
                        if let Some(tab) = tabs.get_mut(tab_idx) {
                            if let Some(ref user_title) = session_tab.snapshot.user_title {
                                tab.set_title(user_title);
                                tab.user_named = true;
                            }
                            if let Some(color) = session_tab.snapshot.custom_color {
                                tab.set_custom_color(color);
                            }
                            if let Some(ref icon) = session_tab.snapshot.custom_icon {
                                tab.custom_icon = Some(icon.clone());
                            }
                        }
                    }
                    // Start refresh tasks for restored pane layouts so secondary
                    // panes trigger redraws when they receive output.
                    if let Some(win) = &window_state.window {
                        for tab in window_state.tab_manager.tabs_mut() {
                            tab.start_pane_refresh_tasks(
                                Arc::clone(&self.runtime),
                                Arc::clone(win),
                                self.config.load().max_fps,
                                self.config.load().inactive_tab_fps,
                            );
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

        // Say why the tabs came back, and what did not come with them. A
        // recovered session is otherwise indistinguishable from a normal one, so
        // silently reopening everything after a crash reads as par-term having
        // ignored the crash rather than having rescued the workspace — and the
        // user has no way to know the scrollback is gone until they scroll.
        if restored_after_crash {
            for window_state in self.windows.values_mut() {
                window_state
                    .show_toast("Recovered session after a crash — scrollback not restored");
            }
        }

        true
    }

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
            self.config.store(Arc::new(fresh_config));
        }

        // Build window title
        let window_number = self.windows.len() + 1;
        let title = if self.config.load().show_window_number {
            format!("{} [{}]", self.config.load().window_title, window_number)
        } else {
            self.config.load().window_title.clone()
        };

        // position and size are in logical pixels (scale-factor-independent).
        let mut window_attrs = Window::default_attributes()
            .with_title(&title)
            .with_inner_size(winit::dpi::LogicalSize::new(size.0 as f64, size.1 as f64))
            .with_position(winit::dpi::LogicalPosition::new(
                position.0 as f64,
                position.1 as f64,
            ))
            .with_decorations(self.config.load().window.window_decorations);

        if self.config.load().lock_window_size {
            window_attrs = window_attrs.with_resizable(false);
        }

        // Load and set the application icon
        let icon_bytes = include_bytes!("../../../assets/icon.png");
        if let Ok(icon_image) = image::load_from_memory(icon_bytes) {
            let rgba = icon_image.to_rgba8();
            let (w, h) = rgba.dimensions();
            if let Ok(icon) = winit::window::Icon::from_rgba(rgba.into_raw(), w, h) {
                window_attrs = window_attrs.with_window_icon(Some(icon));
            }
        }

        if self.config.load().window.window_always_on_top {
            window_attrs = window_attrs.with_window_level(winit::window::WindowLevel::AlwaysOnTop);
        }

        window_attrs = window_attrs.with_transparent(true);

        // macOS: accept the first mouse click (see window_lifecycle.rs for rationale).
        #[cfg(target_os = "macos")]
        {
            use winit::platform::macos::WindowAttributesExtMacOS as _;
            window_attrs = window_attrs.with_accepts_first_mouse(true);
        }

        match event_loop.create_window(window_attrs) {
            Ok(window) => {
                let window_id = window.id();

                // Initialize menu BEFORE the blocking GPU init (same rationale
                // as create_window — see window_lifecycle.rs for details).
                if self.menu.is_none() {
                    match MenuManager::new() {
                        Ok(menu) => {
                            if let Err(e) = menu.init_global() {
                                log::warn!("Failed to initialize global menu: {}", e);
                            }
                            self.menu = Some(menu);
                        }
                        Err(e) => {
                            log::warn!("Failed to create menu: {}", e);
                        }
                    }
                }

                let mut window_state =
                    WindowState::new((**self.config.load()).clone(), Arc::clone(&self.runtime));
                window_state.window_index = window_number;

                // Extract the first tab's CWD to pass during initialization
                let first_tab_cwd = tab_cwds.first().and_then(|c| c.clone());

                let runtime = Arc::clone(&self.runtime);
                if let Err(e) =
                    runtime.block_on(window_state.initialize_async(window, false, first_tab_cwd))
                {
                    log::error!("Failed to initialize arranged window: {}", e);
                    return None;
                }

                // Attach menu to the window (platform-specific: per-window on Windows/Linux)
                if let Some(menu) = &self.menu
                    && let Some(win) = &window_state.window
                    && let Err(e) = menu.init_for_window(win)
                {
                    log::warn!("Failed to initialize menu for window: {}", e);
                }

                // Set the position explicitly (in case the WM overrode it).
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
                        &self.config.load(),
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

                // Start refresh tasks for all tabs (and their split panes)
                if let Some(win) = &window_state.window {
                    for tab in window_state.tab_manager.tabs_mut() {
                        tab.start_refresh_task(
                            Arc::clone(&self.runtime),
                            Arc::clone(win),
                            self.config.load().max_fps,
                            self.config.load().inactive_tab_fps,
                        );
                        tab.start_pane_refresh_tasks(
                            Arc::clone(&self.runtime),
                            Arc::clone(win),
                            self.config.load().max_fps,
                            self.config.load().inactive_tab_fps,
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
                    ws.update_state.last_result = update_result_clone;
                    ws.update_state.installation_type = install_type;
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
}

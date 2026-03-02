//! Settings window lifecycle and settings-action dispatch.
//!
//! This module contains all `WindowManager` methods that relate to the
//! settings window: opening/closing it, routing window events to it, and
//! dispatching the resulting `SettingsWindowAction` payloads.
//!
//! Config propagation (applying changes from the settings window to all terminal
//! windows) lives in `config_propagation.rs` (R-39), keeping this file focused
//! on lifecycle and dispatch.
//!
//! Relocated from `window_manager/settings.rs` (R-27): the file was renamed
//! to `settings_actions.rs` to reflect that it handles settings *actions*
//! (dispatcher + application), not just settings window *lifecycle*.
//!
//! # Error Handling Convention
//!
//! Functions that can fail for reasons surfaced to the user (e.g., shader
//! compilation errors) return `Result<(), String>` so callers can display
//! the error in the UI. For internal errors that should not escape to UI
//! callers, use `anyhow::Result` or `Option`. New functions should follow
//! the `Result<T, String>` pattern when the error message needs to be
//! displayed to the user.

use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::settings_window::{SettingsWindow, SettingsWindowAction};

use super::WindowManager;
use super::update_checker::to_settings_update_result;

impl WindowManager {
    /// Open the settings window (or focus if already open)
    pub fn open_settings_window(&mut self, event_loop: &ActiveEventLoop) {
        // If already open, bring to front and focus
        if let Some(settings_window) = &self.settings_window {
            settings_window.focus();
            return;
        }

        // Create new settings window using shared runtime
        let config = self.config.clone();
        let runtime = std::sync::Arc::clone(&self.runtime);

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

    // NOTE: apply_config_to_windows is extracted to config_propagation.rs (R-39).
    // It is still accessible as `WindowManager::apply_config_to_windows`.

    /// Apply shader changes from settings window editor
    pub fn apply_shader_from_editor(&mut self, source: &str) -> Result<(), String> {
        let mut last_error = None;

        for window_state in self.windows.values_mut() {
            if let Some(renderer) = &mut window_state.renderer {
                match renderer.reload_shader_from_source(source) {
                    Ok(()) => {
                        window_state.focus_state.needs_redraw = true;
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
                        window_state.focus_state.needs_redraw = true;
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
}

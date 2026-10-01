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

use par_term_settings_ui::{ShaderInstallResult, ShaderUninstallResult};

use crate::config::resolve_shader_config;
use crate::settings_window::{SettingsWindow, SettingsWindowAction};

use super::WindowManager;
use super::update_checker::to_settings_update_result;

/// Wrapper that adapts `shader_installer::install_shaders_with_manifest` to the
/// `ShaderInstallResult` type expected by the settings UI callback.
fn shader_install_wrapper(force: bool) -> Result<ShaderInstallResult, String> {
    let r = crate::shader_installer::install_shaders_with_manifest(force)?;
    Ok(ShaderInstallResult {
        installed: r.installed,
        skipped: r.skipped,
        removed: r.removed,
    })
}

/// Wrapper that adapts `shader_installer::uninstall_shaders` to the
/// `ShaderUninstallResult` type expected by the settings UI callback.
fn shader_uninstall_wrapper(force: bool) -> Result<ShaderUninstallResult, String> {
    let r = crate::shader_installer::uninstall_shaders(force)?;
    Ok(ShaderUninstallResult {
        removed: r.removed,
        kept: r.kept,
        needs_confirmation: !r.needs_confirmation.is_empty(),
    })
}

impl WindowManager {
    /// Open the settings window (or focus if already open)
    pub fn open_settings_window(&mut self, event_loop: &ActiveEventLoop) {
        // If already open, bring to front and focus
        if let Some(settings_window) = &self.settings_window {
            settings_window.focus();
            return;
        }

        // Create new settings window using shared runtime
        let config = (**self.config.load()).clone();
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
                if let Some(nav) = self.settings_nav.clone() {
                    settings_window.settings_ui.restore_nav(nav);
                }
                // Wire up shell integration fn pointers
                settings_window
                    .settings_ui
                    .shell_integration_detected_shell_fn =
                    Some(crate::shell_integration_installer::detected_shell);
                settings_window
                    .settings_ui
                    .shell_integration_is_installed_fn =
                    Some(crate::shell_integration_installer::is_installed);
                settings_window.settings_ui.mux_hook_status_fn =
                    Some(crate::mux_hook_installer::hook_status);
                // Wire up shader fn pointers
                settings_window.settings_ui.shader_has_files_fn =
                    Some(crate::shader_installer::has_shader_files);
                settings_window.settings_ui.shader_count_files_fn =
                    Some(crate::shader_installer::count_shader_files);
                settings_window.settings_ui.shader_detect_modified_fn =
                    Some(crate::shader_installer::detect_modified_bundled_shaders);
                settings_window.settings_ui.shader_install_fn = Some(shader_install_wrapper);
                settings_window.settings_ui.shader_uninstall_fn = Some(shader_uninstall_wrapper);
                settings_window.settings_ui.shader_lint_fn =
                    Some(crate::shader_lint::shader_lint_settings_report);
                // Sync last update check result to settings UI
                settings_window.settings_ui.last_update_result = self
                    .last_update_result
                    .as_ref()
                    .map(to_settings_update_result);
                // Sync profiles from the focused window's profile manager
                let profiles = self
                    .focused_window()
                    .map(|ws| ws.overlay_ui.profile_manager.to_vec())
                    .unwrap_or_default();
                settings_window.settings_ui.sync_profiles(profiles);
                // Sync available agents from the focused window's discovered agents
                if let Some(ws) = self.focused_window() {
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

    /// Close the settings window.
    ///
    /// Never persists unsaved edits: the Save / Revert / Cancel prompt has
    /// already run for a user close. Only a changed collapsed-section set is
    /// written, on top of the last saved config (the baseline), so a forced
    /// close (last terminal window gone) cannot leak live-preview edits.
    pub fn close_settings_window(&mut self) {
        if let Some(settings_window) = self.settings_window.take() {
            self.settings_nav = Some(settings_window.settings_ui.nav_state());
            // Windows hold whatever the last live-preview frame sent them. If
            // that is not the last saved config, put the saved config back.
            let baseline = settings_window.settings_ui.baseline_config().clone();
            if !par_term_settings_ui::configs_equal(&self.config.load(), &baseline) {
                self.apply_config_to_windows(&baseline);
            }

            let persisted = self.config.load().collapsed_settings_sections.clone();
            if let Some(collapsed) = par_term_settings_ui::collapsed_sections_to_persist(
                &persisted,
                &settings_window.settings_ui.collapsed_sections,
            ) {
                self.config.rcu(|old| {
                    let mut new = (**old).clone();
                    new.collapsed_settings_sections = collapsed.clone();
                    std::sync::Arc::new(new)
                });
                for window_state in self.windows.values_mut() {
                    window_state.config.rcu(|old| {
                        let mut new = (**old).clone();
                        new.collapsed_settings_sections = collapsed.clone();
                        std::sync::Arc::new(new)
                    });
                }
                let mut to_persist = settings_window.settings_ui.baseline_config().clone();
                to_persist.collapsed_settings_sections = collapsed;
                if let Err(e) = to_persist.save() {
                    log::error!("Failed to persist collapsed settings sections: {}", e);
                }
            }
            log::info!("Closed settings window");
        }
    }

    /// Close the settings window if its last frame asked to close. Called
    /// after that frame's action has been applied, so a final Save or Revert
    /// from the close prompt is never dropped.
    pub fn close_settings_window_if_requested(&mut self) {
        if self
            .settings_window
            .as_ref()
            .is_some_and(|sw| sw.should_close())
        {
            self.close_settings_window();
        }
    }

    /// Check if a window ID belongs to the settings window
    pub fn is_settings_window(&self, window_id: WindowId) -> bool {
        self.settings_window
            .as_ref()
            .is_some_and(|sw| sw.window_id() == window_id)
    }

    /// Handle an event for the settings window.
    ///
    /// The caller applies the returned action and then calls
    /// [`Self::close_settings_window_if_requested`].
    pub fn handle_settings_window_event(
        &mut self,
        event: WindowEvent,
    ) -> Option<SettingsWindowAction> {
        self.settings_window
            .as_mut()
            .map(|settings_window| settings_window.handle_window_event(event))
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
                        if let Some(shader_name) =
                            window_state.config.load().shader.custom_shader.clone()
                        {
                            window_state
                                .shader_state
                                .shader_metadata_cache
                                .invalidate(&shader_name);
                            let metadata =
                                par_term_config::parse_shader_metadata(source).or_else(|| {
                                    window_state
                                        .shader_state
                                        .shader_metadata_cache
                                        .get_fresh(&shader_name)
                                });
                            let resolved = resolve_shader_config(
                                window_state.config.load().get_shader_override(&shader_name),
                                metadata.as_ref(),
                                &window_state.config.load(),
                            );
                            renderer.set_custom_shader_uniform_values(resolved.custom_uniforms);
                        }
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

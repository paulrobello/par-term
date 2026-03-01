//! Shader watcher lifecycle and hot-reload handling for WindowState.

use crate::app::window_state::WindowState;
use crate::config::Config;
use crate::shader_watcher::{ShaderReloadEvent, ShaderType, ShaderWatcher};

impl WindowState {
    /// Initialize the shader watcher for hot reload support
    pub(crate) fn init_shader_watcher(&mut self) {
        debug_info!(
            "SHADER",
            "init_shader_watcher: hot_reload={}",
            self.config.shader_hot_reload
        );

        if !self.config.shader_hot_reload {
            log::debug!("Shader hot reload disabled");
            return;
        }

        let background_path = self
            .config
            .custom_shader
            .as_ref()
            .filter(|_| self.config.custom_shader_enabled)
            .map(|s| Config::shader_path(s));

        let cursor_path = self
            .config
            .cursor_shader
            .as_ref()
            .filter(|_| self.config.cursor_shader_enabled)
            .map(|s| Config::shader_path(s));

        debug_info!(
            "SHADER",
            "Shader paths: background={:?}, cursor={:?}",
            background_path,
            cursor_path
        );

        if background_path.is_none() && cursor_path.is_none() {
            debug_info!("SHADER", "No shaders to watch for hot reload");
            return;
        }

        match ShaderWatcher::new(
            background_path.as_deref(),
            cursor_path.as_deref(),
            self.config.shader_hot_reload_delay,
        ) {
            Ok(watcher) => {
                debug_info!(
                    "SHADER",
                    "Shader hot reload initialized (debounce: {}ms)",
                    self.config.shader_hot_reload_delay
                );
                self.shader_state.shader_watcher = Some(watcher);
            }
            Err(e) => {
                debug_info!("SHADER", "Failed to initialize shader hot reload: {}", e);
            }
        }
    }

    /// Reinitialize shader watcher when shader paths change
    pub(crate) fn reinit_shader_watcher(&mut self) {
        debug_info!(
            "SHADER",
            "reinit_shader_watcher CALLED: shader={:?}, cursor={:?}",
            self.config.custom_shader,
            self.config.cursor_shader
        );
        // Drop existing watcher
        self.shader_state.shader_watcher = None;
        self.shader_state.shader_reload_error = None;

        // Reinitialize if hot reload is still enabled
        self.init_shader_watcher();
    }

    /// Check for and handle shader reload events
    ///
    /// Should be called periodically (e.g., in about_to_wait or render loop).
    /// Returns true if a shader was reloaded.
    pub(crate) fn check_shader_reload(&mut self) -> bool {
        let Some(watcher) = &self.shader_state.shader_watcher else {
            return false;
        };

        let Some(event) = watcher.try_recv() else {
            return false;
        };

        self.handle_shader_reload_event(event)
    }

    /// Handle a shader reload event
    ///
    /// On success: clears errors, triggers redraw, optionally shows notification
    /// On failure: preserves the old working shader, logs error, shows notification
    fn handle_shader_reload_event(&mut self, event: ShaderReloadEvent) -> bool {
        let shader_name = match event.shader_type {
            ShaderType::Background => "Background shader",
            ShaderType::Cursor => "Cursor shader",
        };
        let file_name = event
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("shader");

        log::info!("Hot reload: {} from {}", shader_name, event.path.display());

        // Read the shader source
        let source = match std::fs::read_to_string(&event.path) {
            Ok(s) => s,
            Err(e) => {
                let error_msg = format!("Cannot read '{}': {}", file_name, e);
                log::error!("Shader hot reload failed: {}", error_msg);
                self.shader_state.shader_reload_error = Some(error_msg.clone());
                // Track error for standalone settings window propagation
                match event.shader_type {
                    ShaderType::Background => {
                        self.shader_state.background_shader_reload_result =
                            Some(Some(error_msg.clone()));
                    }
                    ShaderType::Cursor => {
                        self.shader_state.cursor_shader_reload_result =
                            Some(Some(error_msg.clone()));
                    }
                }
                // Notify user of the error
                self.deliver_notification(
                    "Shader Reload Failed",
                    &format!("{} - {}", shader_name, error_msg),
                );
                // Trigger visual bell if enabled to alert user
                if self.config.notification_bell_visual
                    && let Some(tab) = self.tab_manager.active_tab_mut()
                {
                    tab.active_bell_mut().visual_flash = Some(std::time::Instant::now());
                }
                return false;
            }
        };

        let Some(renderer) = &mut self.renderer else {
            log::error!("Cannot reload shader: no renderer available");
            return false;
        };

        // Attempt to reload the shader
        // Note: On compilation failure, the old shader pipeline is preserved
        let result = match event.shader_type {
            ShaderType::Background => renderer.reload_shader_from_source(&source),
            ShaderType::Cursor => renderer.reload_cursor_shader_from_source(&source),
        };

        match result {
            Ok(()) => {
                log::info!("{} reloaded successfully from {}", shader_name, file_name);
                self.shader_state.shader_reload_error = None;
                // Track success for standalone settings window propagation
                match event.shader_type {
                    ShaderType::Background => {
                        self.shader_state.background_shader_reload_result = Some(None);
                    }
                    ShaderType::Cursor => {
                        self.shader_state.cursor_shader_reload_result = Some(None);
                    }
                }
                self.focus_state.needs_redraw = true;
                self.request_redraw();
                true
            }
            Err(e) => {
                // Extract the most relevant error message from the chain
                let root_cause = e.to_string();
                let error_msg = if root_cause.len() > 200 {
                    // Truncate very long error messages
                    format!("{}...", &root_cause[..200])
                } else {
                    root_cause
                };

                log::error!(
                    "{} compilation failed (old shader preserved): {}",
                    shader_name,
                    error_msg
                );
                log::debug!("Full error chain: {:#}", e);

                self.shader_state.shader_reload_error = Some(error_msg.clone());
                // Track error for standalone settings window propagation
                match event.shader_type {
                    ShaderType::Background => {
                        self.shader_state.background_shader_reload_result =
                            Some(Some(error_msg.clone()));
                    }
                    ShaderType::Cursor => {
                        self.shader_state.cursor_shader_reload_result =
                            Some(Some(error_msg.clone()));
                    }
                }

                // Notify user of the compilation error
                self.deliver_notification(
                    "Shader Compilation Error",
                    &format!("{}: {}", file_name, error_msg),
                );

                // Trigger visual bell if enabled to alert user
                if self.config.notification_bell_visual
                    && let Some(tab) = self.tab_manager.active_tab_mut()
                {
                    tab.active_bell_mut().visual_flash = Some(std::time::Instant::now());
                }

                false
            }
        }
    }
}

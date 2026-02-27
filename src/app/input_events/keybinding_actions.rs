//! Keybinding action, snippet, and custom action execution.
//!
//! - `execute_keybinding_action`: dispatches named actions (toggle shaders,
//!   new tab, copy, paste, etc.)
//! - `execute_snippet`: substitutes variables and sends snippet text to PTY
//! - `execute_custom_action`: runs shell commands / paste actions

use crate::app::window_state::WindowState;
use crate::config::resolve_shader_config;

impl WindowState {
    /// Execute a keybinding action by name.
    ///
    /// Returns true if the action was handled, false if unknown.
    pub(crate) fn execute_keybinding_action(&mut self, action: &str) -> bool {
        match action {
            "toggle_background_shader" => {
                self.toggle_background_shader();
                true
            }
            "toggle_cursor_shader" => {
                self.toggle_cursor_shader();
                true
            }
            "toggle_prettifier" => {
                if let Some(tab) = self.tab_manager.active_tab_mut()
                    && let Some(ref mut pipeline) = tab.prettifier
                {
                    pipeline.toggle_global();
                    log::info!(
                        "Prettifier toggled: {}",
                        if pipeline.is_enabled() {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    );
                }
                self.needs_redraw = true;
                true
            }
            "reload_config" => {
                self.reload_config();
                true
            }
            "open_settings" => {
                self.open_settings_window_requested = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                log::info!("Settings window requested via keybinding");
                true
            }
            "toggle_fullscreen" => {
                if let Some(window) = &self.window {
                    self.is_fullscreen = !self.is_fullscreen;
                    if self.is_fullscreen {
                        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                        log::info!("Entering fullscreen mode via keybinding");
                    } else {
                        window.set_fullscreen(None);
                        log::info!("Exiting fullscreen mode via keybinding");
                    }
                }
                true
            }
            "maximize_vertically" => {
                if let Some(window) = &self.window {
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
                        log::info!("Window maximized vertically via keybinding");
                    }
                }
                true
            }
            "toggle_help" => {
                self.overlay_ui.help_ui.toggle();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                log::info!(
                    "Help UI toggled via keybinding: {}",
                    if self.overlay_ui.help_ui.visible {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
                true
            }
            "toggle_fps_overlay" => {
                self.debug.show_fps_overlay = !self.debug.show_fps_overlay;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                log::info!(
                    "FPS overlay toggled via keybinding: {}",
                    if self.debug.show_fps_overlay {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
                true
            }
            "toggle_search" => {
                self.overlay_ui.search_ui.toggle();
                if self.overlay_ui.search_ui.visible {
                    self.overlay_ui.search_ui.init_from_config(
                        self.config.search_case_sensitive,
                        self.config.search_regex,
                    );
                }
                self.needs_redraw = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                log::info!(
                    "Search UI toggled via keybinding: {}",
                    if self.overlay_ui.search_ui.visible {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
                true
            }
            "toggle_ai_inspector" => {
                if self.config.ai_inspector_enabled {
                    let just_opened = self.overlay_ui.ai_inspector.toggle();
                    self.sync_ai_inspector_width();
                    if just_opened {
                        self.try_auto_connect_agent();
                    }
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
                true
            }
            "new_tab" => {
                self.new_tab_or_show_profiles();
                true
            }
            "close_tab" => {
                if self.has_multiple_tabs() {
                    self.close_current_tab();
                    log::info!("Tab closed via keybinding");
                }
                true
            }
            "next_tab" => {
                self.next_tab();
                log::debug!("Switched to next tab via keybinding");
                true
            }
            "prev_tab" => {
                self.prev_tab();
                log::debug!("Switched to previous tab via keybinding");
                true
            }
            "paste_special" => {
                // Get clipboard content and open paste special UI
                if let Some(text) = self.input_handler.paste_from_clipboard() {
                    self.overlay_ui.paste_special_ui.open(text);
                    self.needs_redraw = true;
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                    log::info!("Paste special UI opened");
                } else {
                    log::debug!("Paste special: no clipboard content");
                }
                true
            }
            "toggle_session_logging" => {
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    match tab.toggle_session_logging(&self.config) {
                        Ok(is_active) => {
                            let message = if is_active {
                                "⏺ Recording Started"
                            } else {
                                "⏹ Recording Stopped"
                            };
                            log::info!(
                                "Session logging toggled: {}",
                                if is_active { "started" } else { "stopped" }
                            );
                            // Show toast after releasing tab borrow
                            self.show_toast(message);
                        }
                        Err(e) => {
                            log::error!("Failed to toggle session logging: {}", e);
                            self.show_toast(format!("Recording Error: {}", e));
                        }
                    }
                }
                true
            }
            "split_horizontal" => {
                self.split_pane_horizontal();
                true
            }
            "split_vertical" => {
                self.split_pane_vertical();
                true
            }
            "close_pane" => {
                self.close_focused_pane();
                true
            }
            "navigate_pane_left" => {
                self.navigate_pane(crate::pane::NavigationDirection::Left);
                true
            }
            "navigate_pane_right" => {
                self.navigate_pane(crate::pane::NavigationDirection::Right);
                true
            }
            "navigate_pane_up" => {
                self.navigate_pane(crate::pane::NavigationDirection::Up);
                true
            }
            "navigate_pane_down" => {
                self.navigate_pane(crate::pane::NavigationDirection::Down);
                true
            }
            "resize_pane_left" => {
                self.resize_pane(crate::pane::NavigationDirection::Left);
                true
            }
            "resize_pane_right" => {
                self.resize_pane(crate::pane::NavigationDirection::Right);
                true
            }
            "resize_pane_up" => {
                self.resize_pane(crate::pane::NavigationDirection::Up);
                true
            }
            "resize_pane_down" => {
                self.resize_pane(crate::pane::NavigationDirection::Down);
                true
            }
            "toggle_tmux_session_picker" => {
                self.overlay_ui.tmux_session_picker_ui.toggle();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                log::info!(
                    "tmux session picker toggled via keybinding: {}",
                    if self.overlay_ui.tmux_session_picker_ui.visible {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
                true
            }
            "toggle_copy_mode" | "enter_copy_mode" => {
                if self.is_copy_mode_active() {
                    self.exit_copy_mode();
                } else {
                    self.enter_copy_mode();
                }
                true
            }
            "toggle_broadcast_input" => {
                self.broadcast_input = !self.broadcast_input;
                let message = if self.broadcast_input {
                    "Broadcast Input: ON"
                } else {
                    "Broadcast Input: OFF"
                };
                self.show_toast(message);
                log::info!(
                    "Broadcast input mode {}",
                    if self.broadcast_input {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
                true
            }
            "toggle_profile_drawer" => {
                self.toggle_profile_drawer();
                log::info!(
                    "Profile drawer toggled via keybinding: {}",
                    if self.overlay_ui.profile_drawer_ui.expanded {
                        "expanded"
                    } else {
                        "collapsed"
                    }
                );
                true
            }
            "toggle_clipboard_history" => {
                self.toggle_clipboard_history();
                log::info!(
                    "Clipboard history toggled via keybinding: {}",
                    if self.overlay_ui.clipboard_history_ui.visible {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
                true
            }
            "toggle_command_history" => {
                self.toggle_command_history();
                log::info!(
                    "Command history toggled via keybinding: {}",
                    if self.overlay_ui.command_history_ui.visible {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
                true
            }
            "clear_scrollback" => {
                let cleared = if let Some(tab) = self.tab_manager.active_tab_mut() {
                    // try_lock: intentional — keybinding action in sync event loop.
                    // On miss: scrollback not cleared this invocation. User can retry.
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
                    self.set_scroll_target(0);
                    log::info!("Cleared scrollback buffer via keybinding");
                }
                true
            }
            "increase_font_size" => {
                self.config.font_size = (self.config.font_size + 1.0).min(72.0);
                self.pending_font_rebuild = true;
                log::info!(
                    "Font size increased to {} via keybinding",
                    self.config.font_size
                );
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                true
            }
            "decrease_font_size" => {
                self.config.font_size = (self.config.font_size - 1.0).max(6.0);
                self.pending_font_rebuild = true;
                log::info!(
                    "Font size decreased to {} via keybinding",
                    self.config.font_size
                );
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                true
            }
            "reset_font_size" => {
                self.config.font_size = 14.0;
                self.pending_font_rebuild = true;
                log::info!("Font size reset to default (14.0) via keybinding");
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                true
            }
            "cycle_cursor_style" => {
                use crate::config::CursorStyle;
                use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;

                self.config.cursor_style = match self.config.cursor_style {
                    CursorStyle::Block => CursorStyle::Beam,
                    CursorStyle::Beam => CursorStyle::Underline,
                    CursorStyle::Underline => CursorStyle::Block,
                };

                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.cache.cells = None;
                }
                self.needs_redraw = true;

                log::info!(
                    "Cycled cursor style to {:?} via keybinding",
                    self.config.cursor_style
                );

                let term_style = if self.config.cursor_blink {
                    match self.config.cursor_style {
                        CursorStyle::Block => TermCursorStyle::BlinkingBlock,
                        CursorStyle::Beam => TermCursorStyle::BlinkingBar,
                        CursorStyle::Underline => TermCursorStyle::BlinkingUnderline,
                    }
                } else {
                    match self.config.cursor_style {
                        CursorStyle::Block => TermCursorStyle::SteadyBlock,
                        CursorStyle::Beam => TermCursorStyle::SteadyBar,
                        CursorStyle::Underline => TermCursorStyle::SteadyUnderline,
                    }
                };

                // try_lock: intentional — cursor blink toggle via keybinding in sync loop.
                // On miss: cursor style not updated this invocation. Cosmetic only.
                if let Some(tab) = self.tab_manager.active_tab()
                    && let Ok(mut term) = tab.terminal.try_lock()
                {
                    term.set_cursor_style(term_style);
                }
                true
            }
            "move_tab_left" => {
                self.move_tab_left();
                log::debug!("Moved tab left via keybinding");
                true
            }
            "move_tab_right" => {
                self.move_tab_right();
                log::debug!("Moved tab right via keybinding");
                true
            }
            "switch_to_tab_1" => {
                self.switch_to_tab_index(1);
                true
            }
            "switch_to_tab_2" => {
                self.switch_to_tab_index(2);
                true
            }
            "switch_to_tab_3" => {
                self.switch_to_tab_index(3);
                true
            }
            "switch_to_tab_4" => {
                self.switch_to_tab_index(4);
                true
            }
            "switch_to_tab_5" => {
                self.switch_to_tab_index(5);
                true
            }
            "switch_to_tab_6" => {
                self.switch_to_tab_index(6);
                true
            }
            "switch_to_tab_7" => {
                self.switch_to_tab_index(7);
                true
            }
            "switch_to_tab_8" => {
                self.switch_to_tab_index(8);
                true
            }
            "switch_to_tab_9" => {
                self.switch_to_tab_index(9);
                true
            }
            "toggle_throughput_mode" => {
                self.config.maximize_throughput = !self.config.maximize_throughput;
                let message = if self.config.maximize_throughput {
                    "Throughput Mode: ON"
                } else {
                    "Throughput Mode: OFF"
                };
                self.show_toast(message);
                log::info!(
                    "Throughput mode {}",
                    if self.config.maximize_throughput {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
                true
            }
            "reopen_closed_tab" => {
                self.reopen_closed_tab();
                true
            }
            "save_arrangement" => {
                // Open settings to Arrangements tab
                self.open_settings_window_requested = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                log::info!("Save arrangement requested via keybinding");
                true
            }
            "ssh_quick_connect" => {
                self.overlay_ui.ssh_connect_ui.open(
                    self.config.enable_mdns_discovery,
                    self.config.mdns_scan_timeout_secs,
                );
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                log::info!("SSH Quick Connect opened via keybinding");
                true
            }
            "reload_dynamic_profiles" => {
                self.reload_dynamic_profiles_requested = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                log::info!("Dynamic profiles reload requested via keybinding");
                true
            }
            _ => {
                // Check for snippet or action keybindings
                if let Some(snippet_id) = action.strip_prefix("snippet:") {
                    self.execute_snippet(snippet_id)
                } else if let Some(action_id) = action.strip_prefix("action:") {
                    self.execute_custom_action(action_id)
                } else if let Some(arrangement_name) = action.strip_prefix("restore_arrangement:") {
                    // Restore arrangement by name - handled by WindowManager
                    self.pending_arrangement_restore = Some(arrangement_name.to_string());
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                    log::info!(
                        "Arrangement restore requested via keybinding: {}",
                        arrangement_name
                    );
                    true
                } else {
                    log::warn!("Unknown keybinding action: {}", action);
                    false
                }
            }
        }
    }

    /// Execute a snippet by ID.
    ///
    /// Returns true if the snippet was found and executed, false otherwise.
    fn execute_snippet(&mut self, snippet_id: &str) -> bool {
        // Find the snippet by ID
        let snippet = match self.config.snippets.iter().find(|s| s.id == snippet_id) {
            Some(s) => s,
            None => {
                log::warn!("Snippet not found: {}", snippet_id);
                return false;
            }
        };

        // Check if snippet is enabled
        if !snippet.enabled {
            log::debug!("Snippet '{}' is disabled", snippet.title);
            return false;
        }

        // Substitute variables in the snippet content, including session variables
        let substituted_content = {
            let session_vars = self.badge_state.variables.read();
            let result = crate::snippets::VariableSubstitutor::new().substitute_with_session(
                &snippet.content,
                &snippet.variables,
                Some(&session_vars),
            );
            drop(session_vars); // Explicitly drop before using self again
            match result {
                Ok(content) => content,
                Err(e) => {
                    log::error!(
                        "Failed to substitute variables in snippet '{}': {}",
                        snippet.title,
                        e
                    );
                    self.show_toast(format!("Snippet Error: {}", e));
                    return false;
                }
            }
        };

        // Write to the active terminal
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            // try_lock: intentional — execute_snippet called from keybinding handler in
            // sync event loop. On miss: the snippet is not sent to the terminal this
            // invocation. The user can trigger the keybinding again.
            if let Ok(terminal) = tab.terminal.try_lock() {
                // Append newline if auto_execute is enabled
                let content_to_write = if snippet.auto_execute {
                    format!("{}\n", substituted_content)
                } else {
                    substituted_content.clone()
                };

                if let Err(e) = terminal.write(content_to_write.as_bytes()) {
                    log::error!("Failed to write snippet to terminal: {}", e);
                    return false;
                }

                log::info!(
                    "Executed snippet '{}' (auto_execute={})",
                    snippet.title,
                    snippet.auto_execute
                );
                return true;
            } else {
                log::error!("Failed to lock terminal for snippet execution");
                return false;
            }
        }

        false
    }

    /// Execute a custom action by ID.
    ///
    /// Returns true if the action was found and executed, false otherwise.
    fn execute_custom_action(&mut self, action_id: &str) -> bool {
        use crate::config::snippets::CustomActionConfig;

        // Find the action by ID
        let action = match self.config.actions.iter().find(|a| a.id() == action_id) {
            Some(a) => a,
            None => {
                log::warn!("Custom action not found: {}", action_id);
                return false;
            }
        };

        match action {
            CustomActionConfig::ShellCommand {
                command,
                args,
                notify_on_success,
                ..
            } => {
                log::info!("Executing shell command: {} {}", command, args.join(" "));

                // Execute the shell command
                let result = std::process::Command::new(command).args(args).output();

                match result {
                    Ok(output) => {
                        if output.status.success() {
                            if *notify_on_success {
                                let message =
                                    String::from_utf8_lossy(&output.stdout).trim().to_string();
                                self.show_toast(format!("Command completed: {}", message));
                            }
                            log::info!("Shell command completed successfully");
                            true
                        } else {
                            let error_msg =
                                String::from_utf8_lossy(&output.stderr).trim().to_string();
                            log::error!("Shell command failed: {}", error_msg);
                            self.show_toast(format!("Command failed: {}", error_msg));
                            false
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to execute shell command: {}", e);
                        self.show_toast(format!("Execution Error: {}", e));
                        false
                    }
                }
            }
            CustomActionConfig::InsertText {
                text, variables, ..
            } => {
                // Substitute variables
                let substituted_text =
                    match crate::snippets::VariableSubstitutor::new().substitute(text, variables) {
                        Ok(content) => content,
                        Err(e) => {
                            log::error!("Failed to substitute variables in action: {}", e);
                            self.show_toast(format!("Action Error: {}", e));
                            return false;
                        }
                    };

                // Write to the active terminal
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    // try_lock: intentional — execute_custom_action runs from keybinding
                    // handler in sync event loop. On miss: the action text is not written.
                    // Logged as an error so the user is aware; they can retry the keybinding.
                    if let Ok(terminal) = tab.terminal.try_lock() {
                        if let Err(e) = terminal.write(substituted_text.as_bytes()) {
                            log::error!("Failed to write action text to terminal: {}", e);
                            return false;
                        }

                        log::info!("Executed insert text action");
                        return true;
                    } else {
                        log::error!("Failed to lock terminal for action execution");
                        return false;
                    }
                }

                false
            }
            CustomActionConfig::KeySequence { keys, title, .. } => {
                use crate::keybindings::parse_key_sequence;

                let byte_sequences = match parse_key_sequence(keys) {
                    Ok(seqs) => seqs,
                    Err(e) => {
                        log::error!("Invalid key sequence '{}': {}", keys, e);
                        self.show_toast(format!("Invalid key sequence: {}", e));
                        return false;
                    }
                };

                // Write all key sequences to the terminal
                let write_error = if let Some(tab) = self.tab_manager.active_tab_mut() {
                    // try_lock: intentional — send_keys action in sync event loop.
                    // On miss: the key sequences are not written. User can retry.
                    if let Ok(terminal) = tab.terminal.try_lock() {
                        let mut err: Option<String> = None;
                        for bytes in &byte_sequences {
                            if let Err(e) = terminal.write(bytes) {
                                err = Some(format!("{}", e));
                                break;
                            }
                        }
                        err
                    } else {
                        log::error!("Failed to lock terminal for key sequence execution");
                        return false;
                    }
                } else {
                    return false;
                };

                if let Some(e) = write_error {
                    log::error!("Failed to write key sequence: {}", e);
                    self.show_toast(format!("Key sequence error: {}", e));
                    return false;
                }

                log::info!(
                    "Executed key sequence action '{}' ({} keys)",
                    title,
                    byte_sequences.len()
                );
                true
            }
        }
    }

    /// Show a toast notification with the given message.
    ///
    /// The toast will be displayed for 2 seconds and then automatically hidden.
    pub(crate) fn show_toast(&mut self, message: impl Into<String>) {
        self.toast_message = Some(message.into());
        self.toast_hide_time = Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
        self.needs_redraw = true;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// Show pane index overlays for a specified duration.
    pub(crate) fn show_pane_indices(&mut self, duration: std::time::Duration) {
        self.pane_identify_hide_time = Some(std::time::Instant::now() + duration);
        self.needs_redraw = true;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// Toggle the background/custom shader on/off.
    pub(crate) fn toggle_background_shader(&mut self) {
        self.config.custom_shader_enabled = !self.config.custom_shader_enabled;

        if let Some(renderer) = &mut self.renderer {
            // Get shader metadata from cache for resolution
            let metadata = self
                .config
                .custom_shader
                .as_ref()
                .and_then(|name| self.shader_state.shader_metadata_cache.get(name).cloned());

            // Get per-shader overrides
            let shader_override = self
                .config
                .custom_shader
                .as_ref()
                .and_then(|name| self.config.shader_configs.get(name).cloned());

            // Resolve config with 3-tier system
            let resolved =
                resolve_shader_config(shader_override.as_ref(), metadata.as_ref(), &self.config);

            let _ = renderer.set_custom_shader_enabled(
                self.config.custom_shader_enabled,
                self.config.custom_shader.as_deref(),
                self.config.window_opacity,
                self.config.custom_shader_animation,
                resolved.animation_speed,
                resolved.full_content,
                resolved.brightness,
                &resolved.channel_paths(),
                resolved.cubemap_path().map(|p| p.as_path()),
            );
        }

        self.needs_redraw = true;
        if let Some(window) = &self.window {
            window.request_redraw();
        }

        log::info!(
            "Background shader {}",
            if self.config.custom_shader_enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
    }

    /// Toggle the cursor shader on/off.
    pub(crate) fn toggle_cursor_shader(&mut self) {
        self.config.cursor_shader_enabled = !self.config.cursor_shader_enabled;

        if let Some(renderer) = &mut self.renderer {
            let _ = renderer.set_cursor_shader_enabled(
                self.config.cursor_shader_enabled,
                self.config.cursor_shader.as_deref(),
                self.config.window_opacity,
                self.config.cursor_shader_animation,
                self.config.cursor_shader_animation_speed,
            );
        }

        self.needs_redraw = true;
        if let Some(window) = &self.window {
            window.request_redraw();
        }

        log::info!(
            "Cursor shader {}",
            if self.config.cursor_shader_enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
    }
}

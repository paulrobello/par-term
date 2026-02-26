//! Window event handling and per-frame update methods for WindowState.
//!
//! Contains:
//! - Shell integration title/badge sync
//! - `handle_window_event`: routes winit WindowEvents to terminal/renderer
//! - `handle_focus_change`: power-saving focus logic
//! - `about_to_wait`: per-frame polling (notifications, tmux, config reload, etc.)

use crate::app::window_state::WindowState;
use std::sync::Arc;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};

impl WindowState {
    /// Update window title with shell integration info (cwd and exit code)
    /// Only updates if not scrolled and not hovering over URL
    pub(crate) fn update_window_title_with_shell_integration(&self) {
        // Get active tab state
        let tab = if let Some(t) = self.tab_manager.active_tab() {
            t
        } else {
            return;
        };

        // Skip if scrolled (scrollback indicator takes priority)
        if tab.scroll_state.offset != 0 {
            return;
        }

        // Skip if hovering over URL (URL tooltip takes priority)
        if tab.mouse.hovered_url.is_some() {
            return;
        }

        // Skip if window not available
        let window = if let Some(w) = &self.window {
            w
        } else {
            return;
        };

        // Try to get shell integration info
        // try_lock: intentional — called every frame from the render path; blocking would
        // stall rendering. On miss: window title is not updated this frame. No data loss.
        if let Ok(term) = tab.terminal.try_lock() {
            let mut title_parts = vec![self.config.window_title.clone()];

            // Add window number if configured
            if self.config.show_window_number {
                title_parts.push(format!("[{}]", self.window_index));
            }

            // Add current working directory if available
            if let Some(cwd) = term.shell_integration_cwd() {
                // Abbreviate home directory to ~
                let abbreviated_cwd = if let Some(home) = dirs::home_dir() {
                    cwd.replace(&home.to_string_lossy().to_string(), "~")
                } else {
                    cwd
                };
                title_parts.push(format!("({})", abbreviated_cwd));
            }

            // Add running command indicator if a command is executing
            if let Some(cmd_name) = term.get_running_command_name() {
                title_parts.push(format!("[{}]", cmd_name));
            }

            // Add exit code indicator if last command failed
            if let Some(exit_code) = term.shell_integration_exit_code()
                && exit_code != 0
            {
                title_parts.push(format!("[Exit: {}]", exit_code));
            }

            // Add recording indicator
            if self.is_recording {
                title_parts.push("[RECORDING]".to_string());
            }

            // Build and set title
            let title = title_parts.join(" ");
            window.set_title(&title);
        }
    }

    /// Sync shell integration data (exit code, command, cwd, hostname, username) to badge variables
    pub(crate) fn sync_badge_shell_integration(&mut self) {
        let tab = if let Some(t) = self.tab_manager.active_tab() {
            t
        } else {
            return;
        };

        // try_lock: intentional — sync_badge_shell_integration is called from the render
        // path. On miss: badge variables are not updated this frame; they will be on the next.
        if let Ok(term) = tab.terminal.try_lock() {
            let exit_code = term.shell_integration_exit_code();
            let current_command = term.get_running_command_name();
            let cwd = term.shell_integration_cwd();
            let hostname = term.shell_integration_hostname();
            let username = term.shell_integration_username();

            let mut vars = self.badge_state.variables_mut();
            let mut badge_changed = false;

            if vars.exit_code != exit_code {
                vars.set_exit_code(exit_code);
                badge_changed = true;
            }
            if vars.current_command != current_command {
                vars.set_current_command(current_command);
                badge_changed = true;
            }
            if let Some(cwd) = cwd
                && vars.path != cwd
            {
                vars.set_path(cwd);
                badge_changed = true;
            }
            if let Some(ref host) = hostname
                && vars.hostname != *host
            {
                vars.hostname = host.clone();
                badge_changed = true;
            } else if hostname.is_none() && !vars.hostname.is_empty() {
                // Returned to localhost — keep the initial hostname from new()
            }
            if let Some(ref user) = username
                && vars.username != *user
            {
                vars.username = user.clone();
                badge_changed = true;
            }
            drop(vars);
            if badge_changed {
                self.badge_state.mark_dirty();
            }
        }
    }

    /// Handle window events for this window state
    pub(crate) fn handle_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: WindowEvent,
    ) -> bool {
        use winit::keyboard::{Key, NamedKey};

        // Debug: Log ALL keyboard events at entry to diagnose Space issue
        if let WindowEvent::KeyboardInput {
            event: key_event, ..
        } = &event
        {
            match &key_event.logical_key {
                Key::Character(s) => {
                    log::trace!(
                        "window_event: Character '{}', state={:?}",
                        s,
                        key_event.state
                    );
                }
                Key::Named(named) => {
                    log::trace!(
                        "window_event: Named key {:?}, state={:?}",
                        named,
                        key_event.state
                    );
                }
                other => {
                    log::trace!(
                        "window_event: Other key {:?}, state={:?}",
                        other,
                        key_event.state
                    );
                }
            }
        }

        // Let egui handle the event (needed for proper rendering state)
        let (egui_consumed, egui_needs_repaint) =
            if let (Some(egui_state), Some(window)) = (&mut self.egui_state, &self.window) {
                let event_response = egui_state.on_window_event(window, &event);
                // Request redraw if egui needs it (e.g., text input in modals)
                if event_response.repaint {
                    window.request_redraw();
                }
                (event_response.consumed, event_response.repaint)
            } else {
                (false, false)
            };
        let _ = egui_needs_repaint; // Used above, silence unused warning

        // Debug: Log when egui consumes events but we ignore it
        let any_ui_visible = self.any_modal_ui_visible();
        if egui_consumed
            && !any_ui_visible
            && let WindowEvent::KeyboardInput {
                event: key_event, ..
            } = &event
            && let Key::Named(NamedKey::Space) = &key_event.logical_key
        {
            log::debug!("egui tried to consume Space (UI closed, ignoring)");
        }

        // When shader editor is visible, block keyboard events from terminal
        // even if egui didn't consume them (egui might not have focus)
        if any_ui_visible
            && let WindowEvent::KeyboardInput {
                event: key_event, ..
            } = &event
            // Always block keyboard input when UI is visible (except system keys)
            && !matches!(
                key_event.logical_key,
                Key::Named(NamedKey::F1)
                    | Key::Named(NamedKey::F2)
                    | Key::Named(NamedKey::F3)
                    | Key::Named(NamedKey::F11)
                    | Key::Named(NamedKey::Escape)
            )
        {
            return false;
        }

        if egui_consumed
            && any_ui_visible
            && !matches!(
                event,
                WindowEvent::CloseRequested | WindowEvent::RedrawRequested
            )
        {
            return false; // Event consumed by egui, don't close window
        }

        match event {
            WindowEvent::CloseRequested => {
                log::info!("Close requested for window");

                // Check if prompt_on_quit is enabled and there are active sessions
                let tab_count = self.tab_manager.tab_count();
                if self.config.prompt_on_quit
                    && tab_count > 0
                    && !self.quit_confirmation_ui.is_visible()
                {
                    log::info!(
                        "Showing quit confirmation dialog ({} active sessions)",
                        tab_count
                    );
                    self.quit_confirmation_ui.show_confirmation(tab_count);
                    self.needs_redraw = true;
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                    return false; // Don't close yet - wait for user confirmation
                }

                self.perform_shutdown();
                return true; // Signal to close this window
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
                    log::info!(
                        "Scale factor changed to {} (display change detected)",
                        scale_factor
                    );

                    let size = window.inner_size();
                    let (cols, rows) = renderer.handle_scale_factor_change(scale_factor, size);

                    // Reconfigure surface after scale factor change
                    // This is important when dragging between displays with different DPIs
                    renderer.reconfigure_surface();

                    // Calculate pixel dimensions
                    let cell_width = renderer.cell_width();
                    let cell_height = renderer.cell_height();
                    let width_px = (cols as f32 * cell_width) as usize;
                    let height_px = (rows as f32 * cell_height) as usize;

                    // Resize all tabs' terminals with pixel dimensions for TIOCGWINSZ support
                    for tab in self.tab_manager.tabs_mut() {
                        // try_lock: intentional — resize happens during ScaleFactorChanged
                        // which fires in the sync event loop. On miss: this tab's terminal
                        // keeps its old size until the next resize event. Low risk as scale
                        // factor changes are rare (drag between displays).
                        if let Ok(mut term) = tab.terminal.try_lock() {
                            let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
                        }
                    }

                    // Reconfigure macOS Metal layer after display change
                    #[cfg(target_os = "macos")]
                    {
                        if let Err(e) =
                            crate::macos_metal::configure_metal_layer_for_performance(window)
                        {
                            log::warn!(
                                "Failed to reconfigure Metal layer after display change: {}",
                                e
                            );
                        }
                    }

                    // Request redraw to apply changes
                    window.request_redraw();
                }
            }

            // Handle window moved - surface may become invalid when moving between monitors
            WindowEvent::Moved(_) => {
                if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
                    log::debug!(
                        "Window moved - reconfiguring surface for potential display change"
                    );

                    // Reconfigure surface to handle potential display changes
                    // This catches cases where displays have same DPI but different surface properties
                    renderer.reconfigure_surface();

                    // On macOS, reconfigure the Metal layer for the potentially new display
                    #[cfg(target_os = "macos")]
                    {
                        if let Err(e) =
                            crate::macos_metal::configure_metal_layer_for_performance(window)
                        {
                            log::warn!(
                                "Failed to reconfigure Metal layer after window move: {}",
                                e
                            );
                        }
                    }

                    // Request redraw to ensure proper rendering on new display
                    window.request_redraw();
                }
            }

            WindowEvent::Resized(physical_size) => {
                if let Some(renderer) = &mut self.renderer {
                    let (cols, rows) = renderer.resize(physical_size);

                    // Calculate text area pixel dimensions
                    let cell_width = renderer.cell_width();
                    let cell_height = renderer.cell_height();
                    let width_px = (cols as f32 * cell_width) as usize;
                    let height_px = (rows as f32 * cell_height) as usize;

                    // Resize all tabs' terminals with pixel dimensions for TIOCGWINSZ support
                    // This allows applications like kitty icat to query pixel dimensions
                    // Note: The core library (v0.11.0+) implements scrollback reflow when
                    // width changes - wrapped lines are unwrapped/re-wrapped as needed.
                    for tab in self.tab_manager.tabs_mut() {
                        // try_lock: intentional — Resized fires in the sync event loop.
                        // On miss: this tab's terminal keeps its old dimensions; the cell
                        // cache is still invalidated below so rendering uses the correct
                        // grid size. The terminal size will be fixed on the next resize event.
                        if let Ok(mut term) = tab.terminal.try_lock() {
                            let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
                            tab.cache.scrollback_len = term.scrollback_len();
                        }
                        // Invalidate cell cache to force regeneration
                        tab.cache.cells = None;
                    }

                    // Update scrollbar for active tab
                    if let Some(tab) = self.tab_manager.active_tab() {
                        let total_lines = rows + tab.cache.scrollback_len;
                        // try_lock: intentional — scrollbar mark update during Resized event.
                        // On miss: scrollbar renders without marks this frame. Cosmetic only.
                        let marks = if let Ok(term) = tab.terminal.try_lock() {
                            term.scrollback_marks()
                        } else {
                            Vec::new()
                        };
                        renderer.update_scrollbar(
                            tab.scroll_state.offset,
                            rows,
                            total_lines,
                            &marks,
                        );
                    }

                    // Update resize overlay state
                    self.resize_dimensions =
                        Some((physical_size.width, physical_size.height, cols, rows));
                    self.resize_overlay_visible = true;
                    // Hide overlay 1 second after resize stops
                    self.resize_overlay_hide_time =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(1));

                    // Notify tmux of the new size if gateway mode is active
                    self.notify_tmux_of_resize();
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_key_event(event, event_loop);
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                self.input_handler.update_modifiers(modifiers);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                // Skip terminal handling if egui UI is visible or using the pointer
                // Note: any_ui_visible check is needed because is_egui_using_pointer()
                // returns false before egui is initialized (e.g., at startup when
                // shader_install_ui is shown before first render)
                if !any_ui_visible && !self.is_egui_using_pointer() {
                    self.handle_mouse_wheel(delta);
                }
            }

            WindowEvent::MouseInput { button, state, .. } => {
                use winit::event::ElementState;

                // Eat the first mouse press that brings the window into focus.
                // Without this, the click is forwarded to the PTY where mouse-aware
                // apps (tmux with `mouse on`) trigger a zero-char selection that
                // clears the system clipboard — destroying any clipboard image.
                //
                // Some platforms deliver `Focused(true)` before the mouse press, others
                // can deliver it after the press/release. Treat a press that arrives while
                // we're still unfocused as a focus-click too, then avoid double-arming the
                // later focus event path.
                let is_focus_click_press = state == ElementState::Pressed
                    && (self.focus_click_pending || !self.is_focused);
                if is_focus_click_press {
                    self.focus_click_pending = false;
                    if !self.is_focused {
                        self.focus_click_suppressed_while_unfocused_at =
                            Some(std::time::Instant::now());
                    }
                    self.ui_consumed_mouse_press = true; // Also suppress the release
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                } else {
                    // Track UI mouse consumption to prevent release events bleeding through
                    // when UI closes during a click (e.g., drawer toggle)
                    let ui_wants_pointer = any_ui_visible || self.is_egui_using_pointer();

                    if state == ElementState::Pressed {
                        if ui_wants_pointer {
                            self.ui_consumed_mouse_press = true;
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        } else {
                            self.ui_consumed_mouse_press = false;
                            self.begin_clipboard_image_click_guard(button, state);
                            self.handle_mouse_button(button, state);
                            self.finish_clipboard_image_click_guard(button, state);
                        }
                    } else {
                        // Release: block if we consumed the press OR if UI wants pointer
                        if self.ui_consumed_mouse_press || ui_wants_pointer {
                            self.ui_consumed_mouse_press = false;
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        } else {
                            self.begin_clipboard_image_click_guard(button, state);
                            self.handle_mouse_button(button, state);
                            self.finish_clipboard_image_click_guard(button, state);
                        }
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                // Skip terminal handling if egui UI is visible or using the pointer
                if any_ui_visible || self.is_egui_using_pointer() {
                    // Request redraw so egui can update hover states
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                } else {
                    self.handle_mouse_move((position.x, position.y));
                }
            }

            WindowEvent::Focused(focused) => {
                self.handle_focus_change(focused);
            }

            WindowEvent::RedrawRequested => {
                // Skip rendering if shutting down
                if self.is_shutting_down {
                    return false;
                }

                // Handle shell exit based on configured action
                use crate::config::ShellExitAction;
                use crate::pane::RestartState;

                match self.config.shell_exit_action {
                    ShellExitAction::Keep => {
                        // Do nothing - keep dead shells showing
                    }

                    ShellExitAction::Close => {
                        // Original behavior: close exited panes and their tabs
                        let mut tabs_needing_resize: Vec<crate::tab::TabId> = Vec::new();

                        let tabs_to_close: Vec<crate::tab::TabId> = self
                            .tab_manager
                            .tabs_mut()
                            .iter_mut()
                            .filter_map(|tab| {
                                if tab.tmux_gateway_active || tab.tmux_pane_id.is_some() {
                                    return None;
                                }
                                if tab.pane_manager.is_some() {
                                    let (closed_panes, tab_should_close) = tab.close_exited_panes();
                                    if !closed_panes.is_empty() {
                                        log::info!(
                                            "Tab {}: closed {} exited pane(s)",
                                            tab.id,
                                            closed_panes.len()
                                        );
                                        if !tab_should_close {
                                            tabs_needing_resize.push(tab.id);
                                        }
                                    }
                                    if tab_should_close {
                                        return Some(tab.id);
                                    }
                                }
                                None
                            })
                            .collect();

                        if !tabs_needing_resize.is_empty()
                            && let Some(renderer) = &self.renderer
                        {
                            let cell_width = renderer.cell_width();
                            let cell_height = renderer.cell_height();
                            let padding = self.config.pane_padding;
                            let title_offset = if self.config.show_pane_titles {
                                self.config.pane_title_height
                            } else {
                                0.0
                            };
                            for tab_id in tabs_needing_resize {
                                if let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                                    && let Some(pm) = tab.pane_manager_mut()
                                {
                                    pm.resize_all_terminals_with_padding(
                                        cell_width,
                                        cell_height,
                                        padding,
                                        title_offset,
                                    );
                                }
                            }
                        }

                        for tab_id in &tabs_to_close {
                            log::info!("Closing tab {} - all panes exited", tab_id);
                            if self.tab_manager.tab_count() <= 1 {
                                log::info!("Last tab, closing window");
                                self.is_shutting_down = true;
                                for tab in self.tab_manager.tabs_mut() {
                                    tab.stop_refresh_task();
                                }
                                return true;
                            } else {
                                let _ = self.tab_manager.close_tab(*tab_id);
                            }
                        }

                        // Also check legacy single-pane tabs
                        let (shell_exited, active_tab_id, tab_count, tab_title, exit_notified) = {
                            if let Some(tab) = self.tab_manager.active_tab() {
                                let exited = tab.pane_manager.is_none()
                                    // try_lock: intentional — shell exit check during RedrawRequested
                                    // in the sync event loop. On miss: treat as still running
                                    // (is_some_and returns false on None), so the tab stays open
                                    // until the next frame resolves the exit.
                                    && tab
                                        .terminal
                                        .try_lock()
                                        .ok()
                                        .is_some_and(|term| !term.is_running());
                                (
                                    exited,
                                    Some(tab.id),
                                    self.tab_manager.tab_count(),
                                    tab.title.clone(),
                                    tab.exit_notified,
                                )
                            } else {
                                (false, None, 0, String::new(), false)
                            }
                        };

                        if shell_exited {
                            log::info!("Shell in active tab has exited");
                            if self.config.notification_session_ended && !exit_notified {
                                if let Some(tab) = self.tab_manager.active_tab_mut() {
                                    tab.exit_notified = true;
                                }
                                let title = format!("Session Ended: {}", tab_title);
                                let message = "The shell process has exited".to_string();
                                self.deliver_notification(&title, &message);
                            }

                            if tab_count <= 1 {
                                log::info!("Last tab, closing window");
                                self.is_shutting_down = true;
                                for tab in self.tab_manager.tabs_mut() {
                                    tab.stop_refresh_task();
                                }
                                return true;
                            } else if let Some(tab_id) = active_tab_id {
                                let _ = self.tab_manager.close_tab(tab_id);
                            }
                        }
                    }

                    ShellExitAction::RestartImmediately
                    | ShellExitAction::RestartWithPrompt
                    | ShellExitAction::RestartAfterDelay => {
                        // Handle restart variants
                        let config_clone = self.config.clone();

                        for tab in self.tab_manager.tabs_mut() {
                            if tab.tmux_gateway_active || tab.tmux_pane_id.is_some() {
                                continue;
                            }

                            if let Some(pm) = tab.pane_manager_mut() {
                                for pane in pm.all_panes_mut() {
                                    let is_running = pane.is_running();

                                    // Check if pane needs restart action
                                    if !is_running && pane.restart_state.is_none() {
                                        // Shell just exited, handle based on action
                                        match self.config.shell_exit_action {
                                            ShellExitAction::RestartImmediately => {
                                                log::info!(
                                                    "Pane {} shell exited, restarting immediately",
                                                    pane.id
                                                );
                                                if let Err(e) = pane.respawn_shell(&config_clone) {
                                                    log::error!(
                                                        "Failed to respawn shell in pane {}: {}",
                                                        pane.id,
                                                        e
                                                    );
                                                }
                                            }
                                            ShellExitAction::RestartWithPrompt => {
                                                log::info!(
                                                    "Pane {} shell exited, showing restart prompt",
                                                    pane.id
                                                );
                                                pane.write_restart_prompt();
                                                pane.restart_state =
                                                    Some(RestartState::AwaitingInput);
                                            }
                                            ShellExitAction::RestartAfterDelay => {
                                                log::info!(
                                                    "Pane {} shell exited, will restart after 1s",
                                                    pane.id
                                                );
                                                pane.restart_state =
                                                    Some(RestartState::AwaitingDelay(
                                                        std::time::Instant::now(),
                                                    ));
                                            }
                                            _ => {}
                                        }
                                    }

                                    // Check if waiting for delay and time has elapsed
                                    if let Some(RestartState::AwaitingDelay(exit_time)) =
                                        &pane.restart_state
                                        && exit_time.elapsed() >= std::time::Duration::from_secs(1)
                                    {
                                        log::info!(
                                            "Pane {} delay elapsed, restarting shell",
                                            pane.id
                                        );
                                        if let Err(e) = pane.respawn_shell(&config_clone) {
                                            log::error!(
                                                "Failed to respawn shell in pane {}: {}",
                                                pane.id,
                                                e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                self.render();
            }

            WindowEvent::DroppedFile(path) => {
                self.handle_dropped_file(path);
            }

            WindowEvent::CursorEntered { .. } => {
                // Focus follows mouse: auto-focus window when cursor enters
                if self.config.focus_follows_mouse
                    && let Some(window) = &self.window
                {
                    window.focus_window();
                }
            }

            WindowEvent::ThemeChanged(system_theme) => {
                let is_dark = system_theme == winit::window::Theme::Dark;
                let theme_changed = self.config.apply_system_theme(is_dark);
                let tab_style_changed = self.config.apply_system_tab_style(is_dark);

                if theme_changed {
                    log::info!(
                        "System theme changed to {}, switching to theme: {}",
                        if is_dark { "dark" } else { "light" },
                        self.config.theme
                    );
                    let theme = self.config.load_theme();
                    for tab in self.tab_manager.tabs_mut() {
                        // try_lock: intentional — ThemeChanged fires in the sync event loop.
                        // On miss: this tab keeps the old theme until the next theme event
                        // or config reload. Cell cache is still invalidated to prevent stale
                        // rendering with the old theme colors.
                        if let Ok(mut term) = tab.terminal.try_lock() {
                            term.set_theme(theme.clone());
                        }
                        tab.cache.cells = None;
                    }
                }

                if tab_style_changed {
                    log::info!(
                        "Auto tab style: switching to {} tab style",
                        if is_dark {
                            self.config.dark_tab_style.display_name()
                        } else {
                            self.config.light_tab_style.display_name()
                        }
                    );
                }

                if theme_changed || tab_style_changed {
                    if let Err(e) = self.config.save() {
                        log::error!("Failed to save config after theme change: {}", e);
                    }
                    self.needs_redraw = true;
                    self.request_redraw();
                }
            }

            _ => {}
        }

        false // Don't close window
    }

    /// Handle window focus change for power saving
    pub(crate) fn handle_focus_change(&mut self, focused: bool) {
        if self.is_focused == focused {
            return; // No change
        }

        self.is_focused = focused;

        log::info!(
            "Window focus changed: {}",
            if focused { "focused" } else { "blurred" }
        );

        // Suppress the first mouse click after gaining focus to prevent it from
        // being forwarded to the PTY. Without this, clicking to focus sends a
        // mouse event to tmux (or other mouse-aware apps), which can trigger a
        // zero-char selection that clears the system clipboard.
        if focused {
            let suppressed_recent_unfocused_click = self
                .focus_click_suppressed_while_unfocused_at
                .is_some_and(|t| t.elapsed() <= std::time::Duration::from_millis(500));

            self.focus_click_pending = !suppressed_recent_unfocused_click;
            self.focus_click_suppressed_while_unfocused_at = None;
        } else {
            self.focus_click_pending = false;
            self.focus_click_suppressed_while_unfocused_at = None;
        }

        // Update renderer focus state for unfocused cursor styling
        if let Some(renderer) = &mut self.renderer {
            renderer.set_focused(focused);
        }

        // Handle shader animation pause/resume
        if self.config.pause_shaders_on_blur
            && let Some(renderer) = &mut self.renderer
        {
            if focused {
                // Only resume if user has animation enabled in config
                renderer.resume_shader_animations(
                    self.config.custom_shader_animation,
                    self.config.cursor_shader_animation,
                );
            } else {
                renderer.pause_shader_animations();
            }
        }

        // Re-assert tmux client size when window gains focus
        // This ensures par-term's size is respected even after other clients resize tmux
        if focused {
            self.notify_tmux_of_resize();
        }

        // Forward focus events to all PTYs that have focus tracking enabled (DECSET 1004)
        // This is needed for applications like tmux that rely on focus events
        for tab in self.tab_manager.tabs_mut() {
            // try_lock: intentional — Focused fires in the sync event loop. On miss: the
            // focus change event is not delivered to this terminal/pane. For most TUI apps
            // this means the focus-change visual update (e.g., tmux pane highlight) is
            // delayed one or more frames.
            if let Ok(term) = tab.terminal.try_lock() {
                term.report_focus_change(focused);
            }
            // Also forward to all panes if split panes are active
            if let Some(pm) = &tab.pane_manager {
                for pane in pm.all_panes() {
                    // try_lock: intentional — same rationale as tab terminal above.
                    if let Ok(term) = pane.terminal.try_lock() {
                        term.report_focus_change(focused);
                    }
                }
            }
        }

        // Handle refresh rate adjustment for all tabs
        if self.config.pause_refresh_on_blur
            && let Some(window) = &self.window
        {
            let fps = if focused {
                self.config.max_fps
            } else {
                self.config.unfocused_fps
            };
            for tab in self.tab_manager.tabs_mut() {
                tab.stop_refresh_task();
                tab.start_refresh_task(
                    Arc::clone(&self.runtime),
                    Arc::clone(window),
                    fps,
                    self.config.inactive_tab_fps,
                );
            }
            log::info!(
                "Adjusted refresh rate to {} FPS ({})",
                fps,
                if focused { "focused" } else { "unfocused" }
            );
        }

        // Request a redraw when focus changes
        self.needs_redraw = true;
        self.request_redraw();
    }

    /// Process per-window updates in about_to_wait
    pub(crate) fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Skip all processing if shutting down
        if self.is_shutting_down {
            return;
        }

        // Check for and deliver notifications (OSC 9/777)
        self.check_notifications();

        // Check for file transfer events (downloads, uploads, progress)
        self.check_file_transfers();

        // Check for bell events and play audio/visual feedback
        self.check_bell();

        // Check for trigger action results and dispatch them
        self.check_trigger_actions();

        // Check for activity/idle notifications
        self.check_activity_idle_notifications();

        // Check for session exit notifications
        self.check_session_exit_notifications();

        // Check for shader hot reload events
        if self.check_shader_reload() {
            log::debug!("Shader hot reload triggered redraw");
        }

        // Check for config file changes (e.g., from ACP agent)
        self.check_config_reload();

        // Check for MCP server config updates (.config-update.json)
        self.check_config_update_file();

        // Check for MCP screenshot requests (.screenshot-request.json)
        self.check_screenshot_request_file();

        // Check for tmux control mode notifications
        if self.check_tmux_notifications() {
            self.needs_redraw = true;
        }

        // Update window title with shell integration info (CWD, exit code)
        self.update_window_title_with_shell_integration();

        // Sync shell integration data to badge variables
        self.sync_badge_shell_integration();

        // Check for automatic profile switching based on hostname detection (OSC 7)
        if self.check_auto_profile_switch() {
            self.needs_redraw = true;
        }

        // --- POWER SAVING & SMART REDRAW LOGIC ---
        // We use ControlFlow::WaitUntil to sleep until the next expected event.
        // This drastically reduces CPU/GPU usage compared to continuous polling (ControlFlow::Poll).
        // The loop calculates the earliest time any component needs to update.

        let now = std::time::Instant::now();
        let mut next_wake = now + std::time::Duration::from_secs(1); // Default sleep for 1s of inactivity

        // Calculate frame interval based on focus state for power saving
        // When pause_refresh_on_blur is enabled and window is unfocused, use slower refresh rate
        let frame_interval_ms = if self.config.pause_refresh_on_blur && !self.is_focused {
            // Use unfocused FPS (e.g., 10 FPS = 100ms interval)
            1000 / self.config.unfocused_fps.max(1)
        } else {
            // Use normal animation rate based on max_fps
            1000 / self.config.max_fps.max(1)
        };
        let frame_interval = std::time::Duration::from_millis(frame_interval_ms as u64);

        // Check if enough time has passed since last render for FPS throttling
        let time_since_last_render = self
            .last_render_time
            .map(|t| now.duration_since(t))
            .unwrap_or(frame_interval); // If no last render, allow immediate render
        let can_render = time_since_last_render >= frame_interval;

        // --- FLICKER REDUCTION LOGIC ---
        // When reduce_flicker is enabled and cursor is hidden, delay rendering
        // to batch updates and reduce visual flicker during bulk terminal operations.
        let should_delay_for_flicker = if self.config.reduce_flicker {
            let cursor_hidden = if let Some(tab) = self.tab_manager.active_tab() {
                // try_lock: intentional — flicker check runs in about_to_wait (sync event loop).
                // On miss: assume cursor is visible (false) so rendering is not delayed.
                // Slightly conservative but never causes stale frames.
                if let Ok(term) = tab.terminal.try_lock() {
                    !term.is_cursor_visible() && !self.config.lock_cursor_visibility
                } else {
                    false
                }
            } else {
                false
            };

            if cursor_hidden {
                // Track when cursor was first hidden
                if self.cursor_hidden_since.is_none() {
                    self.cursor_hidden_since = Some(now);
                }

                // Check bypass conditions
                let delay_expired = self
                    .cursor_hidden_since
                    .map(|t| {
                        now.duration_since(t)
                            >= std::time::Duration::from_millis(
                                self.config.reduce_flicker_delay_ms as u64,
                            )
                    })
                    .unwrap_or(false);

                // Bypass for UI interactions (modals + resize overlay)
                let any_ui_visible = self.any_modal_ui_visible() || self.resize_overlay_visible;

                // Delay unless bypass conditions met
                !delay_expired && !any_ui_visible
            } else {
                // Cursor visible - clear tracking and allow render
                if self.cursor_hidden_since.is_some() {
                    self.cursor_hidden_since = None;
                    self.flicker_pending_render = false;
                    self.needs_redraw = true; // Render accumulated updates
                }
                false
            }
        } else {
            false
        };

        // Schedule wake at delay expiry if delaying
        if should_delay_for_flicker {
            self.flicker_pending_render = true;
            if let Some(hidden_since) = self.cursor_hidden_since {
                let delay =
                    std::time::Duration::from_millis(self.config.reduce_flicker_delay_ms as u64);
                let render_time = hidden_since + delay;
                if render_time < next_wake {
                    next_wake = render_time;
                }
            }
        } else if self.flicker_pending_render {
            // Delay ended - trigger accumulated render
            self.flicker_pending_render = false;
            if can_render {
                self.needs_redraw = true;
            }
        }

        // --- THROUGHPUT MODE LOGIC ---
        // When maximize_throughput is enabled, always batch renders regardless of cursor state.
        // Uses a longer interval than flicker reduction for better throughput during bulk output.
        let should_delay_for_throughput = if self.config.maximize_throughput {
            // Initialize batch start time if not set
            if self.throughput_batch_start.is_none() {
                self.throughput_batch_start = Some(now);
            }

            let interval =
                std::time::Duration::from_millis(self.config.throughput_render_interval_ms as u64);
            let batch_start = self.throughput_batch_start
                .expect("throughput_batch_start is Some: set to Some on the line above when None");

            // Check if interval has elapsed
            if now.duration_since(batch_start) >= interval {
                self.throughput_batch_start = Some(now); // Reset for next batch
                false // Allow render
            } else {
                true // Delay render
            }
        } else {
            // Clear tracking when disabled
            if self.throughput_batch_start.is_some() {
                self.throughput_batch_start = None;
            }
            false
        };

        // Schedule wake for throughput mode
        if should_delay_for_throughput && let Some(batch_start) = self.throughput_batch_start {
            let interval =
                std::time::Duration::from_millis(self.config.throughput_render_interval_ms as u64);
            let render_time = batch_start + interval;
            if render_time < next_wake {
                next_wake = render_time;
            }
        }

        // Combine delays: throughput mode OR flicker delay
        let should_delay_render = should_delay_for_throughput || should_delay_for_flicker;

        // 1. Cursor Blinking
        // Wake up exactly when the cursor needs to toggle visibility or fade.
        // Skip cursor blinking when unfocused with pause_refresh_on_blur to save power.
        if self.config.cursor_blink && (self.is_focused || !self.config.pause_refresh_on_blur) {
            if self.cursor_blink_timer.is_none() {
                let blink_interval =
                    std::time::Duration::from_millis(self.config.cursor_blink_interval);
                self.cursor_blink_timer = Some(now + blink_interval);
            }

            if let Some(next_blink) = self.cursor_blink_timer {
                if now >= next_blink {
                    // Time to toggle: trigger redraw (if throttle allows) and schedule next phase
                    if can_render {
                        self.needs_redraw = true;
                    }
                    let blink_interval =
                        std::time::Duration::from_millis(self.config.cursor_blink_interval);
                    self.cursor_blink_timer = Some(now + blink_interval);
                } else if next_blink < next_wake {
                    // Schedule wake-up for the next toggle
                    next_wake = next_blink;
                }
            }
        }

        // 2. Smooth Scrolling & Animations
        // If a scroll interpolation or terminal animation is active, use calculated frame interval.
        if let Some(tab) = self.tab_manager.active_tab() {
            if tab.scroll_state.animation_start.is_some() {
                if can_render {
                    self.needs_redraw = true;
                }
                let next_frame = now + frame_interval;
                if next_frame < next_wake {
                    next_wake = next_frame;
                }
            }

            // 3. Visual Bell Feedback
            // Maintain frame rate during the visual flash fade-out.
            if tab.bell.visual_flash.is_some() {
                if can_render {
                    self.needs_redraw = true;
                }
                let next_frame = now + frame_interval;
                if next_frame < next_wake {
                    next_wake = next_frame;
                }
            }

            // 4. Interactive UI Elements
            // Ensure high responsiveness during mouse dragging (text selection or scrollbar).
            // Always allow these for responsiveness, even if throttled.
            if (tab.mouse.is_selecting
                || tab.mouse.selection.is_some()
                || tab.scroll_state.dragging)
                && tab.mouse.button_pressed
            {
                self.needs_redraw = true;
            }
        }

        // 5. Resize Overlay
        // Check if the resize overlay should be hidden (timer expired).
        if self.resize_overlay_visible
            && let Some(hide_time) = self.resize_overlay_hide_time
        {
            if now >= hide_time {
                // Hide the overlay
                self.resize_overlay_visible = false;
                self.resize_overlay_hide_time = None;
                self.needs_redraw = true;
            } else {
                // Overlay still visible - request redraw and schedule wake
                if can_render {
                    self.needs_redraw = true;
                }
                if hide_time < next_wake {
                    next_wake = hide_time;
                }
            }
        }

        // 5b. Toast Notification
        // Check if the toast notification should be hidden (timer expired).
        if self.toast_message.is_some()
            && let Some(hide_time) = self.toast_hide_time
        {
            if now >= hide_time {
                // Hide the toast
                self.toast_message = None;
                self.toast_hide_time = None;
                self.needs_redraw = true;
            } else {
                // Toast still visible - request redraw and schedule wake
                if can_render {
                    self.needs_redraw = true;
                }
                if hide_time < next_wake {
                    next_wake = hide_time;
                }
            }
        }

        // 5c. Pane Identification Overlay
        // Check if the pane index overlay should be hidden (timer expired).
        if let Some(hide_time) = self.pane_identify_hide_time {
            if now >= hide_time {
                self.pane_identify_hide_time = None;
                self.needs_redraw = true;
            } else {
                if can_render {
                    self.needs_redraw = true;
                }
                if hide_time < next_wake {
                    next_wake = hide_time;
                }
            }
        }

        // 5b. Session undo expiry: prune closed tab metadata that has timed out
        if !self.closed_tabs.is_empty() && self.config.session_undo_timeout_secs > 0 {
            let timeout =
                std::time::Duration::from_secs(self.config.session_undo_timeout_secs as u64);
            self.closed_tabs
                .retain(|info| now.duration_since(info.closed_at) < timeout);
        }

        // 6. Custom Background Shaders
        // If a custom shader is animated, render at the calculated frame interval.
        // When unfocused with pause_refresh_on_blur, this uses the slower unfocused_fps rate.
        if let Some(renderer) = &self.renderer
            && renderer.needs_continuous_render()
        {
            if can_render {
                self.needs_redraw = true;
            }
            // Schedule next frame at the appropriate interval
            let next_frame = self
                .last_render_time
                .map(|t| t + frame_interval)
                .unwrap_or(now);
            // Ensure we don't schedule in the past
            let next_frame = if next_frame <= now {
                now + frame_interval
            } else {
                next_frame
            };
            if next_frame < next_wake {
                next_wake = next_frame;
            }
        }

        // 7. Shader Install Dialog
        // Force continuous redraws when shader install dialog is visible (for spinner animation)
        // and when installation is in progress (to check for completion)
        if self.shader_install_ui.visible {
            self.needs_redraw = true;
            // Schedule frequent redraws for smooth spinner animation
            let next_frame = now + std::time::Duration::from_millis(16); // ~60fps
            if next_frame < next_wake {
                next_wake = next_frame;
            }
        }

        // 8. File Transfer Progress
        // Ensure rendering during active file transfers so the progress overlay
        // updates. Uses 1-second interval since progress doesn't need smooth animation.
        // Bypasses render delays (flicker/throughput) for responsive UI feedback.
        let has_active_file_transfers = !self.file_transfer_state.active_uploads.is_empty()
            || !self.file_transfer_state.recent_transfers.is_empty();
        if has_active_file_transfers {
            self.needs_redraw = true;
            // Schedule 1 FPS rendering for progress bar updates
            let next_frame = now + std::time::Duration::from_secs(1);
            if next_frame < next_wake {
                next_wake = next_frame;
            }
        }

        // 9. Anti-idle Keep-alive
        // Periodically send keep-alive codes to prevent SSH/connection timeouts.
        if let Some(next_anti_idle) = self.handle_anti_idle(now)
            && next_anti_idle < next_wake
        {
            next_wake = next_anti_idle;
        }

        // --- TRIGGER REDRAW ---
        // Request a redraw if any of the logic above determined an update is due.
        // Respect combined delay (throughput mode OR flicker reduction),
        // but bypass delays for active file transfers that need UI feedback.
        let mut redraw_requested = false;
        if self.needs_redraw
            && (!should_delay_render || has_active_file_transfers)
            && let Some(window) = &self.window
        {
            window.request_redraw();
            self.needs_redraw = false;
            redraw_requested = true;
        }

        // Set the calculated sleep interval.
        // Use Poll mode during active file transfers — WaitUntil prevents
        // RedrawRequested events from being delivered on macOS when PTY data
        // events keep the event loop busy.
        if has_active_file_transfers {
            event_loop.set_control_flow(ControlFlow::Poll);
        } else {
            // On macOS, ControlFlow::WaitUntil doesn't always prevent the event loop
            // from spinning (CVDisplayLink and NSRunLoop interactions). Add an explicit
            // sleep when no render is needed to guarantee low CPU usage when idle.
            //
            // Important: keep this independent from max_fps. Using frame interval here
            // causes idle focused windows to wake at render cadence (e.g., 60Hz), which
            // burns CPU even when nothing is changing.
            if !self.needs_redraw && !redraw_requested {
                const FOCUSED_IDLE_SPIN_SLEEP_MS: u64 = 50;
                const UNFOCUSED_IDLE_SPIN_SLEEP_MS: u64 = 100;
                let max_idle_spin_sleep = if self.is_focused {
                    std::time::Duration::from_millis(FOCUSED_IDLE_SPIN_SLEEP_MS)
                } else {
                    std::time::Duration::from_millis(UNFOCUSED_IDLE_SPIN_SLEEP_MS)
                };
                let sleep_until = next_wake.min(now + max_idle_spin_sleep);
                let sleep_dur = sleep_until.saturating_duration_since(now);
                if sleep_dur > std::time::Duration::from_millis(1) {
                    std::thread::sleep(sleep_dur);
                }
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(next_wake));
        }
    }
}

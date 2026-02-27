//! WindowEvent routing and dispatch for WindowState.
//!
//! Contains:
//! - `handle_window_event`: routes winit WindowEvents to terminal/renderer handlers,
//!   including close, resize, scale factor change, keyboard, mouse, focus, redraw, theme change.

use crate::app::window_state::WindowState;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;

impl WindowState {
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
                    && !self.overlay_ui.quit_confirmation_ui.is_visible()
                {
                    log::info!(
                        "Showing quit confirmation dialog ({} active sessions)",
                        tab_count
                    );
                    self.overlay_ui
                        .quit_confirmation_ui
                        .show_confirmation(tab_count);
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
                        } else {
                            crate::debug::record_try_lock_failure("scale_factor_resize");
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
                        } else {
                            crate::debug::record_try_lock_failure("resize");
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

                // Handle shell exit based on configured action (Keep / Close / Restart*).
                // Returns true if the window should close.
                if self.handle_shell_exit() {
                    return true;
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
                    if let Err(e) = self.save_config_debounced() {
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
}

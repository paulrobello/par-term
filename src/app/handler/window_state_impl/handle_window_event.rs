//! WindowEvent routing and dispatch for WindowState.
//!
//! Contains:
//! - `handle_window_event`: routes winit WindowEvents to terminal/renderer handlers,
//!   including close, resize, scale factor change, keyboard, mouse, focus, redraw, theme change.

use crate::app::window_state::WindowState;
use std::sync::Arc;
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

        // Key presses route through the overlay stack first (UX.md OV2):
        // the top overlay owns the key, its toggle chord closes it, Escape
        // closes only it, and a key an open overlay does not use never
        // reaches the PTY. A press the stack resolves itself (close,
        // replace) is withheld from egui so the overlay beneath cannot
        // act on it too.
        let key_route = if let WindowEvent::KeyboardInput {
            event: key_event, ..
        } = &event
        {
            // Modifier and Alt-side tracking run for every key event, even
            // one the stack stops before handle_key_event: a modifier
            // released while an overlay is open must still register.
            self.input_handler.sync_modifier_from_key_event(key_event);
            self.input_handler.track_alt_key(key_event);
            Some(self.route_window_key(key_event))
        } else {
            None
        };
        let feed_egui = key_route
            .as_ref()
            .is_none_or(crate::app::overlay::routing::KeyRoute::feeds_egui);

        // Let egui handle the event (needed for proper rendering state)
        let (egui_consumed, egui_needs_repaint) = if !feed_egui {
            (false, false)
        } else if let (Some(egui_state), Some(window)) = (&mut self.egui.state, &self.window) {
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

        // Apply the overlay route: the stack resolves closes and replaces
        // here; a key an overlay owns or consumes stops here too. Only
        // unowned keys and keyboard-mode keys continue to terminal key
        // dispatch (the mode handlers live in handle_key_event).
        if let Some(route) = &key_route
            && (self.apply_overlay_route(route) || !route.continues_to_key_dispatch())
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
                return self.request_window_close();
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window) {
                    log::info!(
                        "Scale factor changed to {} (display change detected)",
                        scale_factor
                    );

                    let size = window.inner_size();
                    let (cols, rows) = renderer.handle_scale_factor_change(scale_factor, size);

                    // A scale-factor change is a display change, and a
                    // same-config reconfigure heals nothing (measured
                    // 2026-09-21) — recover through the present-mode-cycling
                    // path instead, which also covers an unchanged extent.
                    renderer.reconfigure_after_display_change(size.width, size.height);
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
                        if let Ok(mut term) = tab.terminal.try_write() {
                            if let Err(e) = term.resize_with_pixels(cols, rows, width_px, height_px)
                            {
                                crate::debug_error!(
                                    "TERMINAL",
                                    "resize_with_pixels failed (scale_factor): {e}"
                                );
                            }
                        } else {
                            crate::debug::record_try_lock_failure("scale_factor_resize");
                        }
                    }

                    // Request redraw to apply changes
                    window.request_redraw();
                }
            }

            // Handle window moved to a different monitor. winit delivers
            // `Moved` for every position update during a drag, so this must
            // be edge-triggered on an actual monitor crossing: a same-config
            // `Surface::configure` heals nothing while being a
            // compositor-class disturbance under wgpu 30, and one per move
            // event is a configure storm (a strobe candidate — see
            // `display_recovery::WindowMoveGate`).
            WindowEvent::Moved(_) => {
                let crossed_monitors = match self.window.as_deref() {
                    Some(window) => self.window_move_gate.observe(window.current_monitor()),
                    None => false,
                };
                if crossed_monitors
                    && let (Some(renderer), Some(window)) = (&mut self.renderer, &self.window)
                {
                    log::info!("Window moved to a different monitor — healing reconfigure");

                    let size = window.inner_size();
                    renderer.reconfigure_after_display_change(size.width, size.height);

                    // Request redraw to ensure proper rendering on new display
                    window.request_redraw();
                }
            }

            WindowEvent::Resized(physical_size) => {
                // Fullscreen transitions (keyboard, menu, green button) all emit
                // Resized — sync the macOS titlebar backing bar's visibility here
                // so no transition path is missed. No-op when translucency is off.
                if let Some(window) = &self.window {
                    let _ = crate::macos_metal::sync_titlebar_backing_bar_visibility(window);
                }

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
                        let new_scrollback_len = if let Ok(mut term) = tab.terminal.try_write() {
                            if let Err(e) = term.resize_with_pixels(cols, rows, width_px, height_px)
                            {
                                crate::debug_error!(
                                    "TERMINAL",
                                    "resize_with_pixels failed (Resized): {e}"
                                );
                            }
                            Some(term.scrollback_len())
                        } else {
                            crate::debug::record_try_lock_failure("resize");
                            None
                        };
                        if let Some(sl) = new_scrollback_len {
                            tab.active_cache_mut().scrollback_len = sl;
                        }
                        // Invalidate cell cache to force regeneration
                        tab.active_cache_mut().cells = None;
                    }

                    // Update scrollbar for active tab
                    if let Some(tab) = self.tab_manager.active_tab() {
                        let total_lines = rows + tab.active_cache().scrollback_len;
                        // try_lock: intentional — scrollbar mark update during Resized event.
                        // On miss: scrollbar renders without marks this frame. Cosmetic only.
                        let marks = tab
                            .try_with_terminal(|term| term.scrollback_marks())
                            .unwrap_or_default();
                        renderer.update_scrollbar(
                            tab.active_scroll_state().offset,
                            rows,
                            total_lines,
                            &marks,
                        );
                    }

                    // Update resize overlay state
                    self.overlay_state.resize_dimensions =
                        Some((physical_size.width, physical_size.height, cols, rows));
                    self.overlay_state.resize_overlay_visible = true;
                    // Hide overlay 1 second after resize stops
                    self.overlay_state.resize_overlay_hide_time =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(1));

                    // Notify tmux of the new size if gateway mode is active
                    self.notify_tmux_of_resize();

                    // --- Snap window to grid cell boundaries ---
                    //
                    // Goal: eliminate the partial-cell gap between the terminal grid
                    // and the window edge that appears after a user drag.
                    //
                    // Anti-loop guard: if this Resized event IS the response to our
                    // own request_inner_size call, skip — we're already at the snapped size.
                    // We allow a ±1 px tolerance because the OS may round to even physical
                    // pixels (e.g. Retina 2× requires integer logical pixels).
                    if let Some(pending) = self.pending_snap_size.take() {
                        let dw = (pending.width as i32 - physical_size.width as i32).unsigned_abs();
                        let dh =
                            (pending.height as i32 - physical_size.height as i32).unsigned_abs();
                        if dw > 1 || dh > 1 {
                            // Not our snap response (concurrent user drag or OS constraint).
                            // Clear pending — the snap logic below will re-evaluate.
                            crate::debug_info!(
                                "RESIZE",
                                "snap guard: pending {}x{} != physical {}x{} (dw={} dh={}), clearing",
                                pending.width,
                                pending.height,
                                physical_size.width,
                                physical_size.height,
                                dw,
                                dh
                            );
                        }
                        // else: this resize was triggered by our own snap request — done.
                    }

                    if self.pending_snap_size.is_none()
                        && self.config.load().window.snap_window_to_grid
                    {
                        // Only snap in single-pane mode (split pane handled separately).
                        let is_split = self
                            .tab_manager
                            .active_tab()
                            .map(|t| t.pane_count() > 1)
                            .unwrap_or(false);

                        if !is_split && let Some(renderer) = &self.renderer {
                            let (chrome_x, chrome_y) = renderer.chrome_overhead();
                            let cell_w = renderer.cell_width();
                            let cell_h = renderer.cell_height();
                            let snapped_w = (chrome_x + cols as f32 * cell_w).round() as u32;
                            let snapped_h = (chrome_y + rows as f32 * cell_h).round() as u32;

                            crate::debug_info!(
                                "RESIZE",
                                "snap: physical={}x{} snapped={}x{} chrome=({:.1},{:.1}) cell=({:.1},{:.1}) grid={}x{}",
                                physical_size.width,
                                physical_size.height,
                                snapped_w,
                                snapped_h,
                                chrome_x,
                                chrome_y,
                                cell_w,
                                cell_h,
                                cols,
                                rows
                            );

                            if snapped_w != physical_size.width || snapped_h != physical_size.height
                            {
                                let snapped = winit::dpi::PhysicalSize::new(snapped_w, snapped_h);
                                self.pending_snap_size = Some(snapped);
                                self.with_window(|w| {
                                    let _ = w.request_inner_size(snapped);
                                });
                            }
                        }
                    }
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_key_event(event, event_loop);
            }

            WindowEvent::Ime(ime_event) => {
                self.handle_ime_event(ime_event);
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                self.input_handler.update_modifiers(modifiers);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                // Check if mouse is over the tab bar — convert vertical wheel
                // to horizontal tab scrolling when the bar overflows.
                let mouse_position = self
                    .tab_manager
                    .active_tab()
                    .map(|t| t.active_mouse().position)
                    .unwrap_or((0.0, 0.0));
                let tab_count = self.tab_manager.visible_tab_count();

                if self.is_mouse_in_tab_bar(mouse_position)
                    && self.tab_bar_ui.handle_mouse_wheel(
                        &delta,
                        self.config.load().tab_colors.tab_min_width,
                        tab_count,
                    )
                {
                    self.request_redraw();
                } else if !any_ui_visible && !self.is_egui_using_pointer() {
                    // Skip terminal handling if egui UI is visible or using the pointer
                    // Note: any_ui_visible check is needed because is_egui_using_pointer()
                    // returns false before egui is initialized (e.g., at startup when
                    // shader_install_ui is shown before first render)
                    self.handle_mouse_wheel(delta);
                }
            }

            WindowEvent::MouseInput { button, state, .. } => {
                use winit::event::ElementState;
                use winit::event::MouseButton;

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
                    && (self.focus_state.focus_click_pending || !self.focus_state.is_focused);
                if is_focus_click_press {
                    self.focus_state.focus_click_pending = false;
                    if !self.focus_state.is_focused {
                        self.focus_state.focus_click_suppressed_while_unfocused_at =
                            Some(std::time::Instant::now());
                    }
                    // If the focus click landed in the tab bar, let it through so the
                    // tab switch registers in egui. Only suppress clicks in the terminal
                    // area to prevent PTY mouse-tracking apps from seeing the focus click.
                    let mouse_position = self
                        .tab_manager
                        .active_tab()
                        .map(|t| t.active_mouse().position)
                        .unwrap_or((0.0, 0.0));
                    if self.is_mouse_in_tab_bar(mouse_position) {
                        // Direct hit-test against the tab rects cached from the last render
                        // frame.  egui's clicked_by() can miss focus-clicks because pointer
                        // state may be stale when the window was unfocused.  Storing the
                        // target here lets post_render apply the switch as a fallback.
                        let scale_factor = self
                            .window
                            .as_ref()
                            .map(|w| w.scale_factor())
                            .unwrap_or(1.0) as f32;
                        let logical_pos = egui::pos2(
                            mouse_position.0 as f32 / scale_factor,
                            mouse_position.1 as f32 / scale_factor,
                        );
                        self.focus_state.pending_focus_tab_switch =
                            self.tab_bar_ui.tab_at_logical_pos(logical_pos);

                        // Don't suppress — egui needs both press and release to fire clicked_by()
                        self.focus_state.ui_consumed_mouse_press = false;
                        self.begin_clipboard_image_click_guard(button, state);
                        self.handle_mouse_button(button, state);
                        self.finish_clipboard_image_click_guard(button, state);
                    } else {
                        self.focus_state.ui_consumed_mouse_press = true; // Also suppress the release
                        self.request_redraw();
                    }
                } else {
                    // Track UI mouse consumption to prevent release events bleeding through
                    // when UI closes during a click (e.g., drawer toggle)
                    let ui_wants_pointer = any_ui_visible || self.is_egui_using_pointer();

                    if state == ElementState::Pressed {
                        if ui_wants_pointer {
                            self.focus_state.ui_consumed_mouse_press = true;
                            self.request_redraw();
                        } else {
                            self.focus_state.ui_consumed_mouse_press = false;
                            self.begin_clipboard_image_click_guard(button, state);
                            self.handle_mouse_button(button, state);
                            self.finish_clipboard_image_click_guard(button, state);
                        }
                    } else {
                        // Release: block if we consumed the press OR if UI wants pointer
                        if self.focus_state.ui_consumed_mouse_press || ui_wants_pointer {
                            self.focus_state.ui_consumed_mouse_press = false;

                            // Clear terminal mouse state when the release is consumed
                            // here instead of in handle_mouse_button(). Without this,
                            // a press that set button_pressed=true in the terminal
                            // followed by a release consumed by egui (e.g., pointer
                            // moved into the tab bar) leaves button_pressed stuck as
                            // true. The next mouse-move into the terminal then starts
                            // an accidental drag-selection.
                            if button == MouseButton::Left
                                && let Some(tab) = self.tab_manager.active_tab_mut()
                            {
                                tab.active_mouse_mut().button_pressed = false;
                                tab.selection_mouse_mut().is_selecting = false;
                            }

                            self.request_redraw();
                        } else {
                            self.begin_clipboard_image_click_guard(button, state);
                            self.handle_mouse_button(button, state);
                            self.finish_clipboard_image_click_guard(button, state);
                        }
                    }
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                // Always update the stored mouse position so that hit-testing
                // (is_mouse_in_tab_bar, pixel_to_cell, etc.) uses the latest
                // coordinates even when egui claims the pointer and
                // handle_mouse_move() is skipped.  Without this, a stale position
                // can cause a subsequent MouseInput press to bypass the tab-bar
                // guard and leak through to tmux mouse tracking.
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.active_mouse_mut().position = (position.x, position.y);
                }

                // Skip terminal handling if egui UI is visible or using the pointer
                if any_ui_visible || self.is_egui_using_pointer() {
                    // Request redraw so egui can update hover states
                    self.request_redraw();
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
                if self.config.load().mouse.focus_follows_mouse
                    && let Some(window) = &self.window
                {
                    window.focus_window();
                }
            }

            WindowEvent::ThemeChanged(system_theme) => {
                let is_dark = system_theme == winit::window::Theme::Dark;
                // Apply theme changes via rcu (apply_system_theme/tab_style require &mut self)
                let theme_changed = {
                    let old = self.config.load();
                    // Check if change would occur using a clone (apply_system_theme needs &mut)
                    let mut probe = (**old).clone();
                    let changed = probe.apply_system_theme(is_dark);
                    drop(old);
                    if changed {
                        self.config.rcu(|old| {
                            let mut new = (**old).clone();
                            if new.apply_system_theme(is_dark) {
                                Arc::new(new)
                            } else {
                                Arc::clone(old)
                            }
                        });
                    }
                    changed
                };
                let tab_style_changed = {
                    let old = self.config.load();
                    let mut probe = (**old).clone();
                    let changed = probe.apply_system_tab_style(is_dark);
                    drop(old);
                    if changed {
                        self.config.rcu(|old| {
                            let mut new = (**old).clone();
                            if new.apply_system_tab_style(is_dark) {
                                Arc::new(new)
                            } else {
                                Arc::clone(old)
                            }
                        });
                    }
                    changed
                };

                if theme_changed {
                    log::info!(
                        "System theme changed to {}, switching to theme: {}",
                        if is_dark { "dark" } else { "light" },
                        self.config.load().theme_colors.theme
                    );
                    let theme = self.config.load().load_theme();
                    for tab in self.tab_manager.tabs_mut() {
                        // try_lock: intentional — ThemeChanged fires in the sync event loop.
                        // On miss: this tab keeps the old theme until the next theme event
                        // or config reload. Cell cache is still invalidated to prevent stale
                        // rendering with the old theme colors.
                        if let Ok(mut term) = tab.terminal.try_write() {
                            term.set_theme(theme.clone());
                        }
                        // Apply to split pane terminals (primary pane shares tab.terminal).
                        // Theme changes recolor cells without bumping update_generation,
                        // so every pane's cross-frame cell cache must be invalidated too.
                        let tab_terminal = std::sync::Arc::clone(&tab.terminal);
                        if let Some(pm) = tab.pane_manager_mut() {
                            for pane in pm.all_panes_mut() {
                                if !std::sync::Arc::ptr_eq(&pane.terminal, &tab_terminal)
                                    && let Ok(mut term) = pane.terminal.try_write()
                                {
                                    term.set_theme(theme.clone());
                                }
                                pane.cache.invalidate_pane_cells();
                            }
                        }
                        tab.active_cache_mut().cells = None;
                    }
                }

                if tab_style_changed {
                    log::info!(
                        "Auto tab style: switching to {} tab style",
                        if is_dark {
                            self.config.load().tabs.dark_tab_style.display_name()
                        } else {
                            self.config.load().tabs.light_tab_style.display_name()
                        }
                    );
                }

                if theme_changed || tab_style_changed {
                    self.render_loop.external_config_changes.push(
                        crate::app::window_state::ExternalConfigChange::SystemTheme { is_dark },
                    );
                    self.focus_state.needs_redraw = true;
                    self.request_redraw();
                }
            }

            _ => {}
        }

        false // Don't close window
    }
}

impl WindowState {
    /// A request to close this whole window — the title-bar close and the
    /// `close_window` action (UX.md A14) share it. Returns true when the
    /// window may close now; false when a confirmation dialog is shown.
    ///
    /// Close safety (D6, iTerm2 alignment): ask before closing a window
    /// that holds more than one tab; a single-tab window closes silently
    /// because the session-undo window (5 s, shell preserved) can restore
    /// it.
    pub(crate) fn request_window_close(&mut self) -> bool {
        let tab_count = self.tab_manager.visible_tab_count();
        if should_confirm_window_close(&self.config.load().shell, tab_count)
            && !self.overlay_ui.quit_confirmation_ui.is_visible()
        {
            log::info!("Showing quit confirmation dialog ({} open tabs)", tab_count);
            // UX.md M9: when a par-mux session is attached, the dialog must
            // say it survives the quit (it detaches).
            let mux_session = self
                .tmux_state
                .tmux_session_name
                .clone()
                .filter(|_| self.tmux_state.is_mux_attached());
            self.overlay_ui
                .quit_confirmation_ui
                .show_confirmation(tab_count, mux_session.as_deref());
            self.focus_state.needs_redraw = true;
            self.request_redraw();
            return false;
        }
        self.perform_shutdown();
        true
    }
}

/// The window-close confirmation rule (D6, iTerm2 alignment): with the
/// multi-tab guard on, ask only when the window holds more than one tab —
/// a single-tab close is silently undoable for the session-undo window
/// (5 s, shell preserved). With the guard off, `prompt_on_quit` keeps the
/// older ask-on-any-non-empty-window behavior.
fn should_confirm_window_close(shell: &crate::config::ShellConfig, visible_tabs: usize) -> bool {
    if shell.confirm_close_multiple_tabs {
        visible_tabs > 1
    } else {
        shell.prompt_on_quit && visible_tabs > 0
    }
}

#[cfg(test)]
mod tests {
    use super::should_confirm_window_close;
    use crate::config::ShellConfig;

    fn shell(prompt_on_quit: bool, confirm_close_multiple_tabs: bool) -> ShellConfig {
        ShellConfig {
            prompt_on_quit,
            confirm_close_multiple_tabs,
            ..ShellConfig::default()
        }
    }

    #[test]
    fn multi_tab_window_asks_single_tab_does_not() {
        // The D6 defaults.
        let d6 = shell(true, true);
        assert!(should_confirm_window_close(&d6, 2), "two tabs must ask");
        assert!(!should_confirm_window_close(&d6, 1), "one tab must not ask");
        assert!(!should_confirm_window_close(&d6, 0), "no tabs must not ask");
    }

    #[test]
    fn prompt_on_quit_alone_keeps_the_legacy_ask_on_any_session() {
        let legacy = shell(true, false);
        assert!(should_confirm_window_close(&legacy, 1));
        assert!(should_confirm_window_close(&legacy, 3));
    }

    #[test]
    fn both_flags_off_never_asks() {
        let silent = shell(false, false);
        assert!(!should_confirm_window_close(&silent, 1));
        assert!(!should_confirm_window_close(&silent, 4));
    }

    #[test]
    fn multi_tab_guard_asks_even_with_prompt_on_quit_off() {
        let guard_only = shell(false, true);
        assert!(should_confirm_window_close(&guard_only, 2));
        assert!(!should_confirm_window_close(&guard_only, 1));
    }
}

//! WindowEvent routing and dispatch for WindowState.
//!
//! Contains:
//! - `handle_window_event`: routes winit WindowEvents to terminal/renderer handlers,
//!   including close, resize, scale factor change, keyboard, mouse, focus, redraw, theme change.

use super::{
    window_event_display as display, window_event_pointer as pointer, window_event_theme as theme,
};
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
                display::scale_factor_changed(self, scale_factor);
            }

            // Handle window moved to a different monitor. winit delivers
            // `Moved` for every position update during a drag, so this must
            // be edge-triggered on an actual monitor crossing: a same-config
            // `Surface::configure` heals nothing while being a
            // compositor-class disturbance under wgpu 30, and one per move
            // event is a configure storm (a strobe candidate — see
            // `display_recovery::WindowMoveGate`).
            WindowEvent::Moved(_) => {
                display::moved(self);
            }

            WindowEvent::Resized(physical_size) => {
                display::resized(self, physical_size);
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
                pointer::mouse_wheel(self, delta, any_ui_visible);
            }

            WindowEvent::MouseInput { button, state, .. } => {
                pointer::mouse_input(self, button, state, any_ui_visible);
            }

            WindowEvent::CursorMoved { position, .. } => {
                pointer::cursor_moved(self, position, any_ui_visible);
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
                theme::theme_changed(self, system_theme);
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

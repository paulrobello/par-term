//! IME (input method editor) event handling for WindowState.
//!
//! winit on macOS suppresses `KeyboardInput` while preedit is active and on
//! commit, so composed text — dead-key accents (Option+e then e → é) and IME
//! conversions (kana → kanji) — arrives only as `WindowEvent::Ime`. Until this
//! module existed those events were swallowed by the catch-all arm in
//! `handle_window_event`, dropping the input entirely.
//!
//! - `Commit`: routed to the focused pane through the same seam as keystrokes
//!   (`send_input_via_tmux` first, then the focused pane's PTY).
//! - `Preedit`/`Enabled`: position the OS candidate window at the terminal
//!   cursor via `set_ime_cursor_area`; the preedit string is stored in
//!   `overlay_state.ime_preedit` for the egui overlay to draw.
//! - `Disabled`: clear composition state.

use crate::app::window_state::WindowState;
use std::sync::Arc;
use winit::event::Ime;

impl WindowState {
    /// Handle a winit IME event for this window.
    pub(crate) fn handle_ime_event(&mut self, ime: Ime) {
        // While a modal UI is open, egui owns IME (it handles Ime events
        // itself); mirror the keyboard path, which blocks terminal input
        // whenever any UI is visible.
        if self.any_modal_ui_visible() {
            return;
        }

        match ime {
            Ime::Enabled => {
                // The candidate window should appear at the terminal cursor.
                self.update_ime_cursor_area();
            }
            Ime::Preedit(text, _) => {
                if self.overlay_state.ime_preedit.as_deref() != Some(text.as_str()) {
                    self.overlay_state.ime_preedit = Some(text);
                    self.focus_state.needs_redraw = true;
                    self.request_redraw();
                }
                self.update_ime_cursor_area();
            }
            Ime::Commit(text) => {
                // winit sends an empty Preedit right before Commit; clear here
                // too so the overlay never outlives the composition.
                self.overlay_state.ime_preedit = None;
                self.focus_state.needs_redraw = true;
                self.commit_ime_text(&text);
                self.update_ime_cursor_area();
            }
            Ime::Disabled => {
                if self.overlay_state.ime_preedit.take().is_some() {
                    self.focus_state.needs_redraw = true;
                    self.request_redraw();
                }
            }
        }
    }

    /// Send committed IME text to the focused pane as typed bytes.
    ///
    /// Same routing seam as keystrokes: mux/tmux gateway first via
    /// `send_input_via_tmux`, then the focused pane's terminal. Bracketed
    /// paste is deliberately NOT used — the text is typed input, not a paste.
    fn commit_ime_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let bytes = text.as_bytes().to_vec();

        if self.send_input_via_tmux(&bytes) {
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.activity.anti_idle_last_activity = std::time::Instant::now();
            }
            return;
        }

        // Gateway lock contention: send_input_via_tmux can fail because the
        // gateway terminal's RwLock is held. Do NOT fall through to the direct
        // PTY write — that writes to the wrong terminal. Retry asynchronously,
        // exactly like the keystroke path in handle_key_event.
        if self.is_tmux_connected() {
            crate::debug_info!(
                "IME",
                "Gateway lock contention — queuing {} bytes for async delivery",
                bytes.len()
            );
            let cmd = if let Some(session) = &self.tmux_state.tmux_session {
                match session.format_send_keys(&bytes) {
                    Some(c) => c,
                    None => {
                        let escaped = crate::tmux::escape_keys_for_tmux(&bytes);
                        format!("send-keys {}\n", escaped)
                    }
                }
            } else {
                let escaped = crate::tmux::escape_keys_for_tmux(&bytes);
                format!("send-keys {}\n", escaped)
            };
            if let Some(gateway_tab_id) = self.tmux_state.tmux_gateway_tab_id
                && let Some(tab) = self.tab_manager.get_tab(gateway_tab_id)
            {
                let terminal_clone = Arc::clone(&tab.terminal);
                let cmd_bytes = cmd.into_bytes();
                self.runtime.spawn(async move {
                    let term = terminal_clone.read().await;
                    if let Err(e) = term.write(&cmd_bytes) {
                        crate::debug_error!("INPUT", "PTY write failed (IME send-keys): {e}");
                    }
                });
            }
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.activity.anti_idle_last_activity = std::time::Instant::now();
            }
            return;
        }

        // Local pane: write to the focused pane's terminal (or the tab's main
        // terminal for single-pane tabs). read() not write(): TerminalManager's
        // write takes &self — see the locking note in handle_key_event.
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.activity.anti_idle_last_activity = std::time::Instant::now();
            let terminal_clone = if let Some(ref pane_manager) = tab.pane_manager {
                if let Some(focused_pane) = pane_manager.focused_pane() {
                    Arc::clone(&focused_pane.terminal)
                } else {
                    Arc::clone(&tab.terminal)
                }
            } else {
                Arc::clone(&tab.terminal)
            };
            self.runtime.spawn(async move {
                let term = terminal_clone.read().await;
                if let Err(e) = term.write(&bytes) {
                    crate::debug_error!("INPUT", "PTY write failed (IME commit): {e}");
                }
            });
        }
    }

    /// Tell the OS where the text caret is so the IME candidate window and the
    /// committed text land at the terminal cursor.
    fn update_ime_cursor_area(&self) {
        let Some((x, y, w, h)) = self.focused_cursor_pixel_rect() else {
            return;
        };
        if let Some(window) = &self.window {
            window.set_ime_cursor_area(
                winit::dpi::PhysicalPosition::new(x, y),
                winit::dpi::PhysicalSize::new(w, h),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Ime;
    use crate::app::window_state::WindowState;

    /// A `WindowState` with no window or renderer — the `manners_state` seam.
    fn window_state() -> WindowState {
        let runtime = std::sync::Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build test runtime"),
        );
        WindowState::new(crate::config::Config::default(), runtime)
    }

    #[test]
    fn ime_preedit_is_stored_and_cleared_by_disabled() {
        let mut ws = window_state();
        ws.handle_ime_event(Ime::Preedit("あ".into(), None));
        assert_eq!(ws.overlay_state.ime_preedit.as_deref(), Some("あ"));
        ws.handle_ime_event(Ime::Disabled);
        assert_eq!(ws.overlay_state.ime_preedit, None);
    }

    /// IME commit must reach the focused pane's PTY as typed bytes. On macOS,
    /// dead-key composition (Option+e then e) arrives only as `Ime::Commit("é")`
    /// — winit suppresses the KeyboardInput during composition — so before IME
    /// handling existed the composed character was dropped entirely.
    ///
    /// `cat` echoes the PTY write back through the terminal, so the composed
    /// character appearing in the grid is the proof the write happened.
    #[cfg(unix)]
    #[test]
    fn ime_commit_writes_composed_text_to_local_pane() {
        use std::time::{Duration, Instant};

        let runtime = std::sync::Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build test runtime"),
        );
        let config = crate::config::Config {
            shell: crate::config::ShellConfig {
                custom_shell: Some("cat".into()),
                login_shell: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let tab_config = config.clone();
        let mut ws = WindowState::new(config, std::sync::Arc::clone(&runtime));
        ws.tab_manager
            .new_tab(&tab_config, runtime, false, Some((80, 24)))
            .expect("create cat tab");

        // The dead-key sequence shape: preedit while composing, then commit.
        ws.handle_ime_event(Ime::Preedit("´".into(), Some((0, 0))));
        assert_eq!(ws.overlay_state.ime_preedit.as_deref(), Some("´"));

        ws.handle_ime_event(Ime::Commit("é".into()));
        assert_eq!(
            ws.overlay_state.ime_preedit, None,
            "commit clears the preedit overlay state"
        );

        // The PTY write runs as a task on the window's runtime; tick the
        // current-thread runtime while polling the grid for the echo.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            ws.runtime.block_on(async {
                tokio::time::sleep(Duration::from_millis(50)).await;
            });
            let tab = ws.tab_manager.active_tab().expect("active tab");
            let term = tab.terminal.try_read().expect("terminal lock");
            let screen = term.export_text();
            if screen.contains('é') {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "IME commit never reached the pane; screen so far:\n{screen}"
            );
        }
    }
}

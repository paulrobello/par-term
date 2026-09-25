//! tmux input routing: send/paste to tmux sessions and prefix key handling.

use crate::app::window_state::WindowState;

impl WindowState {
    /// Send input through tmux gateway mode.
    ///
    /// When in gateway mode, keyboard input is sent via `send-keys` command
    /// written to the gateway tab's PTY. This routes input to the appropriate tmux pane.
    ///
    /// Returns true if input was handled via tmux, false if it should go to PTY directly.
    pub fn send_input_via_tmux(&self, data: &[u8]) -> bool {
        // par-mux transport routes input through the daemon client — but
        // only when the user is focused on a MUX pane. A local tab's input
        // must fall through to its own PTY; consuming it here starves every
        // non-mux tab the moment a transport is attached.
        #[cfg(feature = "mux")]
        {
            if let Some(transport) = &self.tmux_state.transport {
                // The daemon REQUIRES -t (untargeted send-keys is an error),
                // and nothing sets mux_focused_pane until a pane click or a
                // daemon focus push — so a fresh attach with keyboard focus
                // only must fall back to the pane the user is actually
                // focused on, via the native→tmux reverse map.
                let focused = self.focused_mux_pane_from_native();
                return match focused {
                    Some(focused) => {
                        super::notifications::mux::route_input(&**transport, Some(focused), data)
                    }
                    // Not on a mux pane: let the caller write to the local PTY.
                    None => false,
                };
            }
        }

        // Check if tmux is enabled and connected
        if !self.config.load().tmux.tmux_enabled || !self.is_tmux_connected() {
            crate::debug_trace!(
                "TMUX",
                "send_input_via_tmux: not sending - enabled={}, connected={}",
                self.config.load().tmux.tmux_enabled,
                self.is_tmux_connected()
            );
            return false;
        }

        let session = match &self.tmux_state.tmux_session {
            Some(s) => s,
            None => return false,
        };

        // Format the send-keys command - try pane-specific first
        let cmd = match session.format_send_keys(data) {
            Some(c) => {
                crate::debug_trace!("TMUX", "Using pane-specific send-keys: {}", c.trim());
                c
            }
            None => {
                crate::debug_trace!("TMUX", "No focused pane for send-keys, trying window-based");
                // No focused pane - try window-based routing
                if let Some(cmd) = self.format_send_keys_for_window(data) {
                    crate::debug_trace!("TMUX", "Using window-based send-keys: {}", cmd.trim());
                    cmd
                } else {
                    // No window mapping either - use untargeted send-keys
                    // This sends to tmux's currently active pane
                    let escaped = crate::tmux::escape_keys_for_tmux(data);
                    format!("send-keys {}\n", escaped)
                }
            }
        };

        // Write the command to the gateway tab's PTY
        if self.write_to_gateway(&cmd) {
            crate::debug_trace!("TMUX", "Sent {} bytes via gateway send-keys", data.len());
            return true;
        }

        false
    }

    /// Send raw bytes to the focused tmux pane as literal input, bypassing tmux's
    /// key-name interpretation and any per-pane modifyOtherKeys / extended-keys
    /// re-encoding.
    ///
    /// Uses `send-keys -H`, which tags each byte as `KEYC_LITERAL`. tmux writes
    /// these bytes straight to the pane's PTY via `bufferevent_write`, skipping
    /// the `input_key()` encoder entirely — so whatever we put in arrives
    /// unchanged at the inner application.
    ///
    /// Used for cases where the normal `send-keys` path would mangle the bytes,
    /// e.g. Shift+Enter: the iTerm2 convention is to send raw LF (0x0a), but
    /// `escape_keys_for_tmux` translates that to `C-j`, and tmux's
    /// modifyOtherKeys-mode-2 encoder then delivers `\x1b[27;5;106~` instead of
    /// the literal newline the application expects.
    pub fn send_literal_bytes_via_tmux(&self, bytes: &[u8]) -> bool {
        if !self.config.load().tmux.tmux_enabled || !self.is_tmux_connected() {
            crate::debug_info!(
                "SHIFTENTER",
                "send_literal_bytes_via_tmux: refused - enabled={}, connected={}",
                self.config.load().tmux.tmux_enabled,
                self.is_tmux_connected(),
            );
            return false;
        }

        let session = match &self.tmux_state.tmux_session {
            Some(s) => s,
            None => {
                // par-mux transport: the literal form routes through the
                // daemon client too (send-keys -H).
                #[cfg(feature = "mux")]
                {
                    if let Some(transport) = &self.tmux_state.transport {
                        let focused = self.focused_mux_pane_from_native();
                        return match focused {
                            Some(focused) => super::notifications::mux::route_literal_bytes(
                                &**transport,
                                Some(focused),
                                bytes,
                            ),
                            // Not on a mux pane: local PTY input.
                            None => false,
                        };
                    }
                }
                crate::debug_info!("SHIFTENTER", "send_literal_bytes_via_tmux: no tmux_session");
                return false;
            }
        };

        let focused = session.focused_pane();
        let cmd = match session.format_send_hex_keys(bytes) {
            Some(c) => c,
            None => {
                crate::debug_info!(
                    "SHIFTENTER",
                    "format_send_hex_keys returned None (focused_pane={:?}, state={:?}, gateway={:?})",
                    focused,
                    session.state(),
                    session.gateway_state(),
                );
                return false;
            }
        };

        crate::debug_info!(
            "SHIFTENTER",
            "writing to gateway (focused_pane={:?}): {:?}",
            focused,
            cmd.trim_end(),
        );

        if self.write_to_gateway(&cmd) {
            crate::debug_info!(
                "SHIFTENTER",
                "send_literal_bytes_via_tmux: success ({} bytes)",
                bytes.len()
            );
            return true;
        }

        crate::debug_info!("SHIFTENTER", "write_to_gateway failed");
        false
    }

    /// The mux pane that input, splits, and size pushes target: the
    /// focused native pane's tmux id, or `None` when the user is on a LOCAL
    /// pane (the caller then writes to the local PTY). The focused native
    /// pane is authoritative. The tracked `mux_focused_pane` used to win,
    /// and it outlived its tab: closing the mux tab left it set, so every
    /// keystroke in the remaining local tab went to a daemon pane that no
    /// longer existed.
    #[cfg(feature = "mux")]
    pub(crate) fn focused_mux_pane_from_native(&self) -> Option<u64> {
        let tab = self.tab_manager.active_tab()?;
        let pane = tab.pane_manager()?.focused_pane()?;
        self.tmux_state.tmux_pane_in_tab(tab.id, pane.id)
    }

    /// Route an encoded mouse report to the focused par-mux pane's daemon
    /// PTY. Returns `true` when it was sent that way; `false` means the
    /// focused pane is not a mux pane and the caller writes locally.
    ///
    /// A mux pane's local terminal has no PTY — it only mirrors the daemon
    /// — so a local write silently dropped every click, drag, and wheel
    /// event (mouse-aware TUIs in mux panes ignored the mouse entirely).
    /// The raw bytes go as `send-keys -H` so key-name translation cannot
    /// mangle the escape sequence.
    #[cfg_attr(not(feature = "mux"), allow(unused_variables))]
    pub(crate) fn route_mouse_report_to_mux(&self, encoded: &[u8]) -> bool {
        #[cfg(feature = "mux")]
        if let Some(transport) = &self.tmux_state.transport
            && let Some(pane) = self.focused_mux_pane_from_native()
        {
            return super::notifications::mux::route_literal_bytes(
                &**transport,
                Some(pane),
                encoded,
            );
        }
        false
    }

    /// Format send-keys command for a specific window (if mapping exists)
    fn format_send_keys_for_window(&self, data: &[u8]) -> Option<String> {
        let active_tab_id = self.tab_manager.active_tab_id()?;

        // Find the tmux window for this tab
        let tmux_window_id = self.tmux_state.tmux_sync.get_window(active_tab_id)?;

        // Format send-keys command with window target using proper escaping
        let escaped = crate::tmux::escape_keys_for_tmux(data);
        Some(format!("send-keys -t @{} {}\n", tmux_window_id, escaped))
    }

    /// Send input via tmux window target (fallback when no pane ID is set).
    ///
    /// This method is a planned fallback path for `TmuxSync` integration: when
    /// the pane-level routing in `send_input_bytes` cannot resolve a pane ID,
    /// routing via the tmux window target (`@N`) is the intended recovery.
    /// It duplicates part of `format_send_keys_for_window` on purpose — the
    /// caller needs to write directly rather than just format the command.
    ///
    /// Not yet wired up because `TmuxSync::get_window` is not yet called from
    /// the hot-path input handler. Wire it up when implementing pane-less
    /// gateway fallback (tracked as a GitHub issue under "tmux integration").
    #[allow(dead_code)] // Infrastructure for TmuxSync pane-less fallback — not yet wired
    fn send_input_via_tmux_window(&self, data: &[u8]) -> bool {
        let active_tab_id = match self.tab_manager.active_tab_id() {
            Some(id) => id,
            None => return false,
        };

        // Find the tmux window for this tab
        let tmux_window_id = match self.tmux_state.tmux_sync.get_window(active_tab_id) {
            Some(id) => id,
            None => {
                crate::debug_trace!(
                    "TMUX",
                    "No tmux window mapping for tab {}, using untargeted send-keys",
                    active_tab_id
                );
                return false;
            }
        };

        // Format send-keys command with window target using proper escaping
        let escaped = crate::tmux::escape_keys_for_tmux(data);
        let cmd = format!("send-keys -t @{} {}\n", tmux_window_id, escaped);

        // Write to gateway tab
        if self.write_to_gateway(&cmd) {
            crate::debug_trace!(
                "TMUX",
                "Sent {} bytes via gateway to window @{}",
                data.len(),
                tmux_window_id
            );
            return true;
        }

        false
    }

    /// Send paste text through tmux gateway mode.
    ///
    /// Uses send-keys -l for literal text to handle special characters properly.
    pub fn paste_via_tmux(&self, text: &str) -> bool {
        // par-mux transport: the panes live in the daemon and the local
        // mirror has no PTY, so falling through to a local paste would be
        // dropped. Route the bytes daemon-side in the same literal
        // `send-keys -H` form the mouse-report fix uses — the caller
        // (Cmd+V, option-click, middle-click) has already sanitized the
        // text, and hex form cannot mangle multi-line or special content.
        #[cfg(feature = "mux")]
        if let Some(transport) = &self.tmux_state.transport
            && let Some(pane) = self.focused_mux_pane_from_native()
        {
            return self.paste_via_mux_pane(&**transport, pane, text);
        }

        if !self.config.load().tmux.tmux_enabled || !self.is_tmux_connected() {
            return false;
        }

        let session = match &self.tmux_state.tmux_session {
            Some(s) => s,
            None => return false,
        };

        // Format the literal send command
        let cmd = match session.format_send_literal(text) {
            Some(c) => c,
            None => return false,
        };

        // Write to gateway tab
        if self.write_to_gateway(&cmd) {
            crate::debug_info!("TMUX", "Pasted {} chars via gateway", text.len());
            return true;
        }

        false
    }

    /// Paste into a daemon pane with the same semantics the local PTY
    /// paths apply: the focused mirror's mode-2004 state (tracked from
    /// the daemon pane's own output) decides the bracketed wrapping,
    /// newlines become carriage returns, and a multi-line paste under a
    /// configured `paste_delay_ms` is fed line-by-line — one chunk per
    /// poll tick — instead of one burst (card 01a0d9b551c57243b9888e1ac3be6b55).
    #[cfg(feature = "mux")]
    fn paste_via_mux_pane(
        &self,
        transport: &dyn super::tmux_state::TmuxTransport,
        pane: crate::tmux::TmuxPaneId,
        text: &str,
    ) -> bool {
        let text = text.replace('\n', "\r");
        if text.is_empty() {
            return true; // consumed; matches the local paste no-op
        }
        let (start, end) = self.focused_mirror_bracketed_sequences();
        let delay_ms = self.config.load().selection.paste_delay_ms;

        if delay_ms == 0 || !text.contains('\r') {
            // One burst: start + content + end, the wire equivalent of the
            // three writes `TerminalManager::paste` makes.
            let mut bytes = start;
            bytes.extend_from_slice(text.as_bytes());
            bytes.extend_from_slice(&end);
            if !bytes.is_empty() {
                super::notifications::mux::route_literal_bytes(transport, Some(pane), &bytes);
            }
            return true;
        }

        // Line-by-line under the delay, mirroring `paste_with_delay`: the
        // start sequence leads the first line, every non-final line carries
        // its own `\r`, and the end sequence trails the last line. The
        // lead chunk goes out now so the first line shows immediately.
        let lines: Vec<&str> = text.split('\r').collect();
        let mut lead = start;
        lead.extend_from_slice(lines[0].as_bytes());
        lead.push(b'\r');
        if !lead.is_empty() {
            super::notifications::mux::route_literal_bytes(transport, Some(pane), &lead);
        }
        let mut chunks: std::collections::VecDeque<Vec<u8>> = std::collections::VecDeque::new();
        for (i, line) in lines.iter().enumerate().skip(1) {
            let mut chunk = Vec::from(line.as_bytes());
            if i < lines.len() - 1 {
                chunk.push(b'\r');
            } else {
                chunk.extend_from_slice(&end);
            }
            if !chunk.is_empty() {
                chunks.push_back(chunk);
            }
        }
        if chunks.is_empty() {
            return true;
        }
        let delay = std::time::Duration::from_millis(delay_ms);
        let mut pending = self.tmux_state.pending_mux_paste.borrow_mut();
        if pending.is_some() {
            crate::debug_info!("MUX", "delayed paste replaced an unfinished one");
        }
        *pending = Some(super::tmux_state::PendingMuxPaste {
            pane,
            chunks,
            delay,
            next_due: std::time::Instant::now() + delay,
        });
        drop(pending);
        // The event loop sleeps when idle (`ControlFlow::Wait`); wake it so
        // the first delayed chunk is not parked until an unrelated event.
        self.request_redraw();
        true
    }

    /// The focused mux pane's mirror-terminal bracketed-paste sequences —
    /// empty (paste unwrapped) when the pane cannot be resolved or its
    /// terminal lock is contended, rather than blocking the event loop.
    #[cfg(feature = "mux")]
    fn focused_mirror_bracketed_sequences(&self) -> (Vec<u8>, Vec<u8>) {
        let Some(tab) = self.tab_manager.active_tab() else {
            return (Vec::new(), Vec::new());
        };
        let Some(pane) = tab.pane_manager().and_then(|pm| pm.focused_pane()) else {
            return (Vec::new(), Vec::new());
        };
        match pane.terminal.try_read() {
            Ok(term) => term.bracketed_paste_sequences(),
            Err(_) => (Vec::new(), Vec::new()),
        }
    }

    /// Send the due chunk of a delayed mux paste, if any. Returns whether
    /// a chunk went out (a visual change — the daemon echoes it). A queue
    /// whose daemon died is dropped with the transport.
    #[cfg(feature = "mux")]
    pub(crate) fn tick_pending_mux_paste(&mut self) -> bool {
        let Some(transport) = self.tmux_state.transport.as_deref() else {
            self.tmux_state.pending_mux_paste.borrow_mut().take();
            return false;
        };
        let (pane, chunk, delay, drained) = {
            let mut pending = self.tmux_state.pending_mux_paste.borrow_mut();
            let Some(p) = pending.as_mut() else {
                return false;
            };
            if std::time::Instant::now() < p.next_due {
                return false;
            }
            match p.chunks.pop_front() {
                Some(chunk) => (p.pane, chunk, p.delay, p.chunks.is_empty()),
                None => {
                    pending.take();
                    return false;
                }
            }
        };
        super::notifications::mux::route_literal_bytes(transport, Some(pane), &chunk);
        if drained {
            self.tmux_state.pending_mux_paste.borrow_mut().take();
        } else if let Some(p) = self.tmux_state.pending_mux_paste.borrow_mut().as_mut() {
            p.next_due = std::time::Instant::now() + delay;
        }
        // More chunks remain: re-arm the sleeping event loop for the tick
        // that sends the next one.
        if self.tmux_state.pending_mux_paste.borrow().is_some() {
            self.request_redraw();
        }
        true
    }

    /// Handle tmux prefix key mode
    ///
    /// In control mode, we intercept the prefix key (e.g., Ctrl+B or Ctrl+Space)
    /// and wait for the next key to translate into a tmux command.
    ///
    /// Returns true if the key was handled by the prefix system.
    pub fn handle_tmux_prefix_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        // Only handle on key press
        if event.state != winit::event::ElementState::Pressed {
            return false;
        }

        // Only handle if tmux is connected
        if !self.config.load().tmux.tmux_enabled || !self.is_tmux_connected() {
            return false;
        }

        let modifiers = self.input_handler.modifiers.state();

        // Check if we're in prefix mode (waiting for command key)
        if self.tmux_state.tmux_prefix_state.is_active() {
            // Ignore modifier-only key presses (Shift, Ctrl, Alt, Super)
            // These are needed to type shifted characters like " and %
            use winit::keyboard::{Key, NamedKey};
            let is_modifier_only = matches!(
                event.logical_key,
                Key::Named(
                    NamedKey::Shift
                        | NamedKey::Control
                        | NamedKey::Alt
                        | NamedKey::Super
                        | NamedKey::Meta
                )
            );
            if is_modifier_only {
                crate::debug_trace!(
                    "TMUX",
                    "Ignoring modifier-only key in prefix mode: {:?}",
                    event.logical_key
                );
                return false; // Don't consume - let the modifier key through
            }

            // Exit prefix mode
            self.tmux_state.tmux_prefix_state.exit();

            // Get focused pane ID for targeted commands
            let focused_pane = self
                .tmux_state
                .tmux_session
                .as_ref()
                .and_then(|s| s.focused_pane());

            // Translate the command key to a tmux command
            if let Some(cmd) =
                crate::tmux::translate_command_key(&event.logical_key, modifiers, focused_pane)
            {
                crate::debug_info!(
                    "TMUX",
                    "Prefix command: {:?} -> {}",
                    event.logical_key,
                    cmd.trim()
                );

                // Send the command to tmux
                if self.write_to_gateway(&cmd) {
                    // Show toast for certain commands (check command base, ignoring target)
                    let cmd_base = cmd.split(" -t").next().unwrap_or(&cmd).trim();
                    match cmd_base {
                        "detach-client" => self.show_toast("tmux: Detaching..."),
                        "new-window" => self.show_toast("tmux: New window"),
                        _ => {}
                    }
                    return true;
                }
            } else {
                // Unknown command key - show feedback
                crate::debug_info!(
                    "TMUX",
                    "Unknown prefix command key: {:?}",
                    event.logical_key
                );
                self.show_toast(format!(
                    "tmux: Unknown command key: {:?}",
                    event.logical_key
                ));
            }
            return true; // Consumed the key even if unknown
        }

        // Check if this is the prefix key
        if let Some(ref prefix_key) = self.tmux_state.tmux_prefix_key
            && prefix_key.matches(&event.logical_key, modifiers)
        {
            crate::debug_info!("TMUX", "Prefix key pressed, entering prefix mode");
            self.tmux_state.tmux_prefix_state.enter();
            self.show_toast("tmux: prefix...");
            return true;
        }

        false
    }
}

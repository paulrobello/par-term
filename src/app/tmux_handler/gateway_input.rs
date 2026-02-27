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
        // Check if tmux is enabled and connected
        if !self.config.tmux_enabled || !self.is_tmux_connected() {
            crate::debug_trace!(
                "TMUX",
                "send_input_via_tmux: not sending - enabled={}, connected={}",
                self.config.tmux_enabled,
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

    /// Format send-keys command for a specific window (if mapping exists)
    fn format_send_keys_for_window(&self, data: &[u8]) -> Option<String> {
        let active_tab_id = self.tab_manager.active_tab_id()?;

        // Find the tmux window for this tab
        let tmux_window_id = self.tmux_state.tmux_sync.get_window(active_tab_id)?;

        // Format send-keys command with window target using proper escaping
        let escaped = crate::tmux::escape_keys_for_tmux(data);
        Some(format!("send-keys -t @{} {}\n", tmux_window_id, escaped))
    }

    /// Send input via tmux window target (fallback when no pane ID is set)
    #[allow(dead_code)] // Planned for TmuxSync integration
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
        if !self.config.tmux_enabled || !self.is_tmux_connected() {
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
        if !self.config.tmux_enabled || !self.is_tmux_connected() {
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

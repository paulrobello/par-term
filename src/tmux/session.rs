//! tmux session management
//!
//! This module handles the lifecycle of tmux control mode sessions.
//!
//! ## Gateway Mode
//!
//! Gateway mode writes `tmux -CC` commands to the existing terminal's PTY
//! instead of spawning a separate process. This is the iTerm2 approach and
//! is more reliable because tmux control mode requires a real PTY.
//!
//! The flow is:
//! 1. Write `tmux -CC new-session` or `tmux -CC attach` to the PTY
//! 2. Enable tmux control mode parsing in the terminal
//! 3. Receive notifications via the terminal's parser
//! 4. Route input via `send-keys` commands written to the same PTY

use super::types::{TmuxPaneId, TmuxSessionInfo, TmuxWindow, TmuxWindowId};
use std::collections::HashMap;

/// State of a tmux control mode session
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Not connected to any session
    Disconnected,
    /// Connecting to a session
    Connecting,
    /// Connected and active
    Connected,
    /// Session ended or lost connection
    Ended,
}

/// Gateway mode state machine
///
/// Tracks the state of a gateway-mode tmux connection where commands
/// are written to the existing terminal's PTY.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayState {
    /// Not in gateway mode
    Inactive,
    /// Command has been written, waiting for control mode to start
    Initiating,
    /// Received %begin, detecting session info
    Detecting,
    /// Fully connected and receiving notifications
    Connected,
    /// Gateway mode ended (exit or error)
    Ended,
}

/// A tmux control mode session
pub struct TmuxSession {
    /// Current session state (used by both gateway and legacy modes)
    state: SessionState,
    /// Gateway-specific state
    gateway_state: GatewayState,
    /// Session info (if connected)
    info: Option<TmuxSessionInfo>,
    /// Windows in this session
    windows: HashMap<TmuxWindowId, TmuxWindow>,
    /// Active window ID
    active_window: Option<TmuxWindowId>,
    /// Session name (for display and commands)
    session_name: Option<String>,
    /// Focused pane ID (for send-keys targeting)
    focused_pane: Option<TmuxPaneId>,
}

/// Notifications received from tmux control mode
#[derive(Debug, Clone)]
pub enum TmuxNotification {
    /// Control mode has started (%begin received)
    ControlModeStarted,
    /// Session has started
    SessionStarted(String),
    /// Session was renamed
    SessionRenamed(String),
    /// A window was added
    WindowAdd(TmuxWindowId),
    /// A window was closed
    WindowClose(TmuxWindowId),
    /// Window was renamed
    WindowRenamed { id: TmuxWindowId, name: String },
    /// Layout changed
    LayoutChange {
        window_id: TmuxWindowId,
        layout: String,
    },
    /// Pane output received
    Output { pane_id: TmuxPaneId, data: Vec<u8> },
    /// Pane focus changed (user selected different pane in external tmux)
    PaneFocusChanged { pane_id: TmuxPaneId },
    /// Session ended
    SessionEnded,
    /// Error occurred
    Error(String),
    /// Paused notification (for slow connections)
    Pause,
    /// Continue notification (resume after pause)
    Continue,
}

impl TmuxSession {
    /// Create a new disconnected session
    pub fn new() -> Self {
        Self {
            state: SessionState::Disconnected,
            gateway_state: GatewayState::Inactive,
            info: None,
            windows: HashMap::new(),
            active_window: None,
            session_name: None,
            focused_pane: None,
        }
    }

    /// Get the current session state
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Get the gateway state
    pub fn gateway_state(&self) -> GatewayState {
        self.gateway_state
    }

    /// Check if gateway mode is active
    pub fn is_gateway_active(&self) -> bool {
        matches!(
            self.gateway_state,
            GatewayState::Initiating | GatewayState::Detecting | GatewayState::Connected
        )
    }

    /// Get session info if connected
    pub fn info(&self) -> Option<&TmuxSessionInfo> {
        self.info.as_ref()
    }

    /// Get session name
    pub fn session_name(&self) -> Option<&str> {
        self.session_name.as_deref()
    }

    /// Get all windows
    pub fn windows(&self) -> &HashMap<TmuxWindowId, TmuxWindow> {
        &self.windows
    }

    /// Get a window by ID
    pub fn window(&self, id: TmuxWindowId) -> Option<&TmuxWindow> {
        self.windows.get(&id)
    }

    /// Get the active window
    pub fn active_window(&self) -> Option<&TmuxWindow> {
        self.active_window.and_then(|id| self.windows.get(&id))
    }

    /// Get the focused pane ID
    pub fn focused_pane(&self) -> Option<TmuxPaneId> {
        self.focused_pane
    }

    /// Set the focused pane ID
    pub fn set_focused_pane(&mut self, pane_id: Option<TmuxPaneId>) {
        self.focused_pane = pane_id;
    }

    // =========================================================================
    // Gateway Mode Methods
    // =========================================================================

    /// Generate the command to initiate a new tmux session in gateway mode.
    ///
    /// This returns the command string that should be written to the terminal's PTY.
    /// After writing this, call `set_gateway_initiating()` to update state.
    ///
    /// Note: Uses `\n` (newline) to execute the command immediately.
    pub fn create_new_command(session_name: Option<&str>) -> String {
        match session_name {
            Some(name) => format!(
                "tmux -CC new-session -s '{}'\n",
                name.replace('\'', "'\\''")
            ),
            None => "tmux -CC new-session\n".to_string(),
        }
    }

    /// Generate the command to attach to an existing tmux session in gateway mode.
    ///
    /// This returns the command string that should be written to the terminal's PTY.
    /// After writing this, call `set_gateway_initiating()` to update state.
    ///
    /// Note: Uses `\n` (newline) to execute the command immediately.
    pub fn create_attach_command(session_name: &str) -> String {
        format!(
            "tmux -CC attach -t '{}'\n",
            session_name.replace('\'', "'\\''")
        )
    }

    /// Generate a command that creates a new session or attaches if it exists.
    ///
    /// This is useful for the session picker where the user provides a name
    /// and we want to either create or attach.
    ///
    /// Note: Uses `\n` (newline) to execute the command immediately.
    pub fn create_or_attach_command(session_name: &str) -> String {
        let escaped = session_name.replace('\'', "'\\''");
        format!("tmux -CC new-session -A -s '{}'\n", escaped)
    }

    /// Set gateway state to initiating (command written, waiting for response)
    pub fn set_gateway_initiating(&mut self) {
        self.gateway_state = GatewayState::Initiating;
        self.state = SessionState::Connecting;
    }

    /// Set gateway state to detecting (received %begin)
    pub fn set_gateway_detecting(&mut self) {
        self.gateway_state = GatewayState::Detecting;
    }

    /// Set gateway state to connected (received %session-changed)
    pub fn set_gateway_connected(&mut self, session_name: String) {
        self.gateway_state = GatewayState::Connected;
        self.state = SessionState::Connected;
        self.session_name = Some(session_name);
    }

    /// Set gateway state to ended
    pub fn set_gateway_ended(&mut self) {
        self.gateway_state = GatewayState::Ended;
        self.state = SessionState::Ended;
    }

    /// Reset gateway mode state (disconnect from gateway)
    pub fn reset_gateway(&mut self) {
        self.gateway_state = GatewayState::Inactive;
        self.state = SessionState::Disconnected;
        self.session_name = None;
        self.focused_pane = None;
        self.windows.clear();
        self.active_window = None;
        self.info = None;
    }

    /// Process a notification in gateway mode and update state accordingly.
    ///
    /// Returns true if the notification caused a state transition.
    pub fn process_gateway_notification(&mut self, notification: &TmuxNotification) -> bool {
        match notification {
            TmuxNotification::ControlModeStarted => {
                // Received %begin - transition from Initiating to Detecting
                if self.gateway_state == GatewayState::Initiating {
                    crate::debug_info!(
                        "TMUX",
                        "Control mode started (%begin), transitioning to Detecting"
                    );
                    self.set_gateway_detecting();
                    return true;
                }
            }
            TmuxNotification::SessionStarted(name) => {
                // Transition from Initiating/Detecting -> Connected
                if matches!(
                    self.gateway_state,
                    GatewayState::Initiating | GatewayState::Detecting
                ) {
                    crate::debug_info!(
                        "TMUX",
                        "Session started, transitioning to Connected: {}",
                        name
                    );
                    self.set_gateway_connected(name.clone());
                    return true;
                }
            }
            TmuxNotification::SessionEnded => {
                // Only treat as session end if we were actually connected
                if self.gateway_state == GatewayState::Connected {
                    crate::debug_info!("TMUX", "Session ended while connected");
                    self.set_gateway_ended();
                    return true;
                } else if self.gateway_state == GatewayState::Detecting {
                    // Exit during detection - tmux started but session failed
                    crate::debug_error!(
                        "TMUX",
                        "Session exit during detection - session creation failed"
                    );
                    self.set_gateway_ended();
                    return true;
                } else if self.gateway_state == GatewayState::Initiating {
                    // Exit before %begin received - this is unusual but handle it
                    crate::debug_error!(
                        "TMUX",
                        "Session exit before control mode started - tmux failed to start"
                    );
                    self.set_gateway_ended();
                    return true;
                }
            }
            TmuxNotification::Error(msg) => {
                crate::debug_error!("TMUX", "Gateway error: {}", msg);
                // Only treat errors as fatal during early initiation (before %begin)
                if self.gateway_state == GatewayState::Initiating {
                    crate::debug_error!("TMUX", "Error during initiation - connection failed");
                    self.set_gateway_ended();
                    return true;
                }
                // During Detecting or Connected state, log the error but don't disconnect
                // tmux may send error notifications for non-fatal issues
            }
            _ => {}
        }
        false
    }

    /// Format input for sending via tmux send-keys command.
    ///
    /// When in gateway mode, keyboard input needs to be sent to tmux
    /// using the send-keys command rather than directly to the PTY.
    ///
    /// Returns the command string to write to the PTY, or None if not in gateway mode.
    pub fn format_send_keys(&self, data: &[u8]) -> Option<String> {
        if !self.is_gateway_active() || self.state != SessionState::Connected {
            return None;
        }

        let pane_id = self.focused_pane?;
        let escaped = escape_keys_for_tmux(data);
        Some(format!("send-keys -t %{} {}\n", pane_id, escaped))
    }

    /// Format a literal paste for sending via tmux.
    ///
    /// Uses send-keys -l for literal text handling.
    pub fn format_send_literal(&self, text: &str) -> Option<String> {
        if !self.is_gateway_active() || self.state != SessionState::Connected {
            return None;
        }

        let pane_id = self.focused_pane?;
        let escaped = text.replace('\'', "'\\''");
        Some(format!("send-keys -t %{} -l '{}'\n", pane_id, escaped))
    }

    /// Disconnect from the session
    pub fn disconnect(&mut self) {
        self.reset_gateway();
    }

    // =========================================================================
    // Window/Pane State Management
    // =========================================================================

    /// Update a window in the session
    pub fn update_window(&mut self, window: TmuxWindow) {
        let id = window.id;
        if window.active {
            self.active_window = Some(id);
        }
        self.windows.insert(id, window);
    }

    /// Remove a window from the session
    pub fn remove_window(&mut self, id: TmuxWindowId) {
        self.windows.remove(&id);
        if self.active_window == Some(id) {
            self.active_window = self.windows.keys().next().copied();
        }
    }

    /// Set session info
    pub fn set_info(&mut self, info: TmuxSessionInfo) {
        self.info = Some(info);
    }
}

impl Default for TmuxSession {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TmuxSession {
    fn drop(&mut self) {
        self.disconnect();
    }
}

/// Escape a byte sequence for tmux send-keys command.
///
/// This handles special characters that need escaping for tmux.
pub fn escape_keys_for_tmux(data: &[u8]) -> String {
    // For simple ASCII, we can use literal strings with proper escaping
    // For complex sequences (control chars, etc.), use hex keys

    let mut result = String::new();
    let mut in_literal = false;

    for &byte in data {
        match byte {
            // Control characters need to be sent as special keys
            0x00 => {
                close_literal(&mut result, &mut in_literal);
                result.push_str("C-Space ");
            }
            0x01..=0x1a => {
                close_literal(&mut result, &mut in_literal);
                // Ctrl+A through Ctrl+Z
                result.push_str(&format!("C-{} ", (b'a' + byte - 1) as char));
            }
            0x1b => {
                close_literal(&mut result, &mut in_literal);
                result.push_str("Escape ");
            }
            0x7f => {
                close_literal(&mut result, &mut in_literal);
                result.push_str("BSpace ");
            }
            // Special characters that need quoting
            b'\'' => {
                if !in_literal {
                    result.push('\'');
                    in_literal = true;
                }
                result.push_str("'\\''");
            }
            b' ' => {
                close_literal(&mut result, &mut in_literal);
                result.push_str("Space ");
            }
            // Printable ASCII can be sent literally (0x21..=0x7e, excluding space 0x20)
            0x21..=0x7e => {
                if !in_literal {
                    result.push('\'');
                    in_literal = true;
                }
                result.push(byte as char);
            }
            // High bytes (UTF-8 continuation, etc.) - send as hex
            _ => {
                close_literal(&mut result, &mut in_literal);
                result.push_str(&format!("0x{:02x} ", byte));
            }
        }
    }

    close_literal(&mut result, &mut in_literal);
    result.trim().to_string()
}

fn close_literal(result: &mut String, in_literal: &mut bool) {
    if *in_literal {
        result.push_str("' ");
        *in_literal = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_new_command() {
        let cmd = TmuxSession::create_new_command(None);
        assert_eq!(cmd, "tmux -CC new-session\n");

        let cmd = TmuxSession::create_new_command(Some("test"));
        assert_eq!(cmd, "tmux -CC new-session -s 'test'\n");
    }

    #[test]
    fn test_create_attach_command() {
        let cmd = TmuxSession::create_attach_command("mysession");
        assert_eq!(cmd, "tmux -CC attach -t 'mysession'\n");
    }

    #[test]
    fn test_create_or_attach_command() {
        let cmd = TmuxSession::create_or_attach_command("dev");
        assert_eq!(cmd, "tmux -CC new-session -A -s 'dev'\n");
    }

    #[test]
    fn test_gateway_state_transitions() {
        let mut session = TmuxSession::new();
        assert_eq!(session.gateway_state(), GatewayState::Inactive);
        assert!(!session.is_gateway_active());

        session.set_gateway_initiating();
        assert_eq!(session.gateway_state(), GatewayState::Initiating);
        assert!(session.is_gateway_active());
        assert_eq!(session.state(), SessionState::Connecting);

        session.set_gateway_connected("test".to_string());
        assert_eq!(session.gateway_state(), GatewayState::Connected);
        assert!(session.is_gateway_active());
        assert_eq!(session.state(), SessionState::Connected);
        assert_eq!(session.session_name(), Some("test"));

        session.set_gateway_ended();
        assert_eq!(session.gateway_state(), GatewayState::Ended);
        assert!(!session.is_gateway_active());
        assert_eq!(session.state(), SessionState::Ended);
    }

    #[test]
    fn test_escape_keys_simple() {
        let escaped = escape_keys_for_tmux(b"hello");
        assert_eq!(escaped, "'hello'");
    }

    #[test]
    fn test_escape_keys_with_space() {
        let escaped = escape_keys_for_tmux(b"hello world");
        assert!(escaped.contains("Space"));
    }

    #[test]
    fn test_escape_keys_ctrl_c() {
        let escaped = escape_keys_for_tmux(&[0x03]);
        assert_eq!(escaped, "C-c");
    }

    #[test]
    fn test_escape_keys_escape() {
        let escaped = escape_keys_for_tmux(&[0x1b]);
        assert_eq!(escaped, "Escape");
    }
}

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

use crate::types::{TmuxPaneId, TmuxSessionInfo, TmuxWindow, TmuxWindowId};
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
        /// The pane the window is zoomed to (`resize-pane -Z`), from the
        /// `Z` window flag and the single-pane visible layout. `layout` is
        /// always the true tree, so unzoom restores it exactly.
        zoomed: Option<TmuxPaneId>,
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
            TmuxNotification::ControlModeStarted
                // Received %begin - transition from Initiating to Detecting
                if self.gateway_state == GatewayState::Initiating => {
                    log::info!("[TMUX] Control mode started (%begin), transitioning to Detecting");
                    self.set_gateway_detecting();
                    return true;
                }
            TmuxNotification::SessionStarted(name) => {
                // Transition from Initiating/Detecting -> Connected
                if matches!(
                    self.gateway_state,
                    GatewayState::Initiating | GatewayState::Detecting
                ) {
                    log::info!(
                        "[TMUX] Session started, transitioning to Connected: {}",
                        name
                    );
                    self.set_gateway_connected(name.clone());
                    return true;
                }
            }
            TmuxNotification::SessionEnded => {
                // Only treat as session end if we were actually connected
                if self.gateway_state == GatewayState::Connected {
                    log::info!("[TMUX] Session ended while connected");
                    self.set_gateway_ended();
                    return true;
                } else if self.gateway_state == GatewayState::Detecting {
                    // Exit during detection - tmux started but session failed
                    log::error!("[TMUX] Session exit during detection - session creation failed");
                    self.set_gateway_ended();
                    return true;
                } else if self.gateway_state == GatewayState::Initiating {
                    // Exit before %begin received - this is unusual but handle it
                    log::error!(
                        "[TMUX] Session exit before control mode started - tmux failed to start"
                    );
                    self.set_gateway_ended();
                    return true;
                }
            }
            TmuxNotification::Error(msg) => {
                log::error!("[TMUX] Gateway error: {}", msg);
                // Only treat errors as fatal during early initiation (before %begin)
                if self.gateway_state == GatewayState::Initiating {
                    log::error!("[TMUX] Error during initiation - connection failed");
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
        Some(format!(
            "send-keys -t %{} {}\n",
            pane_id,
            send_keys_arguments(data)
        ))
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

    /// Format input as hex-encoded bytes for tmux send-keys -H.
    ///
    /// Uses the `-H` flag so tmux injects each byte as a `KEYC_LITERAL` key,
    /// bypassing tmux's key-name interpretation and encoding. The bytes are
    /// written directly to the pane's PTY as raw bytes.
    ///
    /// Used for sending CSI-u extended key sequences (e.g., `\x1b[13;2u` for
    /// Shift+Enter) that need to pass through tmux without re-encoding.
    pub fn format_send_hex_keys(&self, data: &[u8]) -> Option<String> {
        if !self.is_gateway_active() || self.state != SessionState::Connected {
            return None;
        }

        let pane_id = self.focused_pane?;
        let hex_keys: Vec<String> = data.iter().map(|b| format!("{:02x}", b)).collect();
        Some(format!(
            "send-keys -t %{} -H {}\n",
            pane_id,
            hex_keys.join(" ")
        ))
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

/// Format a `set-buffer` command line for the clipboard sync.
///
/// Both consumers parse the payload with POSIX single-quoting, so embedded
/// quotes use the close-escape-reopen idiom and everything else survives
/// verbatim inside the quotes — except newlines: the control wire is
/// line-delimited, so content containing one rides the hex form instead
/// (the same `send-keys -H` idiom the daemon already speaks).
pub fn set_buffer_command(content: &str) -> String {
    if content.contains('\n') || content.contains('\r') {
        let hex = content
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        return format!("set-buffer -H {hex}");
    }
    let escaped = content.replace('\'', "'\\''");
    format!("set-buffer '{}'", escaped)
}

/// Escape a byte sequence for tmux send-keys command.
///
/// This handles special characters that need escaping for tmux: control
/// bytes become key names (`C-c`, `Escape`, `BSpace`), space becomes the
/// `Space` key, high bytes become `0xNN` tokens, and printable runs are
/// quoted literals.
///
/// Quoting alone does not literalize: tmux and the par-mux daemon strip
/// quotes and then resolve key names, so a quoted run that spells a key
/// name or `0xNN` byte — typed text like "C-c" or "Enter" — would be
/// pressed as a key. [`flush_literal_run`] splits such runs into adjacent
/// quoted chunks no receiver resolves as a key. Send-keys runs should
/// format their arguments through [`send_keys_arguments`].
pub fn escape_keys_for_tmux(data: &[u8]) -> String {
    let mut result = String::new();
    // The current printable run's bytes, empty when no run is open.
    let mut run: Vec<u8> = Vec::new();

    for &byte in data {
        // Printable ASCII (the single quote included — flush re-quotes it
        // with the `'\''` idiom) buffers into a literal run.
        if (0x21..=0x7e).contains(&byte) {
            run.push(byte);
            continue;
        }
        flush_literal_run(&mut result, &mut run);
        match byte {
            // Control characters need to be sent as special keys
            0x00 => result.push_str("C-Space "),
            0x01..=0x1a => {
                // Ctrl+A through Ctrl+Z
                result.push_str(&format!("C-{} ", (b'a' + byte - 1) as char));
            }
            0x1b => result.push_str("Escape "),
            0x7f => result.push_str("BSpace "),
            b' ' => result.push_str("Space "),
            // High bytes (UTF-8 continuation, etc.) - send as hex
            _ => result.push_str(&format!("0x{:02x} ", byte)),
        }
    }
    flush_literal_run(&mut result, &mut run);
    result.trim().to_string()
}

/// Emit one printable run as quoted literal token(s), splitting it while
/// the whole remainder would resolve as a key press or `0xNN` byte.
///
/// tmux and the par-mux daemon resolve key names AFTER stripping quotes,
/// so quoting cannot protect a run that spells one. Tokens join with
/// nothing between them, so adjacent chunks deliver the run's bytes
/// verbatim. The split table mirrors the pinned core's `key_part` +
/// `hex_byte_token`; a chunk a future key table resolved would be
/// pressed, not typed. Single bytes are never resolvable, so the loop
/// always leaves a non-empty literal remainder.
fn flush_literal_run(result: &mut String, run: &mut Vec<u8>) {
    if run.is_empty() {
        return;
    }
    let mut rest: &[u8] = run;
    while is_resolvable_key_token(rest) {
        push_quoted_chunk(result, &rest[..1]);
        rest = &rest[1..];
    }
    push_quoted_chunk(result, rest);
    run.clear();
}

fn push_quoted_chunk(result: &mut String, chunk: &[u8]) {
    result.push('\'');
    for &b in chunk {
        if b == b'\'' {
            result.push_str("'\\''");
        } else {
            result.push(b as char);
        }
    }
    result.push_str("' ");
}

/// The key-name tokens a send-keys receiver resolves in default mode — the
/// mirror of the pinned core's `key_part` table (also real tmux's
/// `key-string.c` names).
const RESOLVABLE_KEY_NAMES: &[&str] = &[
    "C-Space", "Enter", "Tab", "Escape", "Esc", "BSpace", "Space", "Up", "Down", "Right", "Left",
    "Home", "End", "PageUp", "PgUp", "PPage", "PageDown", "PgDn", "NPage", "IC", "Insert", "DC",
    "Delete", "BTab", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
];

/// True when a send-keys receiver (pinned core `key_part` +
/// [`RESOLVABLE_KEY_NAMES`] + `hex_byte_token`) resolves this exact token
/// as a key press or raw byte rather than literal text.
fn is_resolvable_key_token(token: &[u8]) -> bool {
    let Some(text) = std::str::from_utf8(token).ok() else {
        return false;
    };
    if RESOLVABLE_KEY_NAMES.contains(&text) {
        return true;
    }
    if let Some(letter) = text.strip_prefix("C-") {
        let bytes = letter.as_bytes();
        return bytes.len() == 1 && bytes[0].is_ascii_alphabetic();
    }
    match text.strip_prefix("0x") {
        Some(digits) => digits.len() == 2 && digits.bytes().all(|b| b.is_ascii_hexdigit()),
        None => false,
    }
}

/// Format the full argument list of a `send-keys` run from raw payload
/// bytes: the [`escape_keys_for_tmux`] tokens, with literal runs split so
/// key-name-like text is typed, never pressed.
///
/// Control bytes stay bare key names (`C-j`), space stays the `Space` key,
/// and high bytes stay `0xNN` tokens: the notation exists so the receiving
/// application's input modes decide what each key does (ARC-093
/// mode-following), which no blanket literal flag may suppress. Typed
/// text — printable runs, the spaces between words — is spelled so no
/// token resolves as a key: "say C-c now" arrives as the characters, not
/// a SIGINT.
pub fn send_keys_arguments(data: &[u8]) -> String {
    escape_keys_for_tmux(data)
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

    #[test]
    fn test_send_keys_arguments_key_name_text_splits() {
        // Typed text that spells tmux key names is split into adjacent
        // chunks no receiver resolves as a key — quoting alone cannot
        // protect it (receivers strip quotes, then resolve key names).
        // Tokens join with nothing between them, so the bytes survive.
        assert_eq!(send_keys_arguments(b"Enter"), "'E' 'nter'");
        assert_eq!(send_keys_arguments(b"Space"), "'S' 'pace'");
        assert_eq!(send_keys_arguments(b"C-c"), "'C' '-c'");
        assert_eq!(send_keys_arguments(b"hello"), "'hello'");
        assert_eq!(send_keys_arguments(b"0x41"), "'0' 'x41'");
        assert_eq!(send_keys_arguments(b"F12"), "'F' '12'");
        // Resolvable remainders split too ("BSpace" → B + Space + pace).
        assert_eq!(send_keys_arguments(b"BSpace"), "'B' 'S' 'pace'");
    }

    #[test]
    fn test_send_keys_arguments_mixed_payload_types_key_name_runs() {
        // The card's regression case: a batched write of typed text. The
        // escaper renders C-c as part of a literal run — split so the pane
        // receives the characters, never the SIGINT.
        assert_eq!(
            send_keys_arguments(b"say C-c now"),
            "'say' Space 'C' '-c' Space 'now'"
        );
    }

    #[test]
    fn test_send_keys_arguments_key_notation_stays_bare() {
        // Deliberate key notation keeps the bare key-name form: the
        // receiving app's input modes decide the key's effect (ARC-093
        // mode-following) — a literal flag would suppress it.
        assert_eq!(send_keys_arguments(&[0x03]), "C-c");
        assert_eq!(send_keys_arguments(&[0x1b]), "Escape");
        assert_eq!(send_keys_arguments(&[0x7f]), "BSpace");
        assert_eq!(send_keys_arguments(b"hello world"), "'hello' Space 'world'");
        assert_eq!(send_keys_arguments(&[0x41, 0xc3, 0xa9]), "'A' 0xc3 0xa9");
        assert_eq!(send_keys_arguments(b""), "");
    }

    #[test]
    fn test_send_keys_arguments_mixed_notation_stays_key_names() {
        // Text AND deliberate notation in one payload: the typed runs are
        // literalized, the notation stays bare key names the daemon
        // resolves.
        assert_eq!(send_keys_arguments(b"one\x0atwo"), "'one' C-j 'two'");
        assert_eq!(
            send_keys_arguments(b"say \x03 now"),
            "'say' Space C-c Space 'now'"
        );
    }

    #[test]
    fn test_format_send_keys_types_key_name_text() {
        let mut session = TmuxSession::new();
        session.set_gateway_connected("dev".to_string());
        session.set_focused_pane(Some(5));
        assert_eq!(
            session.format_send_keys(b"Enter"),
            Some("send-keys -t %5 'E' 'nter'\n".to_string())
        );
        assert_eq!(
            session.format_send_keys(&[0x03]),
            Some("send-keys -t %5 C-c\n".to_string())
        );
    }

    #[test]
    fn test_set_buffer_command_quotes_and_newlines() {
        assert_eq!(set_buffer_command("plain"), "set-buffer 'plain'");
        assert_eq!(set_buffer_command("it's"), "set-buffer 'it'\\''s'",);
        // A newline cannot ride the line-delimited wire — hex form.
        assert_eq!(set_buffer_command("a\nb"), "set-buffer -H 61 0a 62",);
        assert_eq!(set_buffer_command("cr\r"), "set-buffer -H 63 72 0d");
    }
}

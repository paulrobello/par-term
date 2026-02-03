//! tmux command builders for control mode
//!
//! This module provides type-safe builders for tmux commands that can be
//! sent through control mode. Commands are formatted as newline-terminated
//! strings.

use super::types::{TmuxPaneId, TmuxWindowId};

/// A tmux command ready to be sent
#[derive(Debug, Clone)]
pub struct TmuxCommand {
    /// The command string (without trailing newline)
    command: String,
}

impl TmuxCommand {
    /// Create a new command from a raw string
    fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
        }
    }

    /// Get the command string with trailing newline for sending
    pub fn as_str(&self) -> &str {
        &self.command
    }

    /// Get the command as bytes for writing to the control mode session
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes = self.command.as_bytes().to_vec();
        bytes.push(b'\n');
        bytes
    }

    // =========================================================================
    // Session Commands
    // =========================================================================

    /// List all sessions
    pub fn list_sessions() -> Self {
        Self::new(
            "list-sessions -F '#{session_id}:#{session_name}:#{session_attached}:#{session_windows}'",
        )
    }

    /// Attach to a session
    pub fn attach_session(session: &str) -> Self {
        Self::new(format!("attach-session -t '{}'", session))
    }

    /// Create a new session
    pub fn new_session(name: Option<&str>) -> Self {
        match name {
            Some(n) => Self::new(format!("new-session -d -s '{}'", n)),
            None => Self::new("new-session -d"),
        }
    }

    /// Kill a session
    pub fn kill_session(session: &str) -> Self {
        Self::new(format!("kill-session -t '{}'", session))
    }

    // =========================================================================
    // Window Commands
    // =========================================================================

    /// List windows in the current session
    pub fn list_windows() -> Self {
        Self::new(
            "list-windows -F '#{window_id}:#{window_name}:#{window_index}:#{window_active}:#{window_layout}'",
        )
    }

    /// Create a new window
    pub fn new_window(name: Option<&str>) -> Self {
        match name {
            Some(n) => Self::new(format!("new-window -n '{}'", n)),
            None => Self::new("new-window"),
        }
    }

    /// Select a window by ID
    pub fn select_window(window_id: TmuxWindowId) -> Self {
        Self::new(format!("select-window -t @{}", window_id))
    }

    /// Kill a window
    pub fn kill_window(window_id: TmuxWindowId) -> Self {
        Self::new(format!("kill-window -t @{}", window_id))
    }

    /// Rename a window
    pub fn rename_window(window_id: TmuxWindowId, name: &str) -> Self {
        Self::new(format!("rename-window -t @{} '{}'", window_id, name))
    }

    // =========================================================================
    // Pane Commands
    // =========================================================================

    /// List panes in the current window
    pub fn list_panes() -> Self {
        Self::new(
            "list-panes -F '#{pane_id}:#{pane_active}:#{pane_width}:#{pane_height}:#{pane_left}:#{pane_top}:#{pane_current_command}:#{pane_title}'",
        )
    }

    /// Split pane vertically (creates side-by-side panes)
    pub fn split_vertical(pane_id: Option<TmuxPaneId>) -> Self {
        match pane_id {
            Some(id) => Self::new(format!("split-window -h -t %{}", id)),
            None => Self::new("split-window -h"),
        }
    }

    /// Split pane horizontally (creates stacked panes)
    pub fn split_horizontal(pane_id: Option<TmuxPaneId>) -> Self {
        match pane_id {
            Some(id) => Self::new(format!("split-window -v -t %{}", id)),
            None => Self::new("split-window -v"),
        }
    }

    /// Select a pane by ID
    pub fn select_pane(pane_id: TmuxPaneId) -> Self {
        Self::new(format!("select-pane -t %{}", pane_id))
    }

    /// Kill a pane
    pub fn kill_pane(pane_id: TmuxPaneId) -> Self {
        Self::new(format!("kill-pane -t %{}", pane_id))
    }

    /// Resize a pane
    pub fn resize_pane(pane_id: TmuxPaneId, width: Option<usize>, height: Option<usize>) -> Self {
        let mut cmd = format!("resize-pane -t %{}", pane_id);
        if let Some(w) = width {
            cmd.push_str(&format!(" -x {}", w));
        }
        if let Some(h) = height {
            cmd.push_str(&format!(" -y {}", h));
        }
        Self::new(cmd)
    }

    // =========================================================================
    // Input/Output Commands
    // =========================================================================

    /// Send keys to a pane
    pub fn send_keys(pane_id: TmuxPaneId, keys: &str) -> Self {
        // Escape single quotes in the keys
        let escaped = keys.replace('\'', "'\\''");
        Self::new(format!("send-keys -t %{} '{}'", pane_id, escaped))
    }

    /// Send literal text to a pane
    pub fn send_literal(pane_id: TmuxPaneId, text: &str) -> Self {
        // Use -l for literal text
        let escaped = text.replace('\'', "'\\''");
        Self::new(format!("send-keys -t %{} -l '{}'", pane_id, escaped))
    }

    /// Send keys to a window (sends to the active pane in that window)
    pub fn send_keys_to_window(window_id: TmuxWindowId, keys: &str) -> Self {
        let escaped = keys.replace('\'', "'\\''");
        Self::new(format!("send-keys -t @{} '{}'", window_id, escaped))
    }

    /// Send literal text to a window (sends to the active pane in that window)
    pub fn send_literal_to_window(window_id: TmuxWindowId, text: &str) -> Self {
        let escaped = text.replace('\'', "'\\''");
        Self::new(format!("send-keys -t @{} -l '{}'", window_id, escaped))
    }

    /// Capture pane contents
    pub fn capture_pane(
        pane_id: TmuxPaneId,
        start_line: Option<i32>,
        end_line: Option<i32>,
    ) -> Self {
        let mut cmd = format!("capture-pane -t %{} -p", pane_id);
        if let Some(start) = start_line {
            cmd.push_str(&format!(" -S {}", start));
        }
        if let Some(end) = end_line {
            cmd.push_str(&format!(" -E {}", end));
        }
        Self::new(cmd)
    }

    // =========================================================================
    // Clipboard Commands
    // =========================================================================

    /// Set the tmux paste buffer
    pub fn set_buffer(content: &str) -> Self {
        let escaped = content.replace('\'', "'\\''");
        Self::new(format!("set-buffer '{}'", escaped))
    }

    /// Get the tmux paste buffer
    pub fn get_buffer() -> Self {
        Self::new("show-buffer")
    }

    // =========================================================================
    // Status Bar Commands
    // =========================================================================

    /// Get the left side of the status bar
    ///
    /// Uses display-message with -p flag to print to stdout.
    /// The format uses tmux's status-left format string.
    pub fn get_status_left() -> Self {
        Self::new("display-message -p '#{status-left}'")
    }

    /// Get the right side of the status bar
    ///
    /// Uses display-message with -p flag to print to stdout.
    /// The format uses tmux's status-right format string.
    pub fn get_status_right() -> Self {
        Self::new("display-message -p '#{status-right}'")
    }

    /// Get the full status bar content (formatted)
    ///
    /// Returns: session_name | window_list | date/time
    /// This provides a simpler status bar that doesn't require parsing tmux formats.
    pub fn get_status_bar() -> Self {
        Self::new(
            "display-message -p '#{session_name} | #(tmux list-windows -F \"##I:##W#{?window_active,*,}\" | tr \"\\n\" \" \") | %H:%M'",
        )
    }

    /// Get status bar with custom format
    ///
    /// Allows specifying a custom format string for the status bar.
    /// Uses tmux format variables like #{session_name}, #{window_index}, etc.
    pub fn get_status_formatted(format: &str) -> Self {
        let escaped = format.replace('\'', "'\\''");
        Self::new(format!("display-message -p '{}'", escaped))
    }

    // =========================================================================
    // Control Mode Specific
    // =========================================================================

    /// Refresh client (request full state update)
    pub fn refresh_client() -> Self {
        Self::new("refresh-client")
    }

    /// Subscribe to notifications
    pub fn subscribe_notifications() -> Self {
        // Control mode automatically receives notifications
        // This is a no-op but included for documentation
        Self::new("refresh-client -S")
    }

    /// Set the control client size
    ///
    /// In control mode, tmux doesn't know the terminal size unless we tell it.
    /// This command sets the size for the control client, which affects pane sizing.
    pub fn set_client_size(cols: usize, rows: usize) -> Self {
        // Note: tmux requires -C XxY format (lowercase x), not comma
        Self::new(format!("refresh-client -C {}x{}", cols, rows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_sessions() {
        let cmd = TmuxCommand::list_sessions();
        assert!(cmd.as_str().starts_with("list-sessions"));
    }

    #[test]
    fn test_split_vertical() {
        let cmd = TmuxCommand::split_vertical(Some(5));
        assert_eq!(cmd.as_str(), "split-window -h -t %5");
    }

    #[test]
    fn test_send_keys_escaping() {
        let cmd = TmuxCommand::send_keys(1, "echo 'hello'");
        assert!(cmd.as_str().contains("echo"));
    }
}

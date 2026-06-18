//! tmux control-mode methods for [`TerminalManager`].
//!
//! Thin delegation layer to the core terminal's tmux control-mode state,
//! which handles DCS passthrough and tmux status-bar notifications.

use super::TerminalManager;

impl TerminalManager {
    /// Enable or disable tmux control-mode passthrough
    pub fn set_tmux_control_mode(&self, enabled: bool) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.write();
        term.set_tmux_control_mode(enabled);
    }

    /// Return `true` if tmux control-mode is currently active
    pub fn is_tmux_control_mode(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.write();
        term.is_tmux_control_mode()
    }

    /// Drain and return all pending tmux notifications, clearing the internal queue
    pub fn drain_tmux_notifications(
        &self,
    ) -> Vec<par_term_emu_core_rust::tmux_control::TmuxNotification> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.write();
        term.drain_tmux_notifications()
    }

    /// Return a snapshot of pending tmux notifications without consuming them
    pub fn tmux_notifications(
        &self,
    ) -> Vec<par_term_emu_core_rust::tmux_control::TmuxNotification> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.write();
        term.tmux_notifications().to_vec()
    }
}

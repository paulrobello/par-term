//! Progress bar methods for [`TerminalManager`].
//!
//! Exposes OSC 9;4 (simple progress bar) and OSC 934 (named progress bars)
//! state queries to the frontend.

use super::TerminalManager;

impl TerminalManager {
    /// Get the simple progress bar state (OSC 9;4)
    pub fn progress_bar(&self) -> par_term_emu_core_rust::terminal::ProgressBar {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        *term.progress_bar()
    }

    /// Get all named progress bars (OSC 934)
    pub fn named_progress_bars(
        &self,
    ) -> std::collections::HashMap<String, par_term_emu_core_rust::terminal::NamedProgressBar> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.named_progress_bars().clone()
    }

    /// Check if any progress bar is currently active
    pub fn has_any_progress(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.has_progress() || !term.named_progress_bars().is_empty()
    }
}

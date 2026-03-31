use super::TerminalManager;
pub use par_term_emu_core_rust::terminal::HyperlinkInfo;

impl TerminalManager {
    /// Get all OSC 8 hyperlinks from the terminal
    pub fn get_all_hyperlinks(&self) -> Vec<HyperlinkInfo> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.get_all_hyperlinks()
    }

    /// Non-blocking variant of [`get_all_hyperlinks`].
    ///
    /// Uses `try_lock()` on the internal mutexes. Returns `None` when either
    /// lock is held (e.g., by the PTY reader), allowing the caller to keep
    /// stale hyperlink data rather than blocking the render loop.
    pub fn try_get_all_hyperlinks(&self) -> Option<Vec<HyperlinkInfo>> {
        let pty = self.pty_session.try_lock()?;
        let terminal = pty.terminal();
        let term = terminal.try_lock()?;
        Some(term.get_all_hyperlinks())
    }

    /// Get the URL for a specific hyperlink ID
    pub fn get_hyperlink_url(&self, hyperlink_id: u32) -> Option<String> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.get_hyperlink_url(hyperlink_id)
    }
}

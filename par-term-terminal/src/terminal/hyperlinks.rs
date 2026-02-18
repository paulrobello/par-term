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

    /// Get the URL for a specific hyperlink ID
    #[allow(dead_code)]
    pub fn get_hyperlink_url(&self, hyperlink_id: u32) -> Option<String> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.get_hyperlink_url(hyperlink_id)
    }
}

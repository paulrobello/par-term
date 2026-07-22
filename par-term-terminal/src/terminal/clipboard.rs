use super::TerminalManager;
pub use par_term_emu_core_rust::terminal::{ClipboardEntry, ClipboardSlot};

impl TerminalManager {
    /// Get the OSC 52 clipboard content most recently set by a program.
    ///
    /// Returns `None` when no program has issued an OSC 52 set. The frontend
    /// polls this each frame to bridge remote clipboard writes (e.g. over SSH)
    /// to the system clipboard.
    pub fn get_clipboard(&self) -> Option<String> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.write();
        term.get_clipboard()
    }

    /// Get clipboard history for a specific slot
    pub fn get_clipboard_history(&self, slot: ClipboardSlot) -> Vec<ClipboardEntry> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.write();
        term.get_clipboard_history(slot)
    }

    /// Get the most recent clipboard entry for a slot
    pub fn get_latest_clipboard(&self, slot: ClipboardSlot) -> Option<ClipboardEntry> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.write();
        term.get_latest_clipboard(slot)
    }

    /// Search clipboard history across all slots or a specific slot
    pub fn search_clipboard_history(
        &self,
        query: &str,
        slot: Option<ClipboardSlot>,
    ) -> Vec<ClipboardEntry> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.write();
        term.search_clipboard_history(query, slot)
    }

    /// Add content to clipboard history
    pub fn add_to_clipboard_history(
        &self,
        slot: ClipboardSlot,
        content: String,
        label: Option<String>,
    ) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.write();
        term.add_to_clipboard_history(slot, content, label);
    }

    /// Clear clipboard history for a specific slot
    pub fn clear_clipboard_history(&self, slot: ClipboardSlot) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.write();
        term.clear_clipboard_history(slot);
    }

    /// Clear all clipboard history
    pub fn clear_all_clipboard_history(&self) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.write();
        term.clear_all_clipboard_history();
    }

    /// Set maximum clipboard sync events retained
    pub fn set_max_clipboard_sync_events(&self, max: usize) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.write();
        term.set_max_clipboard_sync_events(max);
    }

    /// Set maximum bytes cached per clipboard event
    pub fn set_max_clipboard_event_bytes(&self, max_bytes: usize) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.write();
        term.set_max_clipboard_event_bytes(max_bytes);
    }

    /// Set maximum clipboard history entries per slot
    pub fn set_max_clipboard_sync_history(&self, max: usize) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.write();
        term.set_max_clipboard_sync_history(max);
    }
}

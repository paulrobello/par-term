//! Terminal observer management methods for [`TerminalManager`].
//!
//! Observers are notified of terminal events (output, title changes, etc.)
//! via the core `TerminalObserver` trait.

use super::TerminalManager;

impl TerminalManager {
    /// Register a terminal observer and return its unique ID
    pub fn add_observer(
        &self,
        observer: std::sync::Arc<dyn par_term_emu_core_rust::observer::TerminalObserver>,
    ) -> par_term_emu_core_rust::observer::ObserverId {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.add_observer(observer)
    }

    /// Unregister an observer by its ID; returns `true` if it was found and removed
    pub fn remove_observer(&self, id: par_term_emu_core_rust::observer::ObserverId) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.remove_observer(id)
    }
}

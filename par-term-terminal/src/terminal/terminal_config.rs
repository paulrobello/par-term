//! Terminal configuration and recording methods for [`TerminalManager`].
//!
//! Covers answerback string (ENQ response), character width config, Unicode
//! normalization, output callbacks, and session recording (asciicast export).

use super::TerminalManager;

impl TerminalManager {
    /// Set the answerback string returned in response to ENQ (Ctrl-E)
    pub fn set_answerback_string(&self, answerback: Option<String>) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_answerback_string(answerback);
    }

    /// Configure character width calculation (East Asian width policy)
    pub fn set_width_config(&self, config: par_term_emu_core_rust::WidthConfig) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_width_config(config);
    }

    /// Set the Unicode normalization form applied to incoming text
    pub fn set_normalization_form(&self, form: par_term_emu_core_rust::NormalizationForm) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_normalization_form(form);
    }

    /// Register a callback invoked for every chunk of raw PTY output
    pub fn set_output_callback<F>(&self, callback: F)
    where
        F: Fn(&[u8]) + Send + Sync + 'static,
    {
        let mut pty = self.pty_session.lock();
        pty.set_output_callback(std::sync::Arc::new(callback));
    }

    /// Begin a new recording session
    pub fn start_recording(&self, title: Option<String>) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.start_recording(title);
    }

    /// Stop the current recording session and return it
    pub fn stop_recording(&self) -> Option<par_term_emu_core_rust::terminal::RecordingSession> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.stop_recording()
    }

    /// Return `true` if a recording session is currently active
    pub fn is_recording(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.is_recording()
    }

    /// Export a recording session as an asciicast v2 string
    pub fn export_asciicast(
        &self,
        session: &par_term_emu_core_rust::terminal::RecordingSession,
    ) -> String {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.export_asciicast(session)
    }
}

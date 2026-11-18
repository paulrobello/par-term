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
        let mut term = terminal.write();
        term.set_answerback_string(answerback);
    }

    /// Configure character width calculation (East Asian width policy)
    pub fn set_width_config(&self, config: par_term_emu_core_rust::WidthConfig) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.write();
        term.set_width_config(config);
    }

    /// Set the Unicode normalization form applied to incoming text
    pub fn set_normalization_form(&self, form: par_term_emu_core_rust::NormalizationForm) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.write();
        term.set_normalization_form(form);
    }

    /// Set maximum OSC 9/777/99 notifications retained (0 disables buffering)
    pub fn set_max_notifications(&self, max: usize) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.write();
        term.set_max_notifications(max);
    }

    /// Toggle the xterm.js-style reflow resize mode. When on, later resizes
    /// reflow lines with the cursor tracking its row and scrollback lines
    /// return on grow; when off (default), par-term's legacy resize applies.
    pub fn set_xterm_resize(&self, on: bool) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.write();
        term.set_xterm_resize(on);
    }

    /// Change the live scrollback capacity. A smaller cap drops the oldest
    /// lines like a scroll and rebases graphics anchors; a larger cap grows
    /// the buffer in place.
    pub fn set_max_scrollback(&self, max: usize) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.write();
        term.set_max_scrollback(max);
    }

    /// Switch emoji/width counting to per-code-point widths so hosts that
    /// count wcwidth per code point agree with the grid.
    pub fn set_codepoint_widths(&self, on: bool) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.write();
        term.set_codepoint_widths(on);
    }

    /// Set the maximum total OSC data length in bytes (QA-012 memory-exhaustion guard)
    pub fn set_max_osc_data_length(&self, max: usize) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.write();
        term.set_max_osc_data_length(max);
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
        let mut term = terminal.write();
        term.start_recording(title);
    }

    /// Stop the current recording session and return it
    pub fn stop_recording(&self) -> Option<par_term_emu_core_rust::terminal::RecordingSession> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.write();
        term.stop_recording()
    }

    /// Return `true` if a recording session is currently active
    pub fn is_recording(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.write();
        term.is_recording()
    }

    /// Export a recording session as an asciicast v2 string
    pub fn export_asciicast(
        &self,
        session: &par_term_emu_core_rust::terminal::RecordingSession,
    ) -> String {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.write();
        term.export_asciicast(session)
    }
}

#[cfg(test)]
mod optin_tests {
    use super::TerminalManager;

    #[test]
    fn live_scrollback_cap_drops_oldest_lines() {
        let manager = TerminalManager::new_with_scrollback(8, 3, 64).unwrap();
        manager.process_data(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7\r\n8");

        // 8 lines on a 3-row screen leave 5 lines in scrollback.
        assert_eq!(manager.scrollback_len(), 5);

        // Shrinking the cap drops the OLDEST lines (Grid::set_max_scrollback
        // drains from the front), keeping the newest 3.
        manager.set_max_scrollback(3);
        assert!(manager.scrollback_len() <= 3);

        // Growing the cap back must not resurrect the dropped lines: the
        // buffer only grows in place from its current 3 lines.
        manager.set_max_scrollback(64);
        assert!(manager.scrollback_len() <= 3);
    }

    #[test]
    fn xterm_reflow_returns_scrollback_lines_on_grow() {
        let mut manager = TerminalManager::new_with_scrollback(8, 3, 64).unwrap();
        manager.process_data(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7\r\n8");
        assert_eq!(manager.scrollback_len(), 5);

        // Legacy height-only grow pads the screen bottom and never touches
        // scrollback.
        manager.resize(8, 6).expect("legacy resize");
        assert_eq!(manager.scrollback_len(), 5);

        manager.set_xterm_resize(true);
        // The xterm grow fills blank space BELOW the cursor before pulling
        // lines back from scrollback (core grid XtermBuffer::resize only
        // decrements ybase when the cursor is on the buffer's last line), so
        // the blank tail the legacy grow left must shrink away first. After
        // the shrink the cursor sits on the last buffer line and the grow to
        // 9 rows pulls all 5 scrollback lines back onto the screen (observed:
        // scrollback_len() == 0).
        manager.resize(8, 3).expect("reflow shrink");
        manager.resize(8, 9).expect("reflow resize");
        assert!(manager.scrollback_len() < 5);
    }

    #[test]
    fn codepoint_widths_span_the_zwj_family_across_eight_columns() {
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";

        // Occupied extent of row 0: position of the last cell that holds a
        // grapheme or a wide-character spacer, plus one. Blank cells surface
        // a single-space grapheme (`base_char().to_string()` in rendering.rs
        // cell conversion), so content means a non-space grapheme or a spacer.
        fn row0_extent(cells: &[crate::Cell]) -> usize {
            cells[..16]
                .iter()
                .rposition(|c| (!c.grapheme.is_empty() && c.grapheme != " ") || c.wide_char_spacer)
                .map_or(0, |pos| pos + 1)
        }

        // Cluster mode (default): the whole family is one wide cell plus its
        // spacer, two columns.
        let cluster = TerminalManager::new_with_scrollback(16, 3, 16).unwrap();
        cluster.process_data(family.as_bytes());
        let cells = cluster
            .try_get_cells_with_scrollback(0, None, false)
            .unwrap();
        assert_eq!(row0_extent(&cells), 2);

        // Per-code-point widths: derived from the core's
        // `emoji_sequences_keep_their_own_widths_with_codepoint_widths` test
        // (write.rs) — each emoji keeps its own two columns (2 + 0 + 2 for
        // "emoji ZWJ emoji"), the ZWJ attaching to the previous cell as a
        // zero-width combining mark. Each of the family's four emoji
        // therefore takes two columns, so the family spans 8 columns (not 4).
        let per_cp = TerminalManager::new_with_scrollback(16, 3, 16).unwrap();
        per_cp.set_codepoint_widths(true);
        per_cp.process_data(family.as_bytes());
        let cells = per_cp
            .try_get_cells_with_scrollback(0, None, false)
            .unwrap();
        assert_eq!(row0_extent(&cells), 8);
    }
}

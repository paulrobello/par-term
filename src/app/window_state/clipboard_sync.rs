//! OSC 52 clipboard bridge.
//!
//! Programs (locally or over SSH) set the clipboard via the OSC 52 escape
//! sequence. The core terminal stores that payload but does not touch the OS
//! clipboard itself; this module polls it each frame and, when it changes,
//! writes it to the system clipboard via `arboard`. Gated by
//! `config.osc52_clipboard` (default on) so a remote program can reach the
//! local clipboard over a plain terminal/SSH session.

use super::WindowState;

/// The OSC 52 content to bridge for `tab`, read from the terminal the user
/// is actually looking at.
///
/// In a mux tab, `tab.terminal` is the hidden login shell and never sees the
/// daemon pane's output — the OSC 52 payload set by the pane's program is
/// parsed into the focused pane's mirror as daemon output streams through
/// `process_mux_output`. [`Tab::try_with_read_terminal`] resolves to exactly
/// that terminal, and to `tab.terminal` itself in plain tabs.
fn polled_osc52_content(tab: &crate::tab::Tab) -> Option<String> {
    tab.try_with_read_terminal(|term| term.get_clipboard())
        .flatten()
}

impl WindowState {
    /// Bridge OSC 52 clipboard writes to the system clipboard.
    ///
    /// Called each frame after `check_trigger_actions()`. Polls the active
    /// tab's terminal for OSC 52 content set by a program; when it differs
    /// from the last value we applied, pushes it to the OS clipboard.
    ///
    /// try_lock: intentional — clipboard polling runs in about_to_wait (sync
    /// event loop). On a miss the write is simply retried next frame.
    pub(crate) fn check_clipboard_sync(&mut self) {
        if !self.config.load().clipboard.osc52_clipboard {
            return;
        }

        let content = if let Some(tab) = self.tab_manager.active_tab() {
            polled_osc52_content(tab)
        } else {
            return;
        };

        // Only act on a real change. `None` means no OSC 52 content exists —
        // never clear the system clipboard for that, since it would clobber
        // local copies (and image clipboards) on tab switches.
        if let Some(content) = content
            && self.last_osc52_clipboard.as_deref() != Some(content.as_str())
        {
            match self.input_handler.copy_to_clipboard(&content) {
                Ok(()) => {
                    crate::debug_info!(
                        "CLIPBOARD",
                        "OSC 52 synced {} chars to system clipboard",
                        content.len()
                    );
                    self.last_osc52_clipboard = Some(content);
                }
                Err(e) => log::error!("OSC 52 clipboard sync failed: {}", e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::polled_osc52_content;

    /// OSC 52 set-clipboard for `payload` (base64 data, BEL-terminated) —
    /// the wire form a pane program emits and the mirror's parser stores.
    fn osc52_set(payload_b64: &str) -> Vec<u8> {
        format!("\x1b]52;c;{payload_b64}\x07").into_bytes()
    }

    /// A `WindowState` with no window or renderer — the same seam the mux
    /// manners tests use (`notifications/mux.rs::manners_state`).
    fn bare_state() -> crate::app::window_state::WindowState {
        let runtime = std::sync::Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build test runtime"),
        );
        crate::app::window_state::WindowState::new(crate::config::Config::default(), runtime)
    }

    /// In a mux tab, `tab.terminal` is the hidden login shell and the focused
    /// pane's mirror holds the daemon output — the OSC 52 poll must read the
    /// mirror. Both terminals carry DISTINCT OSC 52 payloads here so the
    /// assertion can tell which one the poll resolved to.
    #[test]
    fn osc52_poll_reads_the_focused_mirror_not_the_hidden_shell() {
        let mut ws = bare_state();
        let tab_id = ws
            .tab_manager
            .new_tab(
                &ws.config.load(),
                std::sync::Arc::clone(&ws.runtime),
                false,
                None,
            )
            .expect("local tab");
        ws.tab_manager.switch_to(tab_id);

        let tab = ws.tab_manager.active_tab().expect("active tab");

        // Hidden shell: what the OLD (buggy) poll read.
        tab.terminal
            .try_read()
            .expect("hidden shell lock")
            .process_data(&osc52_set("aGlkZGVuLXNoZWxs")); // "hidden-shell"

        // Focused mirror: a daemon-pane terminal the layout swapped in,
        // sharing nothing with the tab's hidden shell.
        let mirror = par_term_terminal::TerminalManager::new_with_scrollback(80, 24, 100)
            .expect("mirror terminal without a shell");
        mirror.process_data(&osc52_set("bWlycm9yLXBhbmU=")); // "mirror-pane"
        let pane_manager = crate::pane::PaneManager::new_with_existing_terminal(
            std::sync::Arc::new(tokio::sync::RwLock::new(mirror)),
            None,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );

        let tab = ws
            .tab_manager
            .active_tab_mut()
            .expect("active tab for mutation");
        tab.pane_manager = Some(pane_manager);

        let tab = ws.tab_manager.active_tab().expect("active tab");
        assert_eq!(
            polled_osc52_content(tab).as_deref(),
            Some("mirror-pane"),
            "the OSC 52 poll must read the focused daemon-pane mirror, not the hidden shell"
        );
    }

    /// Plain tab (no pane manager): the poll reads the tab terminal itself —
    /// the pre-mux behavior, byte-for-byte.
    #[test]
    fn osc52_poll_reads_the_tab_terminal_in_a_plain_tab() {
        let mut ws = bare_state();
        let tab_id = ws
            .tab_manager
            .new_tab(
                &ws.config.load(),
                std::sync::Arc::clone(&ws.runtime),
                false,
                None,
            )
            .expect("local tab");
        ws.tab_manager.switch_to(tab_id);

        let tab = ws.tab_manager.active_tab().expect("active tab");
        assert_eq!(polled_osc52_content(tab), None);
        tab.terminal
            .try_read()
            .expect("tab terminal lock")
            .process_data(&osc52_set("cGxhaW4=")); // "plain"
        assert_eq!(polled_osc52_content(tab).as_deref(), Some("plain"));
    }
}

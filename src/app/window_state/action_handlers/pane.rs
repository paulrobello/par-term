//! Pane rename action handling.
//!
//! [`WindowState::rename_pane`] is the single write path for user-set pane
//! titles: the pane title-bar rename popup, the `rename_pane` keybinding,
//! and the palette row for that action all land here.

use crate::app::window_state::WindowState;
use crate::pane::PaneId;

/// Quote a pane title for `select-pane -T` on the control-mode wire.
///
/// Same single-quote grammar as `par_term_mux::quote_env_value` (the quote
/// closes, an escaped `'` follows, the quote reopens), restated here so the
/// gateway path keeps working in builds without the `mux` feature.
fn quote_pane_title(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

impl WindowState {
    /// Set (or clear) a pane's user title.
    ///
    /// A blank name reverts the pane to automatic titles, mirroring
    /// `TabBarAction::RenameTab`'s blank branch. When the pane maps to a
    /// daemon pane the name is also pushed with `select-pane -t %N -T` so
    /// the daemon owns it — it survives detach, other clients see it, and
    /// name targeting can address it. The daemon's `%pane-title-changed`
    /// broadcast then re-applies the same value locally, which is
    /// idempotent.
    pub(crate) fn rename_pane(&mut self, pane_id: PaneId, name: &str) {
        let name = name.trim();
        let Some(tab) = self.tab_manager.active_tab_mut() else {
            return;
        };
        if !tab.rename_pane(pane_id, name) {
            return; // stale id — the pane closed while the popup was open
        }
        // Re-derive now rather than next frame: the blank branch must drop
        // the old name immediately, and a renamed focused pane should move
        // the tab title in the same frame.
        tab.update_title(
            self.config.load().tabs.tab_title_mode,
            self.config.load().tabs.remote_tab_title_format,
            self.config.load().tabs.remote_tab_title_osc_priority,
        );

        if let Some(tmux_pane_id) = self.tmux_state.tmux_pane_in_tab(tab.id, pane_id) {
            if name.contains(['\n', '\r', '\0']) {
                // A newline would end the wire command mid-title; the local
                // name stands, the daemon keeps the old one.
                crate::debug_error!("MUX", "pane title not wire-safe, skipped daemon push");
            } else {
                let value = if name.is_empty() {
                    "''".to_string()
                } else {
                    quote_pane_title(name)
                };
                let cmd = format!("select-pane -t %{tmux_pane_id} -T {value}");
                let sent = match self.tmux_state.transport.as_ref() {
                    Some(transport) => transport.send_command(cmd.trim_end()).is_ok(),
                    None => self.write_to_gateway(&cmd),
                };
                if sent {
                    crate::debug_info!("TMUX", "Renamed pane %{tmux_pane_id}");
                }
            }
        }
        self.request_redraw();
    }
}

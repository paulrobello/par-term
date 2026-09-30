//! The palette's runtime rows (UX.md OV6: one runtime-row builder).
//!
//! Every row that is not a dispatch-table built-in joins the palette here,
//! at open time: plugin actions, agent-authored commands, configured
//! launchable agents, captured crashes, the mux agent roster, the
//! attached par-mux session's rows, and the listed par-mux sessions.
//!
//! A free function over the fields it reads rather than a `WindowState`
//! method: the egui frame opens the palette from inside its render
//! closure, which captures `WindowState` fields disjointly — a whole-`self`
//! method call there would collide with the closure's other borrows. The
//! keybinding path and the status-bar agent chip both call this, so the
//! two paths can no longer drift apart (they had: the egui path lacked the
//! listed par-mux sessions).

use crate::agent_commands_store::AgentCommandStore;
use crate::command_palette::catalog::{
    PaletteEntry, agent_palette_entries, plugin_palette_entries,
};
use crate::config::Config;
use crate::crash_triage::CrashTriageState;
use crate::status_bar::StatusBarUI;

#[cfg(feature = "mux")]
use crate::app::tmux_handler::tmux_state::TmuxState;

/// Every runtime row, in join order (the palette sorts on open).
pub(crate) fn palette_runtime_rows(
    status_bar_ui: &StatusBarUI,
    agent_commands: &AgentCommandStore,
    crash_triage: &mut CrashTriageState,
    config: &Config,
    #[cfg(feature = "mux")] tmux_state: &TmuxState,
) -> Vec<PaletteEntry> {
    let mut rows = plugin_palette_entries(&status_bar_ui.plugin_host().palette_actions());
    // Agent-authored commands, hot-reloaded by the commands-dir watcher.
    rows.extend(agent_commands.palette_rows());
    // Configured launchable agents (the `agents:` config list).
    rows.extend(agent_palette_entries(&config.agents));
    // Captured crashes — the triage consent surface.
    rows.extend(crash_triage.palette_entries());
    #[cfg(feature = "mux")]
    {
        // Rostered agents, scoped to panes the app maps so every offered
        // row is focusable (A2b task 3).
        let map = &tmux_state.tmux_pane_owners;
        rows.extend(
            tmux_state
                .agent_roster
                .palette_rows(&|pane| map.contains_key(&pane)),
        );
        // The attached par-mux session's rows, present only while a
        // transport is installed.
        rows.extend(tmux_state.mux_palette_rows());
        // Listed par-mux sessions (A22), from the cached directory.
        rows.extend(crate::app::window_state::WindowState::mux_session_palette_rows(tmux_state));
    }
    rows
}

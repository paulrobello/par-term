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
    profiles: &crate::profile::ProfileManager,
    #[cfg(feature = "mux")] tmux_state: &TmuxState,
) -> Vec<PaletteEntry> {
    let mut rows = plugin_palette_entries(&status_bar_ui.plugin_host().palette_actions());
    // Four rows per profile (UX.md PR4); Manage Profiles is a built-in.
    rows.extend(profile_palette_entries(profiles));
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

/// The palette's profile rows (UX.md PR4): "Open Profile: X", "Open Profile
/// in New Window: X", "Split with Profile: X", and "Change Tab Profile: X"
/// for every profile, each running its profile registry action. A row's
/// chord column shows the live binding when the action is bound (an
/// `open_profile:` binding migrated from a profile shortcut, MD3) — the
/// palette fills it from the registry on open.
pub(crate) fn profile_palette_entries(
    profiles: &crate::profile::ProfileManager,
) -> Vec<PaletteEntry> {
    use crate::profile::actions::{ProfileAction, ProfileSplit};
    profiles
        .profiles_ordered()
        .into_iter()
        .flat_map(|p| {
            [
                ("Open Profile", ProfileAction::OpenTab(p.id)),
                (
                    "Open Profile in New Window",
                    ProfileAction::OpenWindow(p.id),
                ),
                (
                    "Split with Profile",
                    ProfileAction::Split(p.id, ProfileSplit::Right),
                ),
                ("Change Tab Profile", ProfileAction::SetTabProfile(p.id)),
            ]
            .map(|(verb, action)| PaletteEntry {
                action_id: action.id(),
                label: format!("{verb}: {}", p.name),
                chord: None,
                priority: 0,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_profile_gets_its_four_palette_rows() {
        // UX.md MP3 acceptance: the palette lists Open / Open in New Window
        // / Split with / Change Tab Profile rows.
        let mut profiles = crate::profile::ProfileManager::new();
        let work = crate::profile::Profile::new("Work");
        let id = work.id;
        profiles.add(work);
        let rows = profile_palette_entries(&profiles);
        let pairs: Vec<(String, String)> = rows
            .iter()
            .map(|r| (r.label.clone(), r.action_id.clone()))
            .collect();
        assert_eq!(
            pairs,
            [
                (
                    "Open Profile: Work".to_string(),
                    format!("open_profile:{id}")
                ),
                (
                    "Open Profile in New Window: Work".to_string(),
                    format!("open_profile_window:{id}")
                ),
                (
                    "Split with Profile: Work".to_string(),
                    format!("split_profile:{id}:right")
                ),
                (
                    "Change Tab Profile: Work".to_string(),
                    format!("set_tab_profile:{id}")
                ),
            ]
        );
        for row in &rows {
            assert_eq!(
                crate::command_palette::meta::category(&row.action_id),
                "Profiles"
            );
            assert!(crate::command_palette::meta::description(&row.action_id).is_some());
        }
    }
}

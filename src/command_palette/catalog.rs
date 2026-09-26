//! Builds the palette's action catalog by joining par-term's two dispatch
//! tables with the display names already maintained for the settings UI.
//!
//! The dispatch tables are the authority on what is *invocable*;
//! `AVAILABLE_ACTIONS` is the authority on what an action is *called*. Neither
//! alone can build a usable palette: the tables carry no labels, and the label
//! table is hand-maintained and may lag a newly added action. Actions missing a
//! label fall back to [`humanize`] rather than being hidden, so a new action is
//! always reachable even before someone writes its display name.

use crate::app::input_events::keybinding_actions::ACTION_HANDLERS;
use crate::app::input_events::keybinding_display_actions::DISPLAY_ACTION_HANDLERS;
use par_term_config::agent_launcher::AgentLaunchConfig;
use par_term_scripting::plugin_manager::PluginActionRow;
use par_term_settings_ui::input_tab::actions_table::AVAILABLE_ACTIONS;

/// One invocable row in the palette.
///
/// Owned id so the same row shape carries both built-ins (static ids) and
/// plugin contributions whose wire ids exist only at runtime.
#[derive(Clone)]
pub(crate) struct PaletteEntry {
    /// The string `execute_keybinding_action` dispatches on.
    pub(crate) action_id: String,
    /// Human-readable name — curated where one exists, derived otherwise.
    pub(crate) label: String,
    /// Default chord advertised for this action, shown right-aligned.
    pub(crate) chord: Option<&'static str>,
    /// Ordering boost: higher sorts first (ahead of the label sort), so
    /// runtime rows can lead the empty-query view. 0 is the default for
    /// built-ins and plugin rows; the agent-roster picker uses 1 for agent
    /// rows and 2 for blocked ones — the palette exists to answer "who is
    /// waiting", so blocked agents outrank everything.
    pub(crate) priority: u8,
}

/// Derive a display label from an action id.
///
/// Used only for actions with no `AVAILABLE_ACTIONS` entry. Underscores become
/// spaces and each word is title-cased, so `toggle_foo_bar` reads
/// `Toggle Foo Bar`.
pub(crate) fn humanize(action_id: &str) -> String {
    action_id
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build the full catalog of invocable actions, label-ordered.
pub(crate) fn build_catalog() -> Vec<PaletteEntry> {
    let mut entries: Vec<PaletteEntry> = ACTION_HANDLERS
        .iter()
        .map(|(id, _)| *id)
        .chain(DISPLAY_ACTION_HANDLERS.iter().map(|(id, _)| *id))
        .map(|action_id| {
            let curated = AVAILABLE_ACTIONS.iter().find(|(id, _, _)| *id == action_id);
            match curated {
                Some((_, display_name, chord)) => PaletteEntry {
                    action_id: action_id.to_string(),
                    label: (*display_name).to_string(),
                    chord: *chord,
                    priority: 0,
                },
                None => PaletteEntry {
                    action_id: action_id.to_string(),
                    label: humanize(action_id),
                    chord: None,
                    priority: 0,
                },
            }
        })
        .collect();

    entries.sort_by(|a, b| a.label.cmp(&b.label));
    entries
}

/// Palette entries for plugin-contributed actions, ordered by wire id.
///
/// The label arrives from the host already suffixed ` · <plugin name>` and is
/// passed through untouched; plugin actions have no default chord. Wire-id
/// order (not label order) makes the snapshot deterministic regardless of the
/// host's discovery order — the merged view re-sorts by label on open anyway.
pub(crate) fn plugin_palette_entries(rows: &[PluginActionRow]) -> Vec<PaletteEntry> {
    let mut entries: Vec<PaletteEntry> = rows
        .iter()
        .map(|row| PaletteEntry {
            action_id: row.wire_id.clone(),
            label: row.label.clone(),
            chord: None,
            priority: 0,
        })
        .collect();
    entries.sort_by(|a, b| a.action_id.cmp(&b.action_id));
    entries
}

/// Palette entries for configured launchable agents (the `agents:` config
/// list). Config order is preserved — the user's own ordering is the
/// ordering — and the merged view re-sorts by label on open anyway.
///
/// Every agent gets a plain "Launch <name>" row; an `autonomy_args` entry
/// additionally gets the labelled "(autonomous)" row — the explicit
/// consent surface for autonomy flags, which are never part of a plain
/// launch. A `default: true` entry adds the "Launch Default Agent" row.
pub(crate) fn agent_palette_entries(agents: &[AgentLaunchConfig]) -> Vec<PaletteEntry> {
    let mut entries: Vec<PaletteEntry> = agents
        .iter()
        .flat_map(|agent| {
            let mut rows = vec![PaletteEntry {
                action_id: format!("launch-agent:{}", agent.id),
                label: format!("Launch {}", agent.name),
                chord: None,
                priority: 0,
            }];
            if !agent.autonomy_args.trim().is_empty() {
                rows.push(PaletteEntry {
                    action_id: format!("launch-agent-autonomous:{}", agent.id),
                    label: format!("Launch {} (autonomous)", agent.name),
                    chord: None,
                    priority: 0,
                });
            }
            rows
        })
        .collect();
    if let Some(default) = par_term_config::agent_launcher::default_agent(agents) {
        entries.push(PaletteEntry {
            action_id: "launch-default-agent".to_string(),
            label: format!("Launch Default Agent ({})", default.name),
            chord: None,
            priority: 0,
        });
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humanize_turns_snake_case_into_title_case() {
        assert_eq!(humanize("toggle_fullscreen"), "Toggle Fullscreen");
        assert_eq!(humanize("new_tab"), "New Tab");
        assert_eq!(humanize("switch_to_tab_1"), "Switch To Tab 1");
    }

    #[test]
    fn catalog_is_not_empty() {
        assert!(
            build_catalog().len() > 50,
            "par-term dispatches 70+ actions; a near-empty catalog means the join failed"
        );
    }

    fn launch_agent(id: &str, autonomy: &str, default: bool) -> AgentLaunchConfig {
        AgentLaunchConfig {
            id: id.to_string(),
            name: id.to_string(),
            command: id.to_string(),
            autonomy_args: autonomy.to_string(),
            default,
        }
    }

    #[test]
    fn agent_entries_offering_autonomy_only_when_configured() {
        let entries = agent_palette_entries(&[
            launch_agent("claude", "--permission-mode auto", true),
            launch_agent("codex", "", false),
        ]);
        let ids: Vec<&str> = entries.iter().map(|e| e.action_id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "launch-agent:claude",
                "launch-agent-autonomous:claude",
                "launch-agent:codex",
                "launch-default-agent",
            ]
        );
        let autonomous = &entries[1];
        assert_eq!(autonomous.label, "Launch claude (autonomous)");
        let default = entries.last().unwrap();
        assert_eq!(default.label, "Launch Default Agent (claude)");
    }

    #[test]
    fn agent_entries_empty_when_unconfigured() {
        assert!(agent_palette_entries(&[]).is_empty());
    }

    #[test]
    fn catalog_has_no_duplicate_action_ids() {
        let catalog = build_catalog();
        let mut ids: Vec<&str> = catalog.iter().map(|e| e.action_id.as_str()).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(
            total,
            ids.len(),
            "an action appears twice — the two dispatch tables are asserted \
             disjoint by dispatch_tests::action_tables_are_disjoint, so a \
             duplicate here means the join double-counted"
        );
    }

    #[test]
    fn known_action_uses_its_curated_label_not_the_fallback() {
        let catalog = build_catalog();
        let entry = catalog
            .iter()
            .find(|e| e.action_id == "toggle_ai_inspector")
            .expect("toggle_ai_inspector is dispatchable");
        assert_eq!(
            entry.label, "Toggle Assistant Panel",
            "curated label from AVAILABLE_ACTIONS must win over humanize()"
        );
    }

    #[test]
    fn palette_action_has_a_curated_label() {
        let catalog = build_catalog();
        let entry = catalog
            .iter()
            .find(|e| e.action_id == "toggle_command_palette")
            .expect("toggle_command_palette is dispatchable");
        assert_eq!(
            entry.label, "Open Command Palette",
            "the palette's own action needs a curated label, not the humanize() fallback"
        );
    }

    #[test]
    fn curated_entry_carries_its_default_chord() {
        let catalog = build_catalog();
        let entry = catalog
            .iter()
            .find(|e| e.action_id == "toggle_fullscreen")
            .expect("toggle_fullscreen is dispatchable");
        assert!(
            entry.chord.is_some(),
            "toggle_fullscreen advertises a default chord in AVAILABLE_ACTIONS"
        );
    }

    #[test]
    fn catalog_is_sorted_by_label() {
        let catalog = build_catalog();
        let labels: Vec<&str> = catalog.iter().map(|e| e.label.as_str()).collect();
        let mut sorted = labels.clone();
        sorted.sort_unstable();
        assert_eq!(
            labels, sorted,
            "catalog must be label-ordered for a stable empty-query view"
        );
    }

    #[test]
    fn plugin_palette_entries_maps_host_rows_untouched() {
        let rows = [PluginActionRow {
            wire_id: "plugin-action:com.example.demo:say-hello".to_string(),
            label: "Say Hello · Demo Plugin".to_string(),
        }];
        let entries = plugin_palette_entries(&rows);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].action_id, "plugin-action:com.example.demo:say-hello",
            "the wire id is what execute_keybinding_action dispatches on — it \
             must survive the mapping byte for byte"
        );
        assert_eq!(
            entries[0].label, "Say Hello · Demo Plugin",
            "the host already suffixed the plugin name; the builder must not \
             add or strip anything"
        );
        assert!(
            entries[0].chord.is_none(),
            "plugin actions have no configured default chord"
        );
    }

    #[test]
    fn plugin_palette_entries_are_ordered_by_wire_id() {
        let rows = [
            PluginActionRow {
                wire_id: "plugin-action:com.zzz:act".to_string(),
                label: "Zeta · Z".to_string(),
            },
            PluginActionRow {
                wire_id: "plugin-action:com.aaa:act".to_string(),
                label: "Alpha · A".to_string(),
            },
        ];
        let entries = plugin_palette_entries(&rows);
        let ids: Vec<&str> = entries.iter().map(|e| e.action_id.as_str()).collect();
        assert_eq!(
            ids,
            ["plugin-action:com.aaa:act", "plugin-action:com.zzz:act"],
            "wire-id order makes the snapshot deterministic regardless of \
             discovery order"
        );
    }

    #[test]
    fn identically_labelled_plugins_yield_distinct_rows() {
        let rows = [
            PluginActionRow {
                wire_id: "plugin-action:com.a:go".to_string(),
                label: "Go · Same Name".to_string(),
            },
            PluginActionRow {
                wire_id: "plugin-action:com.b:go".to_string(),
                label: "Go · Same Name".to_string(),
            },
        ];
        let entries = plugin_palette_entries(&rows);
        assert_eq!(entries.len(), 2, "both rows must survive the mapping");
        assert_ne!(
            entries[0].action_id, entries[1].action_id,
            "identical labels must not collapse into one row — the wire id is \
             the only dispatch key"
        );
    }

    #[test]
    fn every_dispatchable_action_appears_with_a_nonempty_label() {
        // The join must never hide an action: whatever the label table says,
        // each dispatchable id yields exactly one row with a usable label.
        // Today AVAILABLE_ACTIONS covers all 67 ids, so the humanize() arm is
        // unexercised by this test — verified by mutation experiment instead
        // (2026-09-20: hiding paste_special's entry left it present, labelled
        // "Paste Special" via the fallback).
        use crate::app::input_events::keybinding_actions::ACTION_HANDLERS;
        use crate::app::input_events::keybinding_display_actions::DISPLAY_ACTION_HANDLERS;
        let catalog = build_catalog();
        assert_eq!(
            catalog.len(),
            ACTION_HANDLERS.len() + DISPLAY_ACTION_HANDLERS.len(),
            "every dispatchable action must yield exactly one palette row"
        );
        assert!(catalog.iter().all(|e| !e.label.is_empty()));
    }
}

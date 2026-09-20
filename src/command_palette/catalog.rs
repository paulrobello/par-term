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
use par_term_settings_ui::input_tab::actions_table::AVAILABLE_ACTIONS;

/// One invocable row in the palette.
pub(crate) struct PaletteEntry {
    /// The string `execute_keybinding_action` dispatches on.
    pub(crate) action_id: &'static str,
    /// Human-readable name — curated where one exists, derived otherwise.
    pub(crate) label: String,
    /// Default chord advertised for this action, shown right-aligned.
    pub(crate) chord: Option<&'static str>,
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
                    action_id,
                    label: (*display_name).to_string(),
                    chord: *chord,
                },
                None => PaletteEntry {
                    action_id,
                    label: humanize(action_id),
                    chord: None,
                },
            }
        })
        .collect();

    entries.sort_by(|a, b| a.label.cmp(&b.label));
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

    #[test]
    fn catalog_has_no_duplicate_action_ids() {
        let catalog = build_catalog();
        let mut ids: Vec<&str> = catalog.iter().map(|e| e.action_id).collect();
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

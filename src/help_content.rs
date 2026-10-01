//! Generated content for the F1 help panel (UX.md V12).
//!
//! Nothing here is a hand-maintained chord list. The rows come from three
//! live sources:
//!
//! - **Which actions exist** and **what they are called**: the palette
//!   catalog ([`build_catalog`]), which joins the two dispatch tables with
//!   the curated `AVAILABLE_ACTIONS` labels.
//! - **Which chord runs each action**: the live [`KeybindingRegistry`]. An
//!   action with no binding is left out, so a user who unbinds a chord never
//!   sees it advertised.
//! - **Menu-only commands** (Copy and Paste, which run on dedicated clipboard
//!   paths rather than registry actions): the menu model's accelerators.
//!
//! The one piece of curation is [`section_for`], which files an action id
//! under Window / Tab / Pane / Session / … by its name. A test asserts that
//! every action the shipped defaults bind lands in a named section, so a new
//! action that fits none fails CI instead of silently falling into "Other".
//!
//! Modal keys (copy mode's vi keys, the search bar's Enter/Escape) are not
//! registry bindings. Their tables live next to the handlers that implement
//! them and are rendered from there; see [`MODAL_SECTIONS`].

use crate::command_palette::catalog::{build_catalog, chord_display};
use crate::menu::model::{all_items, menu_model_with};
use par_term_config::KeyBinding;
use par_term_keybindings::KeybindingRegistry;

/// Help sections, in display order.
pub(crate) const SECTIONS: &[&str] = &[
    "Window",
    "Tab",
    "Pane",
    "Sessions & Profiles",
    "Edit & Search",
    "Scrolling",
    "View",
    "Terminal",
    "Other",
];

/// One generated shortcut row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HelpRow {
    /// Dispatch id, or `menu:<id>` for a menu-only command.
    pub(crate) action_id: String,
    /// Display label (curated where one exists).
    pub(crate) label: String,
    /// The live chord, formatted like the palette formats it.
    pub(crate) chord: String,
}

/// One titled group of rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HelpSection {
    pub(crate) title: &'static str,
    pub(crate) rows: Vec<HelpRow>,
}

/// File an action id under a help section by its name.
pub(crate) fn section_for(action_id: &str) -> &'static str {
    let id = action_id;
    let has = |needle: &str| id.contains(needle);
    if has("pane")
        || id.starts_with("split_")
        || id.starts_with("layout:")
        || matches!(
            id,
            "toggle_broadcast_input" | "cycle_layout" | "enter_resize_mode"
        )
    {
        "Pane"
    } else if has("tab") {
        "Tab"
    } else if has("window")
        || matches!(
            id,
            "quit" | "toggle_always_on_top" | "toggle_tree_picker" | "menu:minimize" | "menu:zoom"
        )
    {
        "Window"
    } else if has("session_picker")
        || has("profile")
        || has("mux_session")
        || matches!(
            id,
            "ssh_quick_connect" | "save_arrangement" | "detach" | "focus_next_attention_agent"
        )
    {
        "Sessions & Profiles"
    } else if has("search")
        || has("history")
        || has("paste")
        || has("copy")
        || id == "select_all"
        || id.starts_with("menu:")
    {
        "Edit & Search"
    } else if id.starts_with("scroll_") {
        "Scrolling"
    } else if has("font")
        || has("fullscreen")
        || has("maximize")
        || has("shader")
        || matches!(
            id,
            "toggle_fps_overlay"
                | "toggle_help"
                | "open_settings"
                | "reload_config"
                | "toggle_command_palette"
                | "toggle_menu"
                | "toggle_ai_inspector"
                | "toggle_agent_usage_panel"
                | "cycle_cursor_style"
                | "toggle_throughput_mode"
        )
    {
        "View"
    } else if has("clear") || matches!(id, "toggle_session_logging") {
        "Terminal"
    } else {
        "Other"
    }
}

/// A menu accelerator in [`chord_display`]'s spelling (`Cmd+Shift+C`), so
/// menu rows and registry rows read alike.
fn spelled_out(accel: &muda::accelerator::Accelerator) -> String {
    use muda::accelerator::{Accelerator, Modifiers};
    let mods = accel.modifiers();
    let mut parts: Vec<String> = Vec::new();
    let named = [
        (
            Modifiers::META,
            if cfg!(target_os = "macos") {
                "Cmd"
            } else {
                "Super"
            },
        ),
        (Modifiers::CONTROL, "Ctrl"),
        (Modifiers::ALT, "Alt"),
        (Modifiers::SHIFT, "Shift"),
    ];
    for (flag, word) in named {
        if mods.contains(flag) {
            parts.push(word.to_string());
        }
    }
    let key_only = Accelerator::new(Modifiers::empty(), accel.key());
    parts.push(crate::menu::model::accelerator_label(&key_only));
    parts.join("+")
}

/// Menu commands that carry a chord but no registry action (Copy, Paste),
/// read from the menu model built with `keybindings`.
fn menu_only_rows(keybindings: &[KeyBinding]) -> Vec<HelpRow> {
    let model = menu_model_with(cfg!(target_os = "macos"), keybindings);
    all_items(&model)
        .into_iter()
        .filter(|item| item.action.keybinding_action().is_none())
        .filter_map(|item| {
            let accel = item.accelerator?;
            Some(HelpRow {
                action_id: format!("menu:{}", item.id),
                label: item.label.to_string(),
                chord: spelled_out(&accel),
            })
        })
        .collect()
}

/// Build the help panel's shortcut sections from the live registry and the
/// menu model. `keybindings` must be the list `registry` was built from.
pub(crate) fn shortcut_sections(
    registry: &KeybindingRegistry,
    keybindings: &[KeyBinding],
) -> Vec<HelpSection> {
    let mut rows: Vec<HelpRow> = build_catalog()
        .into_iter()
        .filter_map(|entry| {
            let combo = registry.chord_for_action(&entry.action_id)?;
            Some(HelpRow {
                action_id: entry.action_id,
                label: entry.label,
                chord: chord_display(&combo),
            })
        })
        .collect();
    rows.extend(menu_only_rows(keybindings));

    SECTIONS
        .iter()
        .map(|title| HelpSection {
            title,
            rows: rows
                .iter()
                .filter(|row| section_for(&row.action_id) == *title)
                .cloned()
                .collect(),
        })
        .filter(|section| !section.rows.is_empty())
        .collect()
}

/// Modal key tables rendered after the generated sections. Each table is
/// owned by the module whose handler implements those keys.
pub(crate) const MODAL_SECTIONS: &[(&str, &[(&str, &str)])] = &[
    ("In the search bar", crate::search::SEARCH_BAR_KEYS),
    (
        "Copy mode (vi-style)",
        crate::app::copy_mode::handler::COPY_MODE_KEYS,
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> (Vec<KeyBinding>, KeybindingRegistry) {
        let bindings = par_term_config::Config::default().keybindings;
        let registry = KeybindingRegistry::from_config(&bindings);
        (bindings, registry)
    }

    fn all_rows(sections: &[HelpSection]) -> Vec<&HelpRow> {
        sections.iter().flat_map(|s| s.rows.iter()).collect()
    }

    /// Independent oracle: walk the shipped default keybindings themselves
    /// (not the catalog/registry the generator uses). Every default whose
    /// action is dispatchable has a row whose chord is that action's FIRST
    /// default binding, parsed; every other default is named here as a
    /// deliberate exclusion.
    #[test]
    fn every_shipped_default_binding_is_in_the_help() {
        use crate::app::input_events::keybinding_actions::ACTION_HANDLERS;
        use crate::app::input_events::keybinding_display_actions::DISPLAY_ACTION_HANDLERS;
        let (bindings, registry) = defaults();
        let sections = shortcut_sections(&registry, &bindings);
        let rows = all_rows(&sections);
        let dispatchable = |id: &str| {
            ACTION_HANDLERS.iter().any(|(a, _)| *a == id)
                || DISPLAY_ACTION_HANDLERS.iter().any(|(a, _)| *a == id)
        };
        // Defaults bound to non-table actions: none ship today. A new one
        // must be added here on purpose (and shown or excluded).
        const EXCLUDED: &[&str] = &[];
        let mut seen: Vec<&str> = Vec::new();
        for kb in &bindings {
            let action =
                par_term_config::config::keybindings_methods::current_action_id(&kb.action);
            if seen.contains(&action) {
                continue; // only the first (primary) binding is advertised
            }
            seen.push(action);
            if !dispatchable(action) {
                assert!(
                    EXCLUDED.contains(&action),
                    "default {:?} -> {action} is not dispatchable and not excluded",
                    kb.key
                );
                continue;
            }
            let want = chord_display(
                &par_term_keybindings::parser::parse_key_combo(&kb.key)
                    .expect("default parses")
                    .platform_normalized(),
            );
            let row = rows
                .iter()
                .find(|r| r.action_id == action)
                .unwrap_or_else(|| panic!("default-bound {action} has no help row"));
            assert_eq!(row.chord, want, "{action}");
        }
    }

    /// Regeneration matches the shipped defaults: every dispatchable action
    /// the default config binds appears exactly once, with its registry
    /// chord, and nothing unbound appears.
    #[test]
    fn generated_rows_match_the_shipped_defaults() {
        let (bindings, registry) = defaults();
        let sections = shortcut_sections(&registry, &bindings);
        let rows = all_rows(&sections);

        let expected: Vec<String> = build_catalog()
            .into_iter()
            .filter(|e| registry.chord_for_action(&e.action_id).is_some())
            .map(|e| e.action_id)
            .collect();
        let mut got: Vec<String> = rows
            .iter()
            .filter(|r| !r.action_id.starts_with("menu:"))
            .map(|r| r.action_id.clone())
            .collect();
        let mut want = expected.clone();
        got.sort();
        want.sort();
        assert_eq!(got, want, "help rows must be exactly the bound actions");

        for row in rows.iter().filter(|r| !r.action_id.starts_with("menu:")) {
            let combo = registry.chord_for_action(&row.action_id).unwrap();
            assert_eq!(row.chord, chord_display(&combo), "{}", row.action_id);
        }
    }

    /// Every shipped-default row lands in a named section, so a new action
    /// is filed on purpose rather than drifting into "Other".
    #[test]
    fn no_default_action_falls_into_other() {
        let (bindings, registry) = defaults();
        let other: Vec<String> = shortcut_sections(&registry, &bindings)
            .into_iter()
            .filter(|s| s.title == "Other")
            .flat_map(|s| s.rows.into_iter().map(|r| r.action_id))
            .collect();
        assert!(other.is_empty(), "unfiled help rows: {other:?}");
    }

    /// The Window / Tab / Pane sections the UX plan asks for are present
    /// with the defaults (V12).
    #[test]
    fn window_tab_and_pane_sections_exist() {
        let (bindings, registry) = defaults();
        let titles: Vec<&str> = shortcut_sections(&registry, &bindings)
            .iter()
            .map(|s| s.title)
            .collect();
        for want in ["Window", "Tab", "Pane"] {
            assert!(titles.contains(&want), "missing {want} in {titles:?}");
        }
    }

    /// A rebind shows the new chord; an unbound action disappears.
    #[test]
    fn a_rebind_regenerates_the_rows() {
        let bindings = vec![KeyBinding {
            key: "F9".to_string(),
            action: "split_right".to_string(),
        }];
        let registry = KeybindingRegistry::from_config(&bindings);
        let sections = shortcut_sections(&registry, &bindings);
        let rows = all_rows(&sections);
        let split = rows
            .iter()
            .find(|r| r.action_id == "split_right")
            .expect("bound action is listed");
        assert_eq!(split.chord, "F9");
        assert!(!rows.iter().any(|r| r.action_id == "split_down"));
    }

    /// Copy and Paste come from the menu model, not a literal.
    #[test]
    fn clipboard_rows_come_from_the_menu_model() {
        let (bindings, registry) = defaults();
        let sections = shortcut_sections(&registry, &bindings);
        let ids: Vec<&str> = all_rows(&sections)
            .iter()
            .map(|r| r.action_id.as_str())
            .collect();
        assert!(ids.contains(&"menu:copy") && ids.contains(&"menu:paste"));
        let copy = all_rows(&sections)
            .into_iter()
            .find(|r| r.action_id == "menu:copy")
            .unwrap()
            .chord
            .clone();
        #[cfg(target_os = "macos")]
        assert_eq!(copy, "Cmd+C");
        #[cfg(not(target_os = "macos"))]
        assert_eq!(copy, "Ctrl+Shift+C");
    }
}

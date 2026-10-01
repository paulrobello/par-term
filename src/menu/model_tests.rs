use super::*;
use crate::app::input_events::keybinding_actions::ACTION_HANDLERS;
use crate::app::input_events::keybinding_display_actions::DISPLAY_ACTION_HANDLERS;
use std::collections::{BTreeSet, HashSet};

/// Registry actions that deliberately have no menu home, each with the
/// reason. Everything else the two dispatch tables can run must be reachable
/// from the menu (UX.md MN1).
const NOT_IN_MENU: &[(&str, &str)] = &[
    (
        "toggle_menu",
        "opens the in-app menu itself; a menu item cannot open its own menu",
    ),
    (
        "close_tab_or_window",
        "legacy smart close kept for configs that bind it; Shell › Close, Close Tab and \
         Close Window cover the same ground (UX.md A14)",
    ),
    (
        "enter_copy_mode",
        "one-way twin of toggle_copy_mode (Edit › Copy Mode); the menu shows the toggle",
    ),
    (
        "open_settings",
        "reached through Settings... (MenuAction::OpenSettings), which also serves the \
         macOS application menu",
    ),
];

/// Menu commands that run no registry action, each with the reason.
const MENU_ONLY: &[(&str, &str)] = &[
    (
        "copy",
        "dedicated clipboard path that also feeds egui text fields",
    ),
    (
        "paste",
        "dedicated clipboard path that also feeds egui text fields",
    ),
    ("minimize", "window-level command with no registry action"),
    ("zoom", "window-level command with no registry action"),
    ("about", "opens the About section of the help overlay"),
    ("open_docs", "opens the documentation in the browser"),
    (
        "install_remote_shell_integration",
        "dialog with no registry action",
    ),
];

fn ids(model: &[MenuSection]) -> Vec<&'static str> {
    all_items(model).into_iter().map(|spec| spec.id).collect()
}

/// The registry actions the menu runs, app menu included on macOS.
fn reachable_actions(has_native_app_menu: bool) -> HashSet<String> {
    let model = menu_model(has_native_app_menu);
    let mut actions: HashSet<String> = all_items(&model)
        .into_iter()
        .filter_map(|spec| spec.action.keybinding_action())
        .map(|id| id.into_owned())
        .collect();
    if has_native_app_menu {
        actions.extend(
            APP_MENU_ACTIONS
                .iter()
                .filter_map(|a| a.keybinding_action())
                .map(|id| id.into_owned()),
        );
    }
    actions
}

/// UX.md MN1 (test): every user-facing registry action has a menu home, on
/// both platform variants, and every menu item maps to a registry action or
/// is a listed menu-only command.
#[test]
fn every_registry_action_has_a_menu_home() {
    let excluded: HashSet<&str> = NOT_IN_MENU.iter().map(|(id, _)| *id).collect();
    for has_native_app_menu in [false, true] {
        let reachable = reachable_actions(has_native_app_menu);
        let homeless: BTreeSet<&str> = ACTION_HANDLERS
            .iter()
            .map(|(id, _)| *id)
            .chain(DISPLAY_ACTION_HANDLERS.iter().map(|(id, _)| *id))
            .filter(|id| !excluded.contains(id) && !reachable.contains(*id))
            .collect();
        assert!(
            homeless.is_empty(),
            "registry actions with no menu home (has_native_app_menu={has_native_app_menu}): \
             {homeless:?} — add a menu item, or list the action in NOT_IN_MENU with a reason"
        );
    }

    // The exclusion list cannot hide a typo or a dead entry.
    for (id, _) in NOT_IN_MENU {
        assert!(
            ACTION_HANDLERS.iter().any(|(a, _)| a == id)
                || DISPLAY_ACTION_HANDLERS.iter().any(|(a, _)| a == id),
            "NOT_IN_MENU lists {id:?}, which no dispatch table handles"
        );
    }
}

/// UX.md MN1, the other direction: every item maps to a registry action that
/// a dispatch table handles, or is a listed menu-only command.
#[test]
fn every_menu_item_maps_to_a_registry_action() {
    let menu_only: HashSet<&str> = MENU_ONLY.iter().map(|(id, _)| *id).collect();
    let handled = |id: &str| {
        ACTION_HANDLERS.iter().any(|(a, _)| *a == id)
            || DISPLAY_ACTION_HANDLERS.iter().any(|(a, _)| *a == id)
    };
    for has_native_app_menu in [false, true] {
        for spec in all_items(&menu_model(has_native_app_menu)) {
            match spec.action.keybinding_action() {
                Some(id) => assert!(
                    handled(&id),
                    "menu item {:?} runs {id:?}, which no dispatch table handles",
                    spec.id
                ),
                None => assert!(
                    menu_only.contains(spec.id),
                    "menu item {:?} runs no registry action and is not in MENU_ONLY",
                    spec.id
                ),
            }
        }
    }
}

#[test]
fn menu_ids_are_unique() {
    for has_native_app_menu in [false, true] {
        let model = menu_model(has_native_app_menu);
        let mut seen = HashSet::new();
        for id in ids(&model) {
            assert!(
                seen.insert(id),
                "duplicate menu id {id:?} (has_native_app_menu={has_native_app_menu})"
            );
        }
        // Submenu ids share muda's id space with items.
        fn submenus(entries: &[MenuEntry], out: &mut Vec<&'static str>) {
            for entry in entries {
                if let MenuEntry::Submenu(sub) = entry {
                    out.push(sub.id);
                    submenus(&sub.entries, out);
                }
            }
        }
        let mut subs = Vec::new();
        for section in &model {
            submenus(&section.entries, &mut subs);
        }
        for id in subs {
            assert!(seen.insert(id), "submenu id {id:?} collides");
        }
    }
}

/// MD1: the iTerm2 section order on every platform.
#[test]
fn sections_follow_md1() {
    for has_native_app_menu in [false, true] {
        let titles: Vec<&str> = menu_model(has_native_app_menu)
            .iter()
            .map(|s| s.title)
            .collect();
        assert_eq!(
            titles,
            [
                "Shell",
                "Edit",
                "View",
                "Session",
                "Profiles",
                WINDOW_SECTION_TITLE,
                HELP_SECTION_TITLE
            ]
        );
    }
}

/// Without a native application menu the model carries Quit, Settings and
/// About itself; with one (macOS) they live in the application menu.
#[test]
fn app_menu_commands_fold_into_the_model_without_a_native_app_menu() {
    let without: Vec<MenuAction> = all_items(&menu_model(false))
        .into_iter()
        .map(|s| s.action)
        .collect();
    let with: Vec<MenuAction> = all_items(&menu_model(true))
        .into_iter()
        .map(|s| s.action)
        .collect();
    for action in APP_MENU_ACTIONS {
        assert!(without.contains(action), "{action:?} missing off macOS");
        assert!(!with.contains(action), "{action:?} duplicated on macOS");
    }
}

/// The two variants differ only by the application-menu commands.
#[test]
fn variants_differ_only_by_app_menu_items() {
    let with_app_menu: HashSet<&str> = ids(&menu_model(true)).into_iter().collect();
    let without: HashSet<&str> = ids(&menu_model(false)).into_iter().collect();
    let mut extra: Vec<&&str> = without.difference(&with_app_menu).collect();
    extra.sort();
    assert_eq!(extra, [&"about", &"quit", &"settings"]);
    assert!(with_app_menu.difference(&without).next().is_none());
}

#[test]
fn every_section_has_entries() {
    for section in menu_model(false) {
        assert!(
            !section.entries.is_empty(),
            "section {:?} is empty",
            section.title
        );
    }
}

/// The dynamic insertion points exist exactly once each.
#[test]
fn dynamic_placeholders_appear_once() {
    fn count(entries: &[MenuEntry], f: &dyn Fn(&MenuEntry) -> bool) -> usize {
        entries
            .iter()
            .map(|entry| match entry {
                MenuEntry::Submenu(sub) => count(&sub.entries, f),
                other => usize::from(f(other)),
            })
            .sum()
    }
    let model = menu_model(false);
    let total = |f: &dyn Fn(&MenuEntry) -> bool| -> usize {
        model.iter().map(|s| count(&s.entries, f)).sum()
    };
    assert_eq!(total(&|e| matches!(e, MenuEntry::Profiles)), 1);
    assert_eq!(total(&|e| matches!(e, MenuEntry::Arrangements)), 1);
}

/// Toggles carry a checkmark rule (MN2); every action whose name says it
/// toggles a panel or mode is one.
#[test]
fn toggles_are_checkable() {
    let not_checkable = [
        // Opens a transient picker or overlay rather than a persistent state.
        "toggle_command_palette",
        "toggle_tree_picker",
        "toggle_session_picker",
        "toggle_search",
        "toggle_clipboard_history",
        "toggle_command_history",
        "toggle_help",
        // Open Profiles… (UX.md PR1): the id predates the launcher popup.
        // The persistent drawer is toggle_profiles_panel.
        "toggle_profile_drawer",
    ];
    for spec in all_items(&menu_model(false)) {
        let Some(id) = spec.action.keybinding_action() else {
            continue;
        };
        if id.starts_with("toggle_") && !not_checkable.contains(&id.as_ref()) {
            assert!(
                spec.check.is_some(),
                "{} toggles but has no checkmark",
                spec.id
            );
        }
    }
}

/// MN2: pane operations need a second pane.
#[test]
fn pane_operations_require_multiple_panes() {
    let model = menu_model(false);
    for id in [
        "navigate_pane_left",
        "next_pane",
        "select_pane_hint",
        "resize_pane_up",
        "swap_pane_down",
        "equalize_panes",
        "toggle_pane_zoom",
        "promote_pane_to_tab",
        "toggle_pane_broadcast",
    ] {
        let spec = all_items(&model)
            .into_iter()
            .find(|s| s.id == id)
            .unwrap_or_else(|| panic!("no menu item {id:?}"));
        assert_eq!(spec.requires, Requires::MultiplePanes, "{id}");
    }
}

/// The order-sensitive snapshot of the top level of every section.
///
/// The macOS and Windows menus are built by walking this model, and neither
/// can be exercised from the other's CI. A reordered section, a dropped
/// separator or a renamed label is invisible to every other test here.
/// Update it deliberately when the menu changes.
#[test]
fn model_matches_the_shipped_menu_structure() {
    fn outline(entries: &[MenuEntry], prefix: &str, out: &mut Vec<String>) {
        for entry in entries {
            match entry {
                MenuEntry::Item(spec) => out.push(format!("{prefix}/{}", spec.label)),
                MenuEntry::Separator => out.push(format!("{prefix}/---")),
                MenuEntry::Profiles => out.push(format!("{prefix}/<profiles>")),
                MenuEntry::Arrangements => out.push(format!("{prefix}/<arrangements>")),
                MenuEntry::BringAllToFront => out.push(format!("{prefix}/<bring all to front>")),
                MenuEntry::Submenu(sub) => {
                    out.push(format!("{prefix}/{} >", sub.title));
                    outline(&sub.entries, &format!("{prefix}/{}", sub.title), out);
                }
            }
        }
    }
    let mut lines = Vec::new();
    for section in menu_model(false) {
        outline(&section.entries, section.title, &mut lines);
    }
    let text = lines.join("\n");
    let expected = include_str!("model_snapshot.txt").trim_end();
    if cfg!(target_os = "macos") {
        // The macOS model adds Bring All to Front after the window list.
        assert_eq!(
            text,
            expected.replace(
                "Window/Select Window/Window 9",
                "Window/Select Window/Window 9\nWindow/---\nWindow/<bring all to front>"
            )
        );
    } else {
        assert_eq!(text, expected);
    }
}

#[test]
fn accelerator_labels_are_readable() {
    let plain = Accelerator::new(Modifiers::empty(), Code::F11);
    assert_eq!(accelerator_label(&plain), "F11");

    let bracket = Accelerator::new(Modifiers::SHIFT, Code::BracketRight);
    let label = accelerator_label(&bracket);
    assert!(label.ends_with(']'), "unexpected label {label:?}");

    let digit = Accelerator::new(Modifiers::ALT, Code::Digit1);
    assert!(accelerator_label(&digit).ends_with('1'));

    // The arrow keys reach the label through `code_label`'s fallback unless
    // they are named, which would print "ArrowLeft" in the in-app menu.
    for (code, word) in [
        (Code::ArrowLeft, "Left"),
        (Code::ArrowRight, "Right"),
        (Code::ArrowUp, "Up"),
        (Code::ArrowDown, "Down"),
    ] {
        let arrow = Accelerator::new(Modifiers::SHIFT, code);
        let label = accelerator_label(&arrow);
        assert!(label.ends_with(word), "unexpected label {label:?}");
        assert!(!label.contains("Arrow"));
    }
}

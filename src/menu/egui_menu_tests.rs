use super::*;
use crate::menu::MenuAction;
use crate::menu::model::all_items;
use crate::menu::state::Requires;

#[test]
fn env_override_forces_the_menu_on_or_off() {
    assert!(enabled_with_override(Some("1")));
    assert!(enabled_with_override(Some("true")));
    assert!(!enabled_with_override(Some("0")));
    assert!(!enabled_with_override(Some("off")));
}

/// Unset, or set to nonsense, the platform decides.
#[test]
fn default_follows_the_platform() {
    let expected = !HAS_NATIVE_MENU_BAR;
    assert_eq!(enabled_with_override(None), expected);
    assert_eq!(enabled_with_override(Some("maybe")), expected);
    assert_eq!(enabled_with_override(Some("")), expected);
}

/// The menu is drawn exactly where par-term cannot attach a native one.
#[test]
fn platform_default_matches_native_menu_availability() {
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        assert!(!enabled_with_override(None));
    } else {
        assert!(enabled_with_override(None));
    }
}

/// The in-app menu is the only menu wherever it is drawn, so it carries the
/// MD1 sections and the commands a native application menu would own.
#[test]
fn menu_carries_the_full_command_set() {
    let menu = AppMenuUi::new();
    let titles: Vec<&str> = menu.sections.iter().map(|s| s.title).collect();
    assert_eq!(
        titles,
        [
            "Shell", "Edit", "View", "Session", "Profiles", "Window", "Help"
        ]
    );

    let actions: Vec<MenuAction> = all_items(&menu.sections)
        .into_iter()
        .map(|spec| spec.action)
        .collect();
    for required in [
        MenuAction::NewWindow,
        MenuAction::Action("close_pane"),
        MenuAction::Quit,
        MenuAction::SelectAll,
        MenuAction::OpenSettings,
        MenuAction::About,
        MenuAction::Action("maximize_vertically"),
    ] {
        assert!(
            actions.contains(&required),
            "in-app menu is missing {required:?}, which has no keybinding on Linux"
        );
    }
}

/// UX.md MN3 for the in-app menu: a rebind relabels the item, an unbind
/// drops its chord, and re-syncing the same bindings does not rebuild.
#[test]
fn sync_rebuilds_when_the_bindings_change() {
    let mut bindings = par_term_config::Config::default().keybindings;
    let mut menu = AppMenuUi::new_with(&bindings);
    let chord_of = |menu: &AppMenuUi, id: &str| {
        all_items(menu.sections())
            .into_iter()
            .find(|s| s.id == id)
            .and_then(|s| s.accelerator)
            .map(|a| model::accelerator_label(&a))
    };
    assert!(chord_of(&menu, "new_tab").is_some());
    assert!(
        !menu.sync(&bindings, MenuState::default()),
        "no change, no rebuild"
    );

    bindings.retain(|kb| kb.action != "new_tab");
    bindings.push(KeyBinding {
        key: "Ctrl+Alt+F2".to_string(),
        action: "new_tab".to_string(),
    });
    bindings.retain(|kb| kb.action != "clear_scrollback");
    assert!(
        menu.sync(&bindings, MenuState::default()),
        "rebind rebuilds"
    );
    let expected = crate::menu::registry_accel::accelerator_from_chord("Ctrl+Alt+F2")
        .map(|a| model::accelerator_label(&a));
    assert_eq!(chord_of(&menu, "new_tab"), expected);
    assert_eq!(
        chord_of(&menu, "clear_scrollback"),
        None,
        "an unbound action must not keep a chord"
    );
}

/// A freshly built menu must not claim to be capturing input.
#[test]
fn menu_starts_closed() {
    assert!(!AppMenuUi::new().is_open());
}

/// Every section's entries must render — separators, submenus, items with
/// and without an accelerator, toggles, disabled items, and the dynamic
/// insertion points.
#[test]
fn every_section_renders_its_entries() {
    let menu = AppMenuUi::new();
    let profiles = crate::profile::ProfileManager::new();
    let mut state = MenuState {
        tab_count: 2,
        tab_titles: vec!["build".into(), "logs".into()],
        arrangements: vec![(uuid::Uuid::new_v4(), "work".into())],
        ..MenuState::default()
    };
    state.checks.insert(crate::menu::state::Check::FpsOverlay);
    egui::__run_test_ui(|ui| {
        for section in &menu.sections {
            draw_entries(ui, &section.entries, &state, &profiles, &menu.registry);
        }
    });
}

/// Tab N items are disabled past the tab count; the state drives it.
#[test]
fn tab_items_follow_the_tab_count() {
    let menu = AppMenuUi::new();
    let state = MenuState {
        tab_count: 2,
        tab_titles: vec!["build".into(), "logs".into()],
        ..MenuState::default()
    };
    for spec in all_items(&menu.sections) {
        if let Requires::Tab(n) = spec.requires {
            assert_eq!(state.enabled(spec), n <= 2, "Tab {n}");
        }
    }
}

/// Drive the real egui code path headlessly: closed, then opened through
/// the same toggle request a `toggle_menu` keybinding would leave behind,
/// then closed again. Covers the trigger button and the drop-down's
/// top level.
#[test]
fn opens_and_closes_through_the_toggle_request() {
    let _guard = super::bridge::TEST_LOCK.lock();
    let _ = bridge::take_toggle_request();
    let _ = bridge::drain_pending_actions();

    let ctx = egui::Context::default();
    let profiles = crate::profile::ProfileManager::new();
    let mut menu = AppMenuUi::new();

    let frame = |menu: &mut AppMenuUi| {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::Panel::top("test_bar").show(ui, |ui| {
                menu.show(ui, &profiles, 24.0);
            });
        });
        // Headless test: no renderer applies the font-atlas delta, and egui
        // 0.36 panics on drop of unapplied deltas.
        out.textures_delta.clear();
    };

    frame(&mut menu);
    assert!(!menu.is_open(), "the menu must start closed");

    // The request is consumed after this frame reads the popup state, so
    // the drop-down appears on the frame after that.
    bridge::request_toggle();
    frame(&mut menu);
    frame(&mut menu);
    assert!(menu.is_open(), "toggle request did not open the menu");

    bridge::request_toggle();
    frame(&mut menu);
    frame(&mut menu);
    assert!(!menu.is_open(), "toggle request did not close the menu");

    // Drawing the menu must not dispatch anything on its own.
    assert!(bridge::drain_pending_actions().is_empty());
}

/// Hiding the bar must leave nothing behind that re-opens the menu later.
///
/// Both halves of this regressed once: egui's popup memory kept the
/// drop-down open across the hidden frames, and a toggle request raised
/// while the bar was hidden latched until the bar came back.
#[test]
fn hiding_the_bar_discards_open_state_and_toggle_requests() {
    let _guard = super::bridge::TEST_LOCK.lock();
    let _ = bridge::take_toggle_request();

    let ctx = egui::Context::default();
    let profiles = crate::profile::ProfileManager::new();
    let mut menu = AppMenuUi::new();

    let frame = |menu: &mut AppMenuUi| {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::Panel::top("test_bar").show(ui, |ui| {
                menu.show(ui, &profiles, 24.0);
            });
        });
        // Headless test: no renderer applies the font-atlas delta, and egui
        // 0.36 panics on drop of unapplied deltas.
        out.textures_delta.clear();
    };

    bridge::request_toggle();
    frame(&mut menu);
    frame(&mut menu);
    assert!(menu.is_open());

    // The bar is hidden while the drop-down is open.
    menu.hide(&ctx);
    assert!(!menu.is_open());

    // And a keybinding fires while there is no menu to toggle.
    bridge::request_toggle();
    menu.hide(&ctx);
    assert!(!bridge::take_toggle_request(), "toggle request latched");

    // The bar comes back: the menu must still be closed.
    frame(&mut menu);
    frame(&mut menu);
    assert!(
        !menu.is_open(),
        "the menu re-opened itself after being hidden"
    );
}

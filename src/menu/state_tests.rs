use super::*;
use crate::menu::model::{all_items, menu_model};
use muda::accelerator::Modifiers;

fn spec(id: &str) -> MenuItemSpec {
    for has_native_app_menu in [false, true] {
        if let Some(spec) = all_items(&menu_model(has_native_app_menu))
            .into_iter()
            .find(|s| s.id == id)
        {
            return spec.clone();
        }
    }
    panic!("no menu item {id:?}");
}

#[test]
fn rules_follow_the_state() {
    let one = MenuState {
        tab_count: 1,
        windows: vec![(1, String::new())],
        ..MenuState::default()
    };
    assert!(!one.satisfies(Requires::MultiplePanes));
    assert!(!one.satisfies(Requires::MultipleTabs));
    assert!(!one.satisfies(Requires::MultipleWindows));
    assert!(!one.satisfies(Requires::SessionAttached));
    assert!(one.satisfies(Requires::Tab(1)));
    assert!(!one.satisfies(Requires::Tab(2)));
    assert!(!one.satisfies(Requires::Tab(0)));
    assert!(one.satisfies(Requires::Window(1)));
    assert!(!one.satisfies(Requires::Window(2)));
    assert!(!one.satisfies(Requires::Arrangements));
    assert!(one.satisfies(Requires::TabMovable));
    assert!(
        !one.satisfies(Requires::TabMovableAway),
        "nothing left behind"
    );

    let busy = MenuState {
        multiple_panes: true,
        tab_count: 3,
        windows: vec![(1, String::new()), (2, String::new())],
        session_attached: true,
        mux_attached: true,
        move_block: Some(MoveBlock::Attached),
        arrangements: vec![(uuid::Uuid::new_v4(), "work".into())],
        ..MenuState::default()
    };
    assert!(busy.satisfies(Requires::MultiplePanes));
    assert!(busy.satisfies(Requires::MultipleTabs));
    assert!(busy.satisfies(Requires::MultipleWindows));
    assert!(busy.satisfies(Requires::SessionAttached));
    assert!(busy.satisfies(Requires::MuxAttached));
    assert!(busy.satisfies(Requires::Tab(3)));
    assert!(busy.satisfies(Requires::Arrangements));
    assert!(!busy.satisfies(Requires::TabMovable));
    assert!(!busy.satisfies(Requires::TabMovableAway));
}

/// MN2: pane items are disabled with one pane and enabled with two.
#[test]
fn pane_items_need_a_second_pane() {
    let mut state = MenuState::default();
    let next = spec("next_pane");
    assert!(!state.enabled(&next));
    state.multiple_panes = true;
    assert!(state.enabled(&next));
}

/// MN2: Tab N carries the tab title and is disabled past the tab count.
#[test]
fn tab_items_show_titles() {
    let state = MenuState {
        tab_count: 2,
        tab_titles: vec!["build".into(), "  ".into()],
        ..MenuState::default()
    };
    assert_eq!(state.label(&spec("switch_to_tab_1")), "Tab 1: build");
    assert_eq!(
        state.label(&spec("switch_to_tab_2")),
        "Tab 2",
        "blank title"
    );
    assert_eq!(state.label(&spec("switch_to_tab_3")), "Tab 3");
    assert!(!state.enabled(&spec("switch_to_tab_3")));

    let long = MenuState {
        tab_count: 1,
        tab_titles: vec!["x".repeat(80)],
        ..MenuState::default()
    };
    let label = long.label(&spec("switch_to_tab_1"));
    assert!(
        label.ends_with('…') && label.chars().count() < 60,
        "{label}"
    );
}

/// MN2: move-to-window for an attached tab is disabled with the reason in
/// the label.
#[test]
fn an_attached_tab_cannot_move_and_says_why() {
    let state = MenuState {
        tab_count: 2,
        move_block: Some(MoveBlock::Attached),
        ..MenuState::default()
    };
    let item = spec("move_tab_to_window_picker");
    assert!(!state.enabled(&item));
    assert_eq!(state.label(&item), "Move Tab to Window... (attached tab)");

    let local = MenuState {
        tab_count: 2,
        ..MenuState::default()
    };
    assert!(local.enabled(&item));
    assert_eq!(local.label(&item), "Move Tab to Window...");
}

/// Window N carries the window's active tab title.
#[test]
fn window_items_show_titles() {
    let state = MenuState {
        windows: vec![(1, "build".into()), (3, String::new())],
        ..MenuState::default()
    };
    assert_eq!(state.label(&spec("switch_to_window_1")), "Window 1: build");
    assert_eq!(state.label(&spec("switch_to_window_3")), "Window 3");
    assert!(!state.enabled(&spec("switch_to_window_2")));
}

/// B61 parity: while a dialog is open only the commands the modal guard
/// lets through stay enabled.
#[test]
fn a_dialog_disables_what_its_key_guard_blocks() {
    let state = MenuState {
        modal_open: true,
        tab_count: 2,
        ..MenuState::default()
    };
    for blocked in ["new_tab", "split_right", "clear_scrollback", "close_pane"] {
        assert!(!state.enabled(&spec(blocked)), "{blocked} must be disabled");
    }
    for allowed in ["copy", "paste", "select_all", "minimize", "quit"] {
        assert!(state.enabled(&spec(allowed)), "{allowed} must stay enabled");
    }
    assert!(!state.dynamic_enabled());
}

#[test]
fn the_key_block_allowlist_matches_the_modal_guard_keys() {
    let f1 = Accelerator::new(Modifiers::empty(), Code::F1);
    let cmd_t = Accelerator::new(Modifiers::META, Code::KeyT);
    let action = MenuAction::Action("toggle_help");
    assert!(passes_key_block(&action, Some(&f1)));
    assert!(!passes_key_block(&action, Some(&cmd_t)));
    assert!(!passes_key_block(&action, None));
    assert!(passes_key_block(&MenuAction::Copy, None));
}

/// The macOS capture rebuild keeps the guard's chords and the open panel's
/// own toggle, and drops everything else.
#[test]
fn releasing_captured_accelerators_keeps_only_the_guard_chords() {
    let mut sections = menu_model(true);
    let open = BTreeSet::from(["toggle_command_palette"]);
    release_captured_accelerators(&mut sections, &open);
    for spec in all_items(&sections) {
        if let Some(accelerator) = &spec.accelerator {
            let open_toggle = matches!(spec.action, MenuAction::Action(id) if open.contains(id));
            assert!(
                passes_key_block(&spec.action, Some(accelerator)) || open_toggle,
                "{} kept {accelerator:?} while the keyboard is captured",
                spec.id
            );
        }
    }
    let kept: Vec<&str> = all_items(&sections)
        .into_iter()
        .filter(|s| s.accelerator.is_some())
        .map(|s| s.id)
        .collect();
    for id in [
        "copy",
        "paste",
        "select_all",
        "toggle_help",
        "toggle_fps_overlay",
        "toggle_command_palette",
    ] {
        assert!(kept.contains(&id), "{id} lost its chord: {kept:?}");
    }
    assert!(!kept.contains(&"new_tab"));
    assert!(!kept.contains(&"toggle_tree_picker"));
}

/// An open panel's toggle stays enabled behind the modal guard, so its
/// chord can close it; another panel's does not.
#[test]
fn an_open_panel_can_be_closed_from_the_menu() {
    let state = MenuState {
        modal_open: true,
        open_toggles: BTreeSet::from(["toggle_command_palette"]),
        ..MenuState::default()
    };
    assert!(state.enabled(&spec("toggle_command_palette")));
    assert!(!state.enabled(&spec("toggle_tree_picker")));
}

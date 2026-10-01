//! The Session, Profiles, Window, and Help menus (UX.md 21.2).

use super::actions::MenuAction;
use super::model::{MenuEntry, MenuItemSpec, submenu};
use super::state::{Check, Requires};
use muda::accelerator::{Accelerator, Code, Modifiers};

/// Registry ids of the Tab 1-9 items.
const TAB_ACTIONS: [&str; 9] = [
    "switch_to_tab_1",
    "switch_to_tab_2",
    "switch_to_tab_3",
    "switch_to_tab_4",
    "switch_to_tab_5",
    "switch_to_tab_6",
    "switch_to_tab_7",
    "switch_to_tab_8",
    "switch_to_tab_9",
];

/// Static labels of the Tab 1-9 items (the state swaps in the title).
const TAB_LABELS: [&str; 9] = [
    "Tab 1", "Tab 2", "Tab 3", "Tab 4", "Tab 5", "Tab 6", "Tab 7", "Tab 8", "Tab 9",
];

/// Menu ids and static labels of the Window 1-9 items.
const WINDOW_ITEMS: [(&str, &str); 9] = [
    ("switch_to_window_1", "Window 1"),
    ("switch_to_window_2", "Window 2"),
    ("switch_to_window_3", "Window 3"),
    ("switch_to_window_4", "Window 4"),
    ("switch_to_window_5", "Window 5"),
    ("switch_to_window_6", "Window 6"),
    ("switch_to_window_7", "Window 7"),
    ("switch_to_window_8", "Window 8"),
    ("switch_to_window_9", "Window 9"),
];

/// Session (iTerm2's per-pane menu): rename, restart, move, record output,
/// and the active tab's profile (UX.md PR5/PR6).
pub(super) fn session() -> Vec<MenuEntry> {
    vec![
        MenuItemSpec::action("rename_pane", "Rename Pane...").into(),
        MenuItemSpec::action("rename_tab", "Rename Tab...").into(),
        MenuItemSpec::action("restart_pane", "Restart Pane").into(),
        MenuEntry::Separator,
        MenuItemSpec::action("promote_pane_to_tab", "Move Pane to New Tab")
            .when(Requires::MultiplePanes)
            .into(),
        MenuEntry::Separator,
        MenuItemSpec::action("toggle_session_logging", "Record Output to File")
            .toggle(Check::OutputRecording)
            .into(),
        MenuEntry::Separator,
        MenuItemSpec::action("edit_tab_profile", "Edit Tab's Profile...").into(),
        MenuItemSpec::action("toggle_tab_profile_pin", "Pin Profile")
            .toggle(Check::TabProfilePinned)
            .into(),
    ]
}

/// Profiles: Open Profiles (iTerm2's launcher), one item per profile, and
/// profile management.
pub(super) fn profiles() -> Vec<MenuEntry> {
    vec![
        MenuItemSpec::action("toggle_profile_drawer", "Open Profiles...").into(),
        MenuEntry::Separator,
        MenuEntry::Profiles,
        MenuItemSpec::action("manage_profiles", "Manage Profiles...").into(),
        MenuItemSpec::action("reload_dynamic_profiles", "Reload Dynamic Profiles").into(),
    ]
}

/// Window: window state, arrangements, the Tab and Pane navigation
/// submenus, and window switching. On macOS AppKit appends the open-window
/// list (this section is registered as NSApp's Window menu).
pub(super) fn window() -> Vec<MenuEntry> {
    let minimize_accel = if cfg!(target_os = "macos") {
        Some(Accelerator::new(Modifiers::META, Code::KeyM))
    } else {
        None
    };
    let mut select_window = vec![
        MenuItemSpec::new("next_window", "Next Window", MenuAction::CycleWindow(1))
            .when(Requires::MultipleWindows)
            .into(),
        MenuItemSpec::new(
            "prev_window",
            "Previous Window",
            MenuAction::CycleWindow(-1),
        )
        .when(Requires::MultipleWindows)
        .into(),
        MenuEntry::Separator,
    ];
    for (index, (id, label)) in WINDOW_ITEMS.iter().enumerate() {
        let n = index + 1;
        select_window.push(
            MenuItemSpec::new(id, label, MenuAction::FocusWindowNumber(n))
                .when(Requires::Window(n))
                .into(),
        );
    }
    let mut entries = vec![
        MenuItemSpec::new("minimize", "Minimize", MenuAction::Minimize)
            .accel(minimize_accel)
            .into(),
        MenuItemSpec::new("zoom", "Zoom", MenuAction::Zoom).into(),
        MenuEntry::Separator,
        submenu(
            "arrangements",
            "Arrangements",
            Requires::Always,
            vec![
                MenuItemSpec::new(
                    "save_arrangement",
                    "Save or Manage Arrangements...",
                    MenuAction::SaveArrangement,
                )
                .into(),
                submenu(
                    "restore_arrangement",
                    "Restore",
                    Requires::Arrangements,
                    vec![MenuEntry::Arrangements],
                ),
            ],
        ),
        MenuEntry::Separator,
        submenu("tab_submenu", "Tab", Requires::Always, tab_entries()),
        submenu("pane_submenu", "Pane", Requires::Always, pane_entries()),
        MenuEntry::Separator,
        MenuItemSpec::action("toggle_always_on_top", "Always on Top")
            .toggle(Check::AlwaysOnTop)
            .into(),
        MenuEntry::Separator,
        submenu(
            "select_window",
            "Select Window",
            Requires::Always,
            select_window,
        ),
    ];
    if cfg!(target_os = "macos") {
        entries.push(MenuEntry::Separator);
        entries.push(MenuEntry::BringAllToFront);
    }
    entries
}

/// Window › Tab: tab navigation, reordering, moving, and Tab 1-9 with the
/// tab titles.
fn tab_entries() -> Vec<MenuEntry> {
    let mut entries = vec![
        MenuItemSpec::action("next_tab", "Next Tab")
            .when(Requires::MultipleTabs)
            .into(),
        MenuItemSpec::action("prev_tab", "Previous Tab")
            .when(Requires::MultipleTabs)
            .into(),
        MenuItemSpec::action("last_tab", "Last-Used Tab")
            .when(Requires::MultipleTabs)
            .into(),
        MenuItemSpec::action("go_to_last_tab", "Rightmost Tab")
            .when(Requires::MultipleTabs)
            .into(),
        MenuEntry::Separator,
        MenuItemSpec::action("move_tab_left", "Move Tab Left")
            .when(Requires::MultipleTabs)
            .into(),
        MenuItemSpec::action("move_tab_right", "Move Tab Right")
            .when(Requires::MultipleTabs)
            .into(),
        MenuItemSpec::action("move_tab_to_new_window", "Move Tab to New Window")
            .when(Requires::TabMovableAway)
            .into(),
        MenuItemSpec::action("move_tab_to_window_picker", "Move Tab to Window...")
            .when(Requires::TabMovable)
            .into(),
        MenuItemSpec::action("demote_tab_to_pane", "Demote Tab to Pane...")
            .when(Requires::MultipleTabs)
            .into(),
        MenuEntry::Separator,
    ];
    for (index, (id, label)) in TAB_ACTIONS.iter().zip(TAB_LABELS).enumerate() {
        entries.push(
            MenuItemSpec::action(id, label)
                .when(Requires::Tab(index + 1))
                .into(),
        );
    }
    entries
}

/// Window › Pane: focus, resize, swap, layout. Every item needs a second
/// pane (UX.md MN2).
fn pane_entries() -> Vec<MenuEntry> {
    let multi = |id: &'static str, label: &'static str| -> MenuEntry {
        MenuItemSpec::action(id, label)
            .when(Requires::MultiplePanes)
            .into()
    };
    vec![
        multi("navigate_pane_left", "Select Pane Left"),
        multi("navigate_pane_right", "Select Pane Right"),
        multi("navigate_pane_up", "Select Pane Above"),
        multi("navigate_pane_down", "Select Pane Below"),
        MenuEntry::Separator,
        multi("next_pane", "Next Pane"),
        multi("prev_pane", "Previous Pane"),
        multi("last_pane", "Last-Focused Pane"),
        multi("select_pane_hint", "Select Pane by Letter"),
        MenuEntry::Separator,
        submenu(
            "resize_pane",
            "Resize",
            Requires::MultiplePanes,
            vec![
                multi("resize_pane_left", "Left"),
                multi("resize_pane_right", "Right"),
                multi("resize_pane_up", "Up"),
                multi("resize_pane_down", "Down"),
                MenuEntry::Separator,
                multi("enter_resize_mode", "With Arrow Keys"),
            ],
        ),
        submenu(
            "swap_pane",
            "Swap",
            Requires::MultiplePanes,
            vec![
                multi("swap_pane_left", "Left"),
                multi("swap_pane_right", "Right"),
                multi("swap_pane_up", "Up"),
                multi("swap_pane_down", "Down"),
            ],
        ),
        multi("equalize_panes", "Equalize Panes"),
        multi("cycle_layout", "Cycle Layout Presets"),
    ]
}

/// Help: shortcuts, documentation, and About off macOS (the application
/// menu carries it there).
pub(super) fn help(has_native_app_menu: bool) -> Vec<MenuEntry> {
    let mut entries = vec![
        MenuItemSpec::action("toggle_help", "Keyboard Shortcuts").into(),
        MenuItemSpec::new("open_docs", "par-term Help", MenuAction::OpenDocs).into(),
        MenuItemSpec::action("reload_config", "Reload Configuration").into(),
    ];
    if !has_native_app_menu {
        entries.push(MenuEntry::Separator);
        entries.push(MenuItemSpec::new("about", "About par-term", MenuAction::About).into());
    }
    entries
}

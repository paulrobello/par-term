//! Declarative description of par-term's menu.
//!
//! This is the single source of truth for the menu's contents. Two renderers
//! consume it:
//!
//! - [`super::MenuManager::new`] walks it to build the [`muda::Menu`] that macOS
//!   and Windows attach natively.
//! - [`super::egui_menu::AppMenuUi`] walks the same model to draw the in-app
//!   menu on platforms that cannot attach a native menu bar (Linux/BSD, where
//!   muda needs a `gtk::Window` that winit never creates — see `super::linux`).
//!
//! Neither renderer owns a list of commands, so the two cannot drift apart.

use super::actions::MenuAction;
use muda::accelerator::{Accelerator, Code, Modifiers};

/// Title of the Help section.
///
/// `MenuManager` inserts the macOS-only native Window menu immediately before
/// this section, following the platform convention of Window preceding Help.
pub const HELP_SECTION_TITLE: &str = "Help";

/// A single activatable menu command.
pub struct MenuItemSpec {
    /// Stable muda menu id. Also used as the egui widget id salt.
    pub id: &'static str,
    /// Human-readable label.
    pub label: &'static str,
    /// Keyboard accelerator, if the command has one.
    pub accelerator: Option<Accelerator>,
    /// Action dispatched when the item is activated.
    pub action: MenuAction,
}

/// One entry inside a menu section.
pub enum MenuEntry {
    /// A command.
    Item(MenuItemSpec),
    /// A horizontal rule.
    Separator,
    /// Insertion point for one entry per configured profile.
    ///
    /// The entries are generated at render time from the live
    /// [`ProfileManager`] by [`profile_entries`], so both renderers stay in
    /// sync with profile edits without duplicating the mapping.
    Profiles,
}

/// A top-level menu (File, Tab, Edit, …).
pub struct MenuSection {
    /// Title shown in the menu bar.
    pub title: &'static str,
    /// Entries in display order.
    pub entries: Vec<MenuEntry>,
}

/// Build the menu model for the current platform's native menu.
///
/// macOS carries Quit and Preferences in the separate application menu built by
/// [`super::macos::build_app_menu`], so they are omitted from File/Edit there.
pub fn platform_menu_model() -> Vec<MenuSection> {
    menu_model(cfg!(target_os = "macos"))
}

/// Build the menu model.
///
/// `has_native_app_menu` is true when the platform provides a separate
/// application menu that already carries Quit and Preferences (macOS). When it
/// is false those two commands are folded into File and Edit, which is what
/// Windows and Linux expect — and what the in-app egui menu always needs, since
/// it is the only menu wherever it is drawn.
pub fn menu_model(has_native_app_menu: bool) -> Vec<MenuSection> {
    // Platform-specific modifier keys
    // macOS: Cmd (META) is safe — it's separate from Ctrl used by terminal control codes
    // Windows/Linux: Use Ctrl+Shift to avoid conflicts with terminal control codes
    // (Ctrl+C=SIGINT, Ctrl+D=EOF, Ctrl+W=delete-word, Ctrl+V=literal-next, etc.)
    #[cfg(target_os = "macos")]
    let cmd_or_ctrl = Modifiers::META;
    #[cfg(not(target_os = "macos"))]
    let cmd_or_ctrl = Modifiers::CONTROL | Modifiers::SHIFT;

    // For items that already include Shift (same on all platforms)
    #[cfg(target_os = "macos")]
    let cmd_or_ctrl_shift = Modifiers::META | Modifiers::SHIFT;
    #[cfg(not(target_os = "macos"))]
    let cmd_or_ctrl_shift = Modifiers::CONTROL | Modifiers::SHIFT;

    // Tab number switching: Cmd+N (macOS) / Alt+N (Windows/Linux)
    #[cfg(target_os = "macos")]
    let tab_switch_mod = Modifiers::META;
    #[cfg(not(target_os = "macos"))]
    let tab_switch_mod = Modifiers::ALT;

    let accel = |mods: Modifiers, code: Code| Some(Accelerator::new(Some(mods), code));

    let mut file = vec![
        item(
            "new_window",
            "New Window",
            accel(cmd_or_ctrl, Code::KeyN),
            MenuAction::NewWindow,
        ),
        // Smart close: closes tab if multiple, window if single
        item(
            "close_window",
            "Close",
            accel(cmd_or_ctrl, Code::KeyW),
            MenuAction::CloseWindow,
        ),
        MenuEntry::Separator,
    ];
    if !has_native_app_menu {
        file.push(item(
            "quit",
            "Quit",
            accel(cmd_or_ctrl, Code::KeyQ),
            MenuAction::Quit,
        ));
    }

    let mut tab = vec![
        item(
            "new_tab",
            "New Tab",
            accel(cmd_or_ctrl, Code::KeyT),
            MenuAction::NewTab,
        ),
        // No accelerator: nothing in `Config::default().keybindings` binds
        // `duplicate_tab`, and an accelerator here would be advertised by the
        // in-app menu on platforms where only a keybinding can dispatch it.
        item(
            "duplicate_tab",
            "Duplicate Tab",
            None,
            MenuAction::DuplicateTab,
        ),
        // No accelerator: same as Close in the File menu (smart close)
        item("close_tab", "Close Tab", None, MenuAction::CloseTab),
        MenuEntry::Separator,
        item(
            "next_tab",
            "Next Tab",
            accel(cmd_or_ctrl_shift, Code::BracketRight),
            MenuAction::NextTab,
        ),
        item(
            "prev_tab",
            "Previous Tab",
            accel(cmd_or_ctrl_shift, Code::BracketLeft),
            MenuAction::PreviousTab,
        ),
        MenuEntry::Separator,
    ];
    for (index, (id, label, code)) in TAB_SWITCH_ITEMS.iter().enumerate() {
        tab.push(item(
            id,
            label,
            accel(tab_switch_mod, *code),
            MenuAction::SwitchToTab(index + 1),
        ));
    }

    let mut edit = vec![
        // Copy/Paste/Select All: Cmd+C/V/A (macOS) / Ctrl+Shift+C/V/A (other)
        item(
            "copy",
            "Copy",
            accel(cmd_or_ctrl, Code::KeyC),
            MenuAction::Copy,
        ),
        item(
            "paste",
            "Paste",
            accel(cmd_or_ctrl, Code::KeyV),
            MenuAction::Paste,
        ),
        item(
            "select_all",
            "Select All",
            accel(cmd_or_ctrl, Code::KeyA),
            MenuAction::SelectAll,
        ),
        MenuEntry::Separator,
        item(
            "clear_scrollback",
            "Clear Scrollback",
            accel(cmd_or_ctrl_shift, Code::KeyK),
            MenuAction::ClearScrollback,
        ),
        item(
            "clipboard_history",
            "Clipboard History",
            accel(cmd_or_ctrl_shift, Code::KeyH),
            MenuAction::ClipboardHistory,
        ),
    ];
    if !has_native_app_menu {
        // Preferences belongs in Edit on Windows and Linux.
        edit.push(MenuEntry::Separator);
        edit.push(item(
            "preferences",
            "Preferences...",
            accel(Modifiers::CONTROL | Modifiers::SHIFT, Code::Comma),
            MenuAction::OpenSettings,
        ));
    }

    vec![
        MenuSection {
            title: "File",
            entries: file,
        },
        MenuSection {
            title: "Tab",
            entries: tab,
        },
        MenuSection {
            title: "Profiles",
            entries: vec![
                item(
                    "manage_profiles",
                    "Manage Profiles...",
                    accel(cmd_or_ctrl_shift, Code::KeyP),
                    MenuAction::ManageProfiles,
                ),
                item(
                    "toggle_profile_drawer",
                    "Toggle Profile Drawer",
                    None,
                    MenuAction::ToggleProfileDrawer,
                ),
                MenuEntry::Separator,
                MenuEntry::Profiles,
            ],
        },
        MenuSection {
            title: "Edit",
            entries: edit,
        },
        MenuSection {
            title: "View",
            entries: vec![
                item(
                    "toggle_fullscreen",
                    "Toggle Fullscreen",
                    Some(Accelerator::new(None, Code::F11)),
                    MenuAction::ToggleFullscreen,
                ),
                item(
                    "maximize_vertically",
                    "Maximize Vertically",
                    accel(Modifiers::SHIFT, Code::F11),
                    MenuAction::MaximizeVertically,
                ),
                MenuEntry::Separator,
                item(
                    "increase_font",
                    "Increase Font Size",
                    accel(cmd_or_ctrl, Code::Equal),
                    MenuAction::IncreaseFontSize,
                ),
                item(
                    "decrease_font",
                    "Decrease Font Size",
                    accel(cmd_or_ctrl, Code::Minus),
                    MenuAction::DecreaseFontSize,
                ),
                item(
                    "reset_font",
                    "Reset Font Size",
                    accel(cmd_or_ctrl, Code::Digit0),
                    MenuAction::ResetFontSize,
                ),
                MenuEntry::Separator,
                item(
                    "fps_overlay",
                    "FPS Overlay",
                    Some(Accelerator::new(None, Code::F3)),
                    MenuAction::ToggleFpsOverlay,
                ),
                item(
                    "settings",
                    "Settings...",
                    Some(Accelerator::new(None, Code::F12)),
                    MenuAction::OpenSettings,
                ),
                MenuEntry::Separator,
                item(
                    "save_arrangement",
                    "Save Window Arrangement...",
                    None,
                    MenuAction::SaveArrangement,
                ),
            ],
        },
        MenuSection {
            title: "Shell",
            entries: vec![item(
                "install_remote_shell_integration",
                "Install Shell Integration on Remote Host...",
                None,
                MenuAction::InstallShellIntegrationRemote,
            )],
        },
        MenuSection {
            title: HELP_SECTION_TITLE,
            entries: vec![
                item(
                    "keyboard_shortcuts",
                    "Keyboard Shortcuts",
                    Some(Accelerator::new(None, Code::F1)),
                    MenuAction::ShowHelp,
                ),
                MenuEntry::Separator,
                item("about", "About par-term", None, MenuAction::About),
            ],
        },
    ]
}

/// Menu ids, labels and key codes for the Tab 1-9 switch items.
const TAB_SWITCH_ITEMS: [(&str, &str, Code); 9] = [
    ("tab_1", "Tab 1", Code::Digit1),
    ("tab_2", "Tab 2", Code::Digit2),
    ("tab_3", "Tab 3", Code::Digit3),
    ("tab_4", "Tab 4", Code::Digit4),
    ("tab_5", "Tab 5", Code::Digit5),
    ("tab_6", "Tab 6", Code::Digit6),
    ("tab_7", "Tab 7", Code::Digit7),
    ("tab_8", "Tab 8", Code::Digit8),
    ("tab_9", "Tab 9", Code::Digit9),
];

/// Shorthand for a command entry.
fn item(
    id: &'static str,
    label: &'static str,
    accelerator: Option<Accelerator>,
    action: MenuAction,
) -> MenuEntry {
    MenuEntry::Item(MenuItemSpec {
        id,
        label,
        accelerator,
        action,
    })
}

/// One dynamically generated profile entry.
pub struct ProfileEntry {
    /// Stable muda menu id.
    pub menu_id: String,
    /// Label as shown in the menu.
    pub label: String,
    /// Action dispatched when the entry is activated.
    pub action: MenuAction,
}

/// Expand [`MenuEntry::Profiles`] into one entry per configured profile.
///
/// Shared by the muda and egui renderers so both show the same profiles, with
/// the same labels, in the same order.
pub fn profile_entries<'a>(
    profiles: impl IntoIterator<Item = &'a crate::profile::Profile>,
) -> Vec<ProfileEntry> {
    profiles
        .into_iter()
        .map(|profile| ProfileEntry {
            menu_id: format!("profile_{}", profile.id),
            label: profile.display_label(),
            action: MenuAction::OpenProfile(profile.id),
        })
        .collect()
}

/// Render an accelerator the way a menu displays it, e.g. `⌘N` or `Ctrl+Shift+N`.
///
/// Derived from the same [`Accelerator`] the native menu registers, so the
/// in-app menu cannot advertise a shortcut the native menu does not have.
pub fn accelerator_label(accelerator: &Accelerator) -> String {
    let mut label = String::new();
    // `Accelerator::new` normalises META to SUPER, so only SUPER is ever set.
    // macOS renders modifiers as adjacent symbols; everywhere else they are
    // spelled out and joined with '+'.
    let named: [(Modifiers, &str, &str); 4] = [
        (Modifiers::CONTROL, "⌃", "Ctrl"),
        (Modifiers::ALT, "⌥", "Alt"),
        (Modifiers::SHIFT, "⇧", "Shift"),
        (Modifiers::SUPER, "⌘", "Super"),
    ];
    let mods = accelerator.modifiers();
    for (flag, symbol, word) in named {
        if mods.contains(flag) {
            if cfg!(target_os = "macos") {
                label.push_str(symbol);
            } else {
                label.push_str(word);
                label.push('+');
            }
        }
    }
    label.push_str(&code_label(accelerator.key()));
    label
}

/// Human-readable name for a key code (`KeyN` → `N`, `BracketLeft` → `[`).
fn code_label(code: Code) -> String {
    let raw = format!("{code:?}");
    match raw.as_str() {
        "Comma" => ",".to_string(),
        "Period" => ".".to_string(),
        "Equal" => "+".to_string(),
        "Minus" => "-".to_string(),
        "BracketLeft" => "[".to_string(),
        "BracketRight" => "]".to_string(),
        "Space" => "Space".to_string(),
        other => other
            .strip_prefix("Key")
            .or_else(|| other.strip_prefix("Digit"))
            .unwrap_or(other)
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn items(model: &[MenuSection]) -> Vec<&MenuItemSpec> {
        model
            .iter()
            .flat_map(|section| &section.entries)
            .filter_map(|entry| match entry {
                MenuEntry::Item(spec) => Some(spec),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn menu_ids_are_unique() {
        for has_native_app_menu in [false, true] {
            let model = menu_model(has_native_app_menu);
            let mut seen = HashSet::new();
            for spec in items(&model) {
                assert!(
                    seen.insert(spec.id),
                    "duplicate menu id {:?} (has_native_app_menu={has_native_app_menu})",
                    spec.id
                );
            }
        }
    }

    /// Without a native application menu the model must carry Quit and
    /// Preferences itself — this is exactly what Linux was missing.
    #[test]
    fn quit_and_preferences_present_without_native_app_menu() {
        let model = menu_model(false);
        let actions: Vec<MenuAction> = items(&model).iter().map(|spec| spec.action).collect();
        assert!(actions.contains(&MenuAction::Quit));
        assert!(actions.contains(&MenuAction::OpenSettings));
        assert!(actions.contains(&MenuAction::NewWindow));
        assert!(actions.contains(&MenuAction::CloseWindow));
        assert!(actions.contains(&MenuAction::SelectAll));
        assert!(actions.contains(&MenuAction::MaximizeVertically));
    }

    /// `MenuAction::DuplicateTab` was declared and handled but emitted by no
    /// menu item, which left `duplicate_tab` with a handler nothing could reach.
    #[test]
    fn duplicate_tab_is_reachable_from_the_menu() {
        for has_native_app_menu in [false, true] {
            let model = menu_model(has_native_app_menu);
            let actions: Vec<MenuAction> = items(&model).iter().map(|spec| spec.action).collect();
            assert!(
                actions.contains(&MenuAction::DuplicateTab),
                "no menu item emits DuplicateTab (has_native_app_menu={has_native_app_menu})"
            );
        }
    }

    /// macOS keeps Quit in the application menu, so File must not duplicate it.
    #[test]
    fn quit_absent_when_native_app_menu_owns_it() {
        let model = menu_model(true);
        let actions: Vec<MenuAction> = items(&model).iter().map(|spec| spec.action).collect();
        assert!(!actions.contains(&MenuAction::Quit));
    }

    /// The two variants must offer the same commands apart from the ones the
    /// native application menu owns.
    #[test]
    fn variants_differ_only_by_app_menu_items() {
        let with_app_menu: HashSet<&str> = items(&menu_model(true))
            .iter()
            .map(|spec| spec.id)
            .collect();
        let without: HashSet<&str> = items(&menu_model(false))
            .iter()
            .map(|spec| spec.id)
            .collect();
        let extra: Vec<&&str> = without.difference(&with_app_menu).collect();
        assert_eq!(extra.len(), 2, "unexpected difference: {extra:?}");
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

    /// The Profiles insertion point must exist exactly once.
    #[test]
    fn profiles_placeholder_appears_once() {
        let count = menu_model(false)
            .iter()
            .flat_map(|section| &section.entries)
            .filter(|entry| matches!(entry, MenuEntry::Profiles))
            .count();
        assert_eq!(count, 1);
    }

    /// Flatten the model to `Section/entry` lines, in order.
    fn outline(model: &[MenuSection]) -> Vec<String> {
        model
            .iter()
            .flat_map(|section| {
                section.entries.iter().map(move |entry| match entry {
                    MenuEntry::Item(spec) => {
                        format!("{}/{} = {:?}", section.title, spec.id, spec.label)
                    }
                    MenuEntry::Separator => format!("{}/---", section.title),
                    MenuEntry::Profiles => format!("{}/<profiles>", section.title),
                })
            })
            .collect()
    }

    /// The order-sensitive snapshot.
    ///
    /// The macOS and Windows menus are built by walking this model, and neither
    /// can be exercised from the other's CI. A reordered section, a dropped
    /// separator or a renamed label is invisible to every other test here, so
    /// this freezes the structure that shipped before the model existed. Update
    /// it deliberately when the menu changes.
    #[test]
    fn model_matches_the_shipped_menu_structure() {
        let expected_common = [
            "File/new_window = \"New Window\"",
            "File/close_window = \"Close\"",
            "File/---",
            "Tab/new_tab = \"New Tab\"",
            "Tab/duplicate_tab = \"Duplicate Tab\"",
            "Tab/close_tab = \"Close Tab\"",
            "Tab/---",
            "Tab/next_tab = \"Next Tab\"",
            "Tab/prev_tab = \"Previous Tab\"",
            "Tab/---",
            "Tab/tab_1 = \"Tab 1\"",
            "Tab/tab_2 = \"Tab 2\"",
            "Tab/tab_3 = \"Tab 3\"",
            "Tab/tab_4 = \"Tab 4\"",
            "Tab/tab_5 = \"Tab 5\"",
            "Tab/tab_6 = \"Tab 6\"",
            "Tab/tab_7 = \"Tab 7\"",
            "Tab/tab_8 = \"Tab 8\"",
            "Tab/tab_9 = \"Tab 9\"",
            "Profiles/manage_profiles = \"Manage Profiles...\"",
            "Profiles/toggle_profile_drawer = \"Toggle Profile Drawer\"",
            "Profiles/---",
            "Profiles/<profiles>",
            "Edit/copy = \"Copy\"",
            "Edit/paste = \"Paste\"",
            "Edit/select_all = \"Select All\"",
            "Edit/---",
            "Edit/clear_scrollback = \"Clear Scrollback\"",
            "Edit/clipboard_history = \"Clipboard History\"",
        ];
        let expected_tail = [
            "View/toggle_fullscreen = \"Toggle Fullscreen\"",
            "View/maximize_vertically = \"Maximize Vertically\"",
            "View/---",
            "View/increase_font = \"Increase Font Size\"",
            "View/decrease_font = \"Decrease Font Size\"",
            "View/reset_font = \"Reset Font Size\"",
            "View/---",
            "View/fps_overlay = \"FPS Overlay\"",
            "View/settings = \"Settings...\"",
            "View/---",
            "View/save_arrangement = \"Save Window Arrangement...\"",
            "Shell/install_remote_shell_integration = \
             \"Install Shell Integration on Remote Host...\"",
            "Help/keyboard_shortcuts = \"Keyboard Shortcuts\"",
            "Help/---",
            "Help/about = \"About par-term\"",
        ];

        // macOS: Quit and Preferences belong to the native application menu.
        let mut with_app_menu: Vec<&str> = expected_common.to_vec();
        with_app_menu.extend(expected_tail);
        assert_eq!(outline(&menu_model(true)), with_app_menu);

        // Everywhere else they are folded into File and Edit.
        let mut without: Vec<&str> = expected_common.to_vec();
        without.insert(3, "File/quit = \"Quit\"");
        without.push("Edit/---");
        without.push("Edit/preferences = \"Preferences...\"");
        without.extend(expected_tail);
        assert_eq!(outline(&menu_model(false)), without);
    }

    /// Which commands carry a keyboard accelerator is part of the contract the
    /// in-app menu advertises, and is platform-independent even though the
    /// modifiers are not.
    #[test]
    fn the_same_commands_carry_accelerators() {
        let accelerated: Vec<&str> = items(&menu_model(false))
            .iter()
            .filter(|spec| spec.accelerator.is_some())
            .map(|spec| spec.id)
            .collect();
        assert_eq!(
            accelerated,
            vec![
                "new_window",
                "close_window",
                "quit",
                "new_tab",
                "next_tab",
                "prev_tab",
                "tab_1",
                "tab_2",
                "tab_3",
                "tab_4",
                "tab_5",
                "tab_6",
                "tab_7",
                "tab_8",
                "tab_9",
                "manage_profiles",
                "copy",
                "paste",
                "select_all",
                "clear_scrollback",
                "clipboard_history",
                "preferences",
                "toggle_fullscreen",
                "maximize_vertically",
                "increase_font",
                "decrease_font",
                "reset_font",
                "fps_overlay",
                "settings",
                "keyboard_shortcuts",
            ]
        );
    }

    #[test]
    fn accelerator_labels_are_readable() {
        let plain = Accelerator::new(None, Code::F11);
        assert_eq!(accelerator_label(&plain), "F11");

        let bracket = Accelerator::new(Some(Modifiers::SHIFT), Code::BracketRight);
        let label = accelerator_label(&bracket);
        assert!(label.ends_with(']'), "unexpected label {label:?}");

        let digit = Accelerator::new(Some(Modifiers::ALT), Code::Digit1);
        assert!(accelerator_label(&digit).ends_with('1'));
    }
}

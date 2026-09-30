//! Declarative description of par-term's menu.
//!
//! This is the single source of truth for the menu's contents. Two renderers
//! consume it:
//!
//! - [`super::MenuManager::new_with`] walks it to build the [`muda::Menu`] that macOS
//!   and Windows attach natively.
//! - [`super::egui_menu::AppMenuUi`] walks the same model to draw the in-app
//!   menu on platforms that cannot attach a native menu bar (Linux/BSD, where
//!   muda needs a `gtk::Window` that winit never creates — see `super::linux`).
//!
//! Neither renderer owns a list of commands, so the two cannot drift apart.

use super::actions::MenuAction;
use muda::accelerator::{Accelerator, Code, Modifiers};
use par_term_config::KeyBinding;

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
    platform_menu_model_with(&par_term_config::Config::default().keybindings)
}

/// Build the menu model for the current platform's native menu, sourcing
/// accelerators from explicit keybindings.
pub fn platform_menu_model_with(keybindings: &[KeyBinding]) -> Vec<MenuSection> {
    menu_model_with(cfg!(target_os = "macos"), keybindings)
}

/// Build the menu model.
///
/// `has_native_app_menu` is true when the platform provides a separate
/// application menu that already carries Quit and Preferences (macOS). When it
/// is false those two commands are folded into File and Edit, which is what
/// Windows and Linux expect — and what the in-app egui menu always needs, since
/// it is the only menu wherever it is drawn.
pub fn menu_model(has_native_app_menu: bool) -> Vec<MenuSection> {
    menu_model_with(
        has_native_app_menu,
        &par_term_config::Config::default().keybindings,
    )
}

/// Build the menu model, sourcing every accelerator from `keybindings` (the
/// registry) and keeping the hardcoded chords only as the fallback for
/// actions with no binding — see [`super::registry_accel`].
pub fn menu_model_with(has_native_app_menu: bool, keybindings: &[KeyBinding]) -> Vec<MenuSection> {
    let mut sections = hardcoded_menu_model(has_native_app_menu);
    super::registry_accel::apply_registry_accelerators(&mut sections, keybindings);
    sections
}

/// The menu model with its hardcoded fallback accelerators, before the
/// registry is applied.
fn hardcoded_menu_model(has_native_app_menu: bool) -> Vec<MenuSection> {
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

    // Close Tab: Cmd+Opt+W (iTerm2) / Ctrl+Alt+W (K1 family)
    #[cfg(target_os = "macos")]
    let close_tab_mods = Modifiers::META | Modifiers::ALT;
    #[cfg(not(target_os = "macos"))]
    let close_tab_mods = Modifiers::CONTROL | Modifiers::ALT;

    // Move tab: Cmd+Opt+Shift+[ / ] (iTerm2) / Ctrl+Shift+Left/Right
    #[cfg(target_os = "macos")]
    let (move_tab_mods, move_tab_left_key, move_tab_right_key) = (
        Modifiers::META | Modifiers::ALT | Modifiers::SHIFT,
        Code::BracketLeft,
        Code::BracketRight,
    );
    #[cfg(not(target_os = "macos"))]
    let (move_tab_mods, move_tab_left_key, move_tab_right_key) =
        (cmd_or_ctrl_shift, Code::ArrowLeft, Code::ArrowRight);

    let accel = |mods: Modifiers, code: Code| Some(Accelerator::new(mods, code));

    #[cfg(target_os = "macos")]
    let profile_drawer_accel = accel(cmd_or_ctrl, Code::KeyO);
    #[cfg(not(target_os = "macos"))]
    let profile_drawer_accel = None;

    let mut file = vec![
        item(
            "new_window",
            "New Window",
            accel(cmd_or_ctrl, Code::KeyN),
            MenuAction::NewWindow,
        ),
        // iTerm2's Close (UX.md I15): the focused pane, cascading to the tab
        // and then the window. The id is kept so the native item is stable.
        item(
            "close_window",
            "Close",
            accel(cmd_or_ctrl, Code::KeyW),
            MenuAction::ClosePane,
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
        // Matches the `duplicate_tab` default in `Config::default().keybindings`,
        // which is what dispatches this on Linux (no native menu there, so the
        // in-app menu only advertises the chord — it does not register it).
        item(
            "duplicate_tab",
            "Duplicate Tab",
            accel(cmd_or_ctrl_shift, Code::KeyJ),
            MenuAction::DuplicateTab,
        ),
        // iTerm2's "Close All Panes in Tab" (UX.md I15): Cmd+Opt+W, and the
        // K1 family's Ctrl+Alt+W elsewhere.
        item(
            "close_tab",
            "Close Tab",
            accel(close_tab_mods, Code::KeyW),
            MenuAction::CloseTab,
        ),
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
        // Reordering is dispatched by the `move_tab_left`/`move_tab_right`
        // registry defaults. The accelerators here name the primary chords —
        // iTerm2's Cmd+Opt+Shift+[ / ] on macOS (UX.md I9), the arrows
        // elsewhere — and the settings window's `AVAILABLE_ACTIONS` advertises
        // them; `key_handler::chord_tests` checks all three agree.
        item(
            "move_tab_left",
            "Move Tab Left",
            accel(move_tab_mods, move_tab_left_key),
            MenuAction::MoveTabLeft,
        ),
        item(
            "move_tab_right",
            "Move Tab Right",
            accel(move_tab_mods, move_tab_right_key),
            MenuAction::MoveTabRight,
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
            title: "Pane",
            entries: super::model_pane_session::pane_entries(),
        },
        MenuSection {
            title: "Session",
            entries: super::model_pane_session::session_entries(),
        },
        MenuSection {
            title: "Profiles",
            entries: vec![
                // `Manage Profiles...` is a configuration dialog, also reachable
                // from Settings, and it has no `AVAILABLE_ACTIONS` row, so a
                // native accelerator on it burned Cmd/Ctrl+Shift+P for a chord
                // no user could rebind.
                item(
                    "manage_profiles",
                    "Manage Profiles...",
                    None,
                    MenuAction::ManageProfiles,
                ),
                // The drawer is iTerm2's Open Profiles, Cmd+O on macOS (UX.md
                // I36). Cmd/Ctrl+Shift+P belongs to the command palette (D2),
                // so this item must not carry it: a native accelerator would
                // eat the chord before the registry runs. Off macOS the
                // drawer ships unbound (Ctrl+Shift+O is split down there).
                item(
                    "toggle_profile_drawer",
                    "Toggle Profile Drawer",
                    profile_drawer_accel,
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
                    Some(Accelerator::new(Modifiers::empty(), Code::F11)),
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
                    Some(Accelerator::new(Modifiers::empty(), Code::F3)),
                    MenuAction::ToggleFpsOverlay,
                ),
                item(
                    "settings",
                    "Settings...",
                    Some(Accelerator::new(Modifiers::empty(), Code::F12)),
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
                    Some(Accelerator::new(Modifiers::empty(), Code::F1)),
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
    // muda 0.20 stores META verbatim (the old META→SUPER normalisation is gone;
    // SUPER is a separate legacy bit nothing produces any more), so the table
    // keys on META. macOS renders modifiers as adjacent symbols; everywhere
    // else they are spelled out and joined with '+'.
    let named: [(Modifiers, &str, &str); 4] = [
        (Modifiers::CONTROL, "⌃", "Ctrl"),
        (Modifiers::ALT, "⌥", "Alt"),
        (Modifiers::SHIFT, "⇧", "Shift"),
        (Modifiers::META, "⌘", "Super"),
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
        // Without these the in-app menu would advertise "ArrowLeft"; "Left" is
        // also what the settings window's keybinding table prints.
        "ArrowLeft" => "Left".to_string(),
        "ArrowRight" => "Right".to_string(),
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
        assert!(actions.contains(&MenuAction::ClosePane));
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

    /// `MoveTabLeft`/`MoveTabRight` were declared and handled but emitted by no
    /// menu item, so tab reordering existed only as an unadvertised chord.
    #[test]
    fn tab_reordering_is_reachable_from_the_menu() {
        for has_native_app_menu in [false, true] {
            let model = menu_model(has_native_app_menu);
            let actions: Vec<MenuAction> = items(&model).iter().map(|spec| spec.action).collect();
            for expected in [MenuAction::MoveTabLeft, MenuAction::MoveTabRight] {
                assert!(
                    actions.contains(&expected),
                    "no menu item emits {expected:?} (has_native_app_menu={has_native_app_menu})"
                );
            }
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
            "Tab/move_tab_left = \"Move Tab Left\"",
            "Tab/move_tab_right = \"Move Tab Right\"",
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
            "Pane/pane_split_right = \"Split Right\"",
            "Pane/pane_split_down = \"Split Down\"",
            "Pane/---",
            "Pane/pane_toggle_pane_zoom = \"Zoom Pane\"",
            "Pane/pane_equalize_panes = \"Equalize Panes\"",
            "Pane/---",
            "Pane/pane_next_pane = \"Next Pane\"",
            "Pane/pane_prev_pane = \"Previous Pane\"",
            "Pane/pane_last_pane = \"Last-Focused Pane\"",
            "Pane/pane_select_pane_hint = \"Select Pane by Letter\"",
            "Pane/---",
            "Pane/pane_rename_pane = \"Rename Pane...\"",
            "Pane/pane_restart_pane = \"Restart Pane Process\"",
            "Pane/pane_promote_pane_to_tab = \"Move Pane to New Tab\"",
            "Pane/---",
            "Pane/pane_toggle_broadcast_input = \"Broadcast Input to This Tab\"",
            "Session/session_toggle_session_picker = \"Sessions...\"",
            "Session/session_new_mux_session = \"New par-mux Session\"",
            "Session/session_detach = \"Detach\"",
            "Session/---",
            "Session/session_focus_next_attention_agent = \"Next Agent Needing Attention\"",
            "Session/session_toggle_tree_picker = \"Open Quickly...\"",
            "Session/---",
            "Session/session_rename_tab = \"Rename Tab...\"",
            "Session/session_last_tab = \"Last-Used Tab\"",
            "Session/session_move_tab_to_window_picker = \"Move Tab to Window...\"",
            "Session/---",
            "Session/session_next_window = \"Next Window\"",
            "Session/session_prev_window = \"Previous Window\"",
            "Session/session_close_window = \"Close Window\"",
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
    /// in-app menu advertises. It is platform-independent except for the
    /// profile drawer, which carries iTerm2's Cmd+O on macOS and ships
    /// unbound elsewhere (Ctrl+Shift+O is split down there, UX.md K12).
    #[test]
    fn the_same_commands_carry_accelerators() {
        let accelerated: Vec<&str> = items(&menu_model(false))
            .iter()
            .filter(|spec| spec.accelerator.is_some())
            .map(|spec| spec.id)
            .collect();
        let mut expected = vec![
            "new_window",
            "close_window",
            "quit",
            "new_tab",
            "duplicate_tab",
            "close_tab",
            "next_tab",
            "prev_tab",
            "move_tab_left",
            "move_tab_right",
            "tab_1",
            "tab_2",
            "tab_3",
            "tab_4",
            "tab_5",
            "tab_6",
            "tab_7",
            "tab_8",
            "tab_9",
            "pane_split_right",
            "pane_split_down",
            "pane_toggle_pane_zoom",
            "pane_equalize_panes",
            "pane_next_pane",
            "pane_prev_pane",
            "pane_select_pane_hint",
            "pane_toggle_broadcast_input",
            "session_toggle_session_picker",
            "session_focus_next_attention_agent",
            "session_toggle_tree_picker",
            "toggle_profile_drawer",
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
        ];
        if !cfg!(target_os = "macos") {
            expected.retain(|id| *id != "toggle_profile_drawer");
        }
        assert_eq!(accelerated, expected);
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
        let arrow = Accelerator::new(Modifiers::SHIFT, Code::ArrowLeft);
        assert!(
            accelerator_label(&arrow).ends_with("Left"),
            "unexpected label {:?}",
            accelerator_label(&arrow)
        );
        assert!(!accelerator_label(&arrow).contains("Arrow"));
    }
}

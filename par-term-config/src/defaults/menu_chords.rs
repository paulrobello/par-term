//! Menu-advertised chords as registry defaults (UX.md K2).
//!
//! Every accelerator the menu model ships with is also a default keybinding,
//! so menu and registry cannot drift: `menu::registry_accel` re-reads the
//! chord from these bindings when the menu model is built, and the menu tests
//! assert the two agree. The chords follow UX.md 3.3a (iTerm2 alignment); a
//! moved chord reaches an existing config only through the 3.3 migration rule
//! (a moved default is added only if its chord is unclaimed).
//!
//! An action's first chord is the one its menu item displays
//! (`registry_accel` is first-wins), so aliases come after the primary.

use crate::types::KeyBinding;

fn kb(key: &str, action: &str) -> KeyBinding {
    KeyBinding {
        key: key.to_string(),
        action: action.to_string(),
    }
}

/// Chords for macOS, where the native menu registers them with NSApp.
///
/// On macOS the menu consumes these chords before the registry ever runs, so
/// these defaults are what the menu *displays* and what the chord falls back
/// to on platforms without a native menu bar.
#[cfg(target_os = "macos")]
pub fn menu_chords() -> Vec<KeyBinding> {
    vec![
        kb("CmdOrCtrl+N", "new_window"),
        // iTerm2's Close (UX.md I15): the focused pane, cascading to the tab
        // and then the window. Close Tab is iTerm2's "Close All Panes in Tab".
        kb("CmdOrCtrl+W", "close_pane"),
        kb("CmdOrCtrl+Alt+W", "close_tab"),
        kb("CmdOrCtrl+Q", "quit"),
        kb("CmdOrCtrl+T", "new_tab"),
        kb("CmdOrCtrl+Shift+]", "next_tab"),
        kb("CmdOrCtrl+Shift+[", "prev_tab"),
        // iTerm2's Move Tab Left/Right (UX.md I9). The menu is what dispatches
        // these on macOS: the registry matcher compares logical keys, and
        // Cmd+Opt+Shift+[ arrives as '{'. The arrow chords stay as aliases.
        kb("CmdOrCtrl+Alt+Shift+[", "move_tab_left"),
        kb("CmdOrCtrl+Alt+Shift+]", "move_tab_right"),
        kb("CmdOrCtrl+Shift+Left", "move_tab_left"),
        kb("CmdOrCtrl+Shift+Right", "move_tab_right"),
        kb("CmdOrCtrl+1", "switch_to_tab_1"),
        kb("CmdOrCtrl+2", "switch_to_tab_2"),
        kb("CmdOrCtrl+3", "switch_to_tab_3"),
        kb("CmdOrCtrl+4", "switch_to_tab_4"),
        kb("CmdOrCtrl+5", "switch_to_tab_5"),
        kb("CmdOrCtrl+6", "switch_to_tab_6"),
        kb("CmdOrCtrl+7", "switch_to_tab_7"),
        kb("CmdOrCtrl+8", "switch_to_tab_8"),
        kb("CmdOrCtrl+9", "switch_to_tab_9"),
        kb("CmdOrCtrl+A", "select_all"),
        kb("CmdOrCtrl+Shift+K", "clear_scrollback"),
        kb("CmdOrCtrl+Shift+H", "toggle_clipboard_history"),
        kb("F11", "toggle_fullscreen"),
        // iTerm2's Toggle Full Screen (UX.md I32); F11 stays primary.
        kb("CmdOrCtrl+Ctrl+F", "toggle_fullscreen"),
        kb("Shift+F11", "maximize_vertically"),
        kb("CmdOrCtrl+=", "increase_font_size"),
        kb("CmdOrCtrl+-", "decrease_font_size"),
        kb("CmdOrCtrl+0", "reset_font_size"),
        kb("F3", "toggle_fps_overlay"),
        kb("F1", "toggle_help"),
        // iTerm2's tmux New Window / New Tab (UX.md I25), aliases of the
        // primary chords above, which already open daemon windows when
        // attached.
        kb("CmdOrCtrl+Ctrl+Shift+N", "new_window"),
        kb("CmdOrCtrl+Ctrl+Shift+T", "new_tab"),
    ]
}

/// Chords for Windows and Linux.
///
/// The menu model's `cmd_or_ctrl` is Ctrl+Shift here, so every letter the
/// menus advertise is spelled with it. On Windows the native menu registers
/// these chords; on Linux the in-app menu only labels them and these defaults
/// are what actually dispatches them (K20: `new_window` had no real chord
/// before this table). Close is UX.md I15 translated through the K1 family
/// (Cmd → Ctrl+Shift, Cmd+Opt → Ctrl+Alt): `Ctrl+Shift+W` closes the focused
/// pane, cascading to the tab and window, and `Ctrl+Alt+W` closes the tab.
/// The previous `Ctrl+Shift+X` pane close stays as an alias.
#[cfg(not(target_os = "macos"))]
pub fn menu_chords() -> Vec<KeyBinding> {
    vec![
        kb("Ctrl+Shift+N", "new_window"),
        kb("Ctrl+Shift+Q", "quit"),
        kb("Ctrl+Shift+T", "new_tab"),
        kb("Ctrl+Shift+W", "close_pane"),
        kb("Ctrl+Shift+X", "close_pane"),
        kb("Ctrl+Alt+W", "close_tab"),
        kb("Ctrl+Shift+]", "next_tab"),
        kb("Ctrl+Shift+[", "prev_tab"),
        kb("Ctrl+Shift+Left", "move_tab_left"),
        kb("Ctrl+Shift+Right", "move_tab_right"),
        kb("Alt+1", "switch_to_tab_1"),
        kb("Alt+2", "switch_to_tab_2"),
        kb("Alt+3", "switch_to_tab_3"),
        kb("Alt+4", "switch_to_tab_4"),
        kb("Alt+5", "switch_to_tab_5"),
        kb("Alt+6", "switch_to_tab_6"),
        kb("Alt+7", "switch_to_tab_7"),
        kb("Alt+8", "switch_to_tab_8"),
        kb("Alt+9", "switch_to_tab_9"),
        kb("Ctrl+Shift+A", "select_all"),
        kb("Ctrl+Shift+K", "clear_scrollback"),
        kb("Ctrl+Shift+H", "toggle_clipboard_history"),
        kb("F11", "toggle_fullscreen"),
        kb("Shift+F11", "maximize_vertically"),
        kb("Ctrl+Shift+=", "increase_font_size"),
        kb("Ctrl+Shift+-", "decrease_font_size"),
        kb("Ctrl+Shift+0", "reset_font_size"),
        kb("F3", "toggle_fps_overlay"),
        kb("F1", "toggle_help"),
    ]
}

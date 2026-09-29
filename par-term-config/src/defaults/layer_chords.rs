//! Hardcoded-layer chords as registry defaults (UX.md K2).
//!
//! The remaining chords the hardcoded key-handler layers shipped with — the
//! ones no menu advertises — as default keybindings, so every shipped chord
//! resolves through the registry and can be rebound or freed
//! (`pass_to_terminal`). The chords match what the hardcoded layers already
//! do; where a layer accepted any modifier combination (an unguarded
//! `matches!` on the logical key), the default pins the exact combination the
//! layer was written for. UX.md 3.3a moves them later, through the 3.3
//! migration rule (a moved default is added only if its chord is unclaimed).

use crate::types::KeyBinding;

fn kb(key: &str, action: &str) -> KeyBinding {
    KeyBinding {
        key: key.to_string(),
        action: action.to_string(),
    }
}

/// Chords for macOS.
#[cfg(target_os = "macos")]
pub fn layer_chords() -> Vec<KeyBinding> {
    vec![
        kb("F5", "reload_config"),
        kb("F12", "open_settings"),
        kb("CmdOrCtrl+F", "toggle_search"),
        kb("CmdOrCtrl+I", "toggle_ai_inspector"),
        kb("Shift+PageUp", "scroll_up_page"),
        kb("Shift+PageDown", "scroll_down_page"),
        kb("Shift+Home", "scroll_to_top"),
        kb("Shift+End", "scroll_to_bottom"),
        kb("Cmd+Up", "scroll_to_previous_mark"),
        kb("Cmd+Down", "scroll_to_next_mark"),
        // utility.rs tail (UX K2 dissolution): Ctrl+L and the Ctrl+Comma arm
        // of the cursor-style cycle. `Cmd+Comma` is the NSApp Settings
        // equivalent and stays off the registry on macOS.
        kb("Ctrl+L", "clear_screen"),
        kb("Ctrl+Comma", "cycle_cursor_style"),
        // tabs.rs tail (UX K2 dissolution): the Ctrl+Tab pair.
        kb("Ctrl+Tab", "next_tab"),
        kb("Ctrl+Shift+Tab", "prev_tab"),
        // The shifted spellings of the font-size keys. The matcher compares
        // logical characters, and the key press that produces '+'/'_' is a
        // different character from the '='/'-' the menu-chord defaults name —
        // the loose inline branch accepted both, so both must ship or the
        // dissolution drops the shifted press.
        kb("CmdOrCtrl+Plus", "increase_font_size"),
        kb("CmdOrCtrl+Shift+_", "decrease_font_size"),
    ]
}

/// Chords for Windows and Linux. `Cmd+`/`Ctrl+Shift+` mirror the hardcoded
/// layers' platform split; the mark-navigation arrows were Super-modified on
/// every platform, so they stay Super here.
#[cfg(not(target_os = "macos"))]
pub fn layer_chords() -> Vec<KeyBinding> {
    vec![
        kb("F5", "reload_config"),
        kb("F12", "open_settings"),
        kb("Ctrl+Shift+F", "toggle_search"),
        kb("Ctrl+Shift+I", "toggle_ai_inspector"),
        kb("Shift+PageUp", "scroll_up_page"),
        kb("Shift+PageDown", "scroll_down_page"),
        kb("Shift+Home", "scroll_to_top"),
        kb("Shift+End", "scroll_to_bottom"),
        kb("Super+Up", "scroll_to_previous_mark"),
        kb("Super+Down", "scroll_to_next_mark"),
        // utility.rs tail (UX K2 dissolution). `Ctrl+Comma` (not Super+Comma)
        // is what the settings table advertises on every platform; the loose
        // inline branch also fired on Super+Comma here, which the exact
        // registry match deliberately drops.
        kb("Ctrl+L", "clear_screen"),
        kb("Ctrl+Comma", "cycle_cursor_style"),
        // tabs.rs tail (UX K2 dissolution): the Ctrl+Tab pair.
        kb("Ctrl+Tab", "next_tab"),
        kb("Ctrl+Shift+Tab", "prev_tab"),
        // The shifted spellings of the font-size keys — without a native
        // menu bar here the registry is the only dispatch path, and
        // Ctrl+Shift+= produces the logical character '+', not '='.
        kb("Ctrl+Shift+Plus", "increase_font_size"),
        kb("Ctrl+Shift+_", "decrease_font_size"),
    ]
}

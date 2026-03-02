//! Input settings tab.
//!
//! Consolidates: keyboard_tab, mouse_tab, keybindings_tab
//!
//! Contains:
//! - Keyboard settings (Option/Alt key modes, modifier remapping, physical keys)
//! - Mouse behavior (scroll speed, click thresholds)
//! - Selection & Clipboard settings
//! - Keybindings editor
//!
//! ## Sub-module layout
//!
//! | File | Contents |
//! |------|----------|
//! | `mod.rs` (this file) | `show()` dispatcher — calls each section in order |
//! | `keyboard.rs` | Keyboard and modifier remapping sections |
//! | `mouse.rs` | Mouse behavior section |
//! | `selection.rs` | Selection, clipboard, and dropped-files sections |
//! | `word_selection.rs` | Word selection and copy mode sections |
//! | `keybindings.rs` | Keybindings editor, `AVAILABLE_ACTIONS`, `capture_key_combo`, `display_key_combo` |

mod keybindings;
mod keyboard;
mod mouse;
mod selection;
mod word_selection;

use crate::SettingsUI;
use crate::section::section_matches;
use std::collections::HashSet;

// Re-export the public key-capture utilities used by actions_tab and snippets_tab.
// `capture_key_combo` is `pub` so external code can call it.
// `display_key_combo` is `pub(crate)` — used within the settings-ui crate only.
pub use keybindings::capture_key_combo;
pub(crate) use keybindings::display_key_combo;

/// Show the input tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // Keyboard section
    if section_matches(
        &query,
        "Keyboard",
        &[
            "option",
            "alt",
            "meta",
            "esc",
            "physical",
            "keyboard layout",
            "terminal applications",
        ],
    ) {
        keyboard::show_keyboard_section(ui, settings, changes_this_frame, collapsed);
    }

    // Modifier Remapping section
    if section_matches(
        &query,
        "Modifier Remapping",
        &[
            "remap",
            "swap",
            "ctrl",
            "super",
            "cmd",
            "modifier",
            "left ctrl",
            "right ctrl",
            "left alt",
            "right alt",
        ],
    ) {
        keyboard::show_modifier_remapping_section(ui, settings, changes_this_frame, collapsed);
    }

    // Mouse section
    if section_matches(
        &query,
        "Mouse",
        &[
            "scroll",
            "scroll speed",
            "double-click",
            "triple-click",
            "focus follows",
            "option+click",
            "alt+click",
            "horizontal scroll",
        ],
    ) {
        mouse::show_mouse_section(ui, settings, changes_this_frame, collapsed);
    }

    // Selection & Clipboard section
    if section_matches(
        &query,
        "Selection & Clipboard",
        &[
            "copy",
            "paste",
            "middle-click",
            "auto-copy",
            "delay",
            "trailing newline",
            "quote style",
            "drop files",
            "dropped file",
        ],
    ) {
        selection::show_selection_section(ui, settings, changes_this_frame, collapsed);
    }

    // Clipboard Limits section (collapsed by default)
    if section_matches(
        &query,
        "Clipboard Limits",
        &["max", "sync", "bytes", "clipboard events", "limit"],
    ) {
        selection::show_clipboard_limits_section(ui, settings, changes_this_frame, collapsed);
    }

    // Word Selection section (collapsed by default)
    if section_matches(
        &query,
        "Word Selection",
        &[
            "word characters",
            "smart selection",
            "patterns",
            "urls",
            "emails",
            "paths",
        ],
    ) {
        word_selection::show_word_selection_section(ui, settings, changes_this_frame, collapsed);
    }

    // Copy Mode section
    if section_matches(
        &query,
        "Copy Mode",
        &[
            "copy mode",
            "vi",
            "vim",
            "yank",
            "visual",
            "selection mode",
            "keyboard-driven",
            "hjkl",
        ],
    ) {
        word_selection::show_copy_mode_section(ui, settings, changes_this_frame, collapsed);
    }

    // Keybindings section (takes most space)
    if section_matches(
        &query,
        "Keybindings",
        &[
            "shortcut",
            "hotkey",
            "binding",
            "key",
            "keyboard shortcut",
            "custom",
        ],
    ) {
        keybindings::show_keybindings_section(ui, settings, changes_this_frame, collapsed);
    }
}

/// Search keywords for the Input settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        // Keyboard
        "keyboard",
        "option",
        "alt",
        "meta",
        "esc",
        "physical",
        "physical keys",
        // Modifier remapping
        "remap",
        "remapping",
        "swap",
        "ctrl",
        "super",
        "cmd",
        "modifier",
        // Mouse
        "mouse",
        "scroll",
        "scroll speed",
        "double-click",
        "triple-click",
        "click threshold",
        "option+click",
        "alt+click",
        "focus follows",
        "focus follows mouse",
        "horizontal scroll",
        // Selection & clipboard
        "selection",
        "clipboard",
        "copy",
        "paste",
        "auto-copy",
        "auto copy",
        "trailing newline",
        "middle-click",
        "middle click",
        "dropped file",
        "quote style",
        // Clipboard limits
        "max sync",
        "max bytes",
        "clipboard max",
        // Word selection
        "word characters",
        "smart selection",
        // Keybindings
        "keybindings",
        "shortcuts",
        "hotkey",
        "binding",
        "key",
        // Copy mode
        "copy mode",
        "yank",
        // Paste
        "paste delay",
        // Smart selection
        "rules",
        "smart selection rules",
    ]
}

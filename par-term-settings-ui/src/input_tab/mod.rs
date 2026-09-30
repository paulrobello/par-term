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

pub mod actions_table;
mod keybindings;
mod keyboard;
mod mouse;
mod selection;
mod word_selection;

use crate::SettingsUI;
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
    // Keyboard section
    keyboard::show_keyboard_section(ui, settings, changes_this_frame, collapsed);

    // Modifier Remapping section
    keyboard::show_modifier_remapping_section(ui, settings, changes_this_frame, collapsed);

    // Mouse section
    mouse::show_mouse_section(ui, settings, changes_this_frame, collapsed);

    // Selection & Clipboard section
    selection::show_selection_section(ui, settings, changes_this_frame, collapsed);

    // Clipboard Limits section (collapsed by default)
    selection::show_clipboard_limits_section(ui, settings, changes_this_frame, collapsed);

    // Word Selection section (collapsed by default)
    word_selection::show_word_selection_section(ui, settings, changes_this_frame, collapsed);

    // Copy Mode section
    word_selection::show_copy_mode_section(ui, settings, changes_this_frame, collapsed);

    // Keybindings section (takes most space)
    keybindings::show_keybindings_section(ui, settings, changes_this_frame, collapsed);
}

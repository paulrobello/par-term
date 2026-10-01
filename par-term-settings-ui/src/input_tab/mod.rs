//! Keyboard, pointer, selection, and keybinding sections. Where each one is
//! drawn is set in [`crate::layout`].
//!
//! | File | Contents |
//! |------|----------|
//! | `keyboard.rs` | Keyboard and modifier remapping sections |
//! | `mouse.rs` | Mouse behavior section |
//! | `selection.rs` | Selection, clipboard, and dropped-files sections |
//! | `word_selection.rs` | Word selection and copy mode sections |
//! | `leader.rs` | Leader key section |
//! | `keybindings.rs` | Keybindings editor, `capture_key_combo`, `display_key_combo` |
//! | `actions_table.rs` | `AVAILABLE_ACTIONS` |

pub mod actions_table;
pub(crate) mod keybindings;
pub(crate) mod keyboard;
pub(crate) mod leader;
pub(crate) mod mouse;
pub(crate) mod selection;
pub(crate) mod word_selection;

// Re-export the public key-capture utilities used by actions_tab and snippets_tab.
// `capture_key_combo` is `pub` so external code can call it.
// `display_key_combo` is `pub(crate)` — used within the settings-ui crate only.
pub use keybindings::capture_key_combo;
pub(crate) use keybindings::display_key_combo;

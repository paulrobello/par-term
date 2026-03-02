//! Keyboard input event processing for WindowState.
//!
//! ## Sub-modules
//!
//! - `key_handler`: `handle_key_event` entry point and all per-category keyboard
//!   sub-handlers (scroll keys, config reload, clipboard history, command history,
//!   paste special, search, AI inspector, utility shortcuts, tab shortcuts,
//!   profile shortcuts, shortcut string building).
//! - `keybinding_actions`: named keybinding action dispatch (`execute_keybinding_action`).
//! - `keybinding_display_actions`: font-size, cursor-style, tab-switch, and
//!   throughput-mode action handlers (delegated from `keybinding_actions`).
//! - `keybinding_helpers`: visual helpers (`show_toast`, `show_pane_indices`) and
//!   shader toggles (`toggle_background_shader`, `toggle_cursor_shader`).
//! - `snippet_actions`: snippet execution (`execute_snippet`) and custom action
//!   execution (`execute_custom_action`).

mod key_handler;
mod keybinding_actions;
mod keybinding_display_actions;
mod keybinding_helpers;
mod snippet_actions;

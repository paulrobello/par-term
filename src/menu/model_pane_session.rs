//! The Pane and Session menus (UX.md V10, section 21.2).
//!
//! Every item runs a registry action ([`MenuAction::Action`]), so its
//! accelerator is the live registry binding and a rebind moves it — the
//! menu adds no chord of its own. Items whose action has no binding carry
//! none.

use super::actions::MenuAction;
use super::model::{MenuEntry, MenuItemSpec};

/// An item whose muda id is `menu_id` (section-prefixed: File > Close
/// already owns the bare `close_window` id) running registry action `id`.
fn action(menu_id: &'static str, id: &'static str, label: &'static str) -> MenuEntry {
    MenuEntry::Item(MenuItemSpec {
        id: menu_id,
        label,
        accelerator: None,
        action: MenuAction::Action(id),
    })
}

/// Pane: split, zoom, equalize, focus cycling, rename, broadcast, and
/// moving the pane to its own tab. Close is File > Close (iTerm2's pane
/// close, Cmd+W); a second item would register the same native chord.
pub(super) fn pane_entries() -> Vec<MenuEntry> {
    vec![
        action("pane_split_right", "split_right", "Split Right"),
        action("pane_split_down", "split_down", "Split Down"),
        MenuEntry::Separator,
        action("pane_toggle_pane_zoom", "toggle_pane_zoom", "Zoom Pane"),
        action("pane_equalize_panes", "equalize_panes", "Equalize Panes"),
        MenuEntry::Separator,
        action("pane_next_pane", "next_pane", "Next Pane"),
        action("pane_prev_pane", "prev_pane", "Previous Pane"),
        action("pane_last_pane", "last_pane", "Last-Focused Pane"),
        action(
            "pane_select_pane_hint",
            "select_pane_hint",
            "Select Pane by Letter",
        ),
        MenuEntry::Separator,
        action("pane_rename_pane", "rename_pane", "Rename Pane..."),
        action("pane_restart_pane", "restart_pane", "Restart Pane Process"),
        action(
            "pane_promote_pane_to_tab",
            "promote_pane_to_tab",
            "Move Pane to New Tab",
        ),
        MenuEntry::Separator,
        action(
            "pane_toggle_broadcast_input",
            "toggle_broadcast_input",
            "Broadcast Input to This Tab",
        ),
    ]
}

/// Session: par-mux / tmux sessions (attach, new, switch, detach) and the
/// tab and window navigation the session work leans on.
pub(super) fn session_entries() -> Vec<MenuEntry> {
    vec![
        action(
            "session_toggle_session_picker",
            "toggle_session_picker",
            "Sessions...",
        ),
        action(
            "session_new_mux_session",
            "new_mux_session",
            "New par-mux Session",
        ),
        action("session_detach", "detach", "Detach"),
        MenuEntry::Separator,
        action(
            "session_focus_next_attention_agent",
            "focus_next_attention_agent",
            "Next Agent Needing Attention",
        ),
        action(
            "session_toggle_tree_picker",
            "toggle_tree_picker",
            "Open Quickly...",
        ),
        MenuEntry::Separator,
        action("session_rename_tab", "rename_tab", "Rename Tab..."),
        action("session_last_tab", "last_tab", "Last-Used Tab"),
        action(
            "session_move_tab_to_window_picker",
            "move_tab_to_window_picker",
            "Move Tab to Window...",
        ),
        MenuEntry::Separator,
        action("session_next_window", "next_window", "Next Window"),
        action("session_prev_window", "prev_window", "Previous Window"),
        action("session_close_window", "close_window", "Close Window"),
    ]
}

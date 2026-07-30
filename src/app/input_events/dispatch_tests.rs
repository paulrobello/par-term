//! Coverage guards for the keybinding dispatch tables.
//!
//! `execute_keybinding_action` dispatches on `&str`, so it never had a
//! compile-time exhaustiveness check. What the `match` form it replaced *did*
//! give was `unreachable_patterns`: a duplicated string arm was a compiler
//! warning. A linear-scan table makes a duplicate key silently first-wins, so
//! `action_table_has_no_duplicate_keys` and its display-table twin exist to
//! replace exactly that guarantee.
//!
//! The remaining tests are guarantees the `match` form could not express:
//! cross-table disjointness, coverage of a frozen action inventory, and
//! coverage of every shipped default keybinding.

use super::key_handler::KEY_LAYERS;
use super::keybinding_actions::ACTION_HANDLERS;
use super::keybinding_display_actions::DISPLAY_ACTION_HANDLERS;

/// Every action name `execute_keybinding_action` resolved by exact match
/// before ENH-004 converted the two `match` ladders into dispatch tables.
///
/// Captured from the pre-refactor arms of `keybinding_actions.rs` (42 names)
/// and `keybinding_display_actions.rs` (20 names) at commit `d807b90e`.
///
/// **Do not regenerate this list from the dispatch tables.** It is an
/// independent record of what the terminal used to answer to; a list derived
/// from the tables would pass no matter how many entries were dropped.
/// Adding a genuinely new action means adding it here too, deliberately.
const FROZEN_ACTION_INVENTORY: &[&str] = &[
    "clear_scrollback",
    "close_pane",
    "close_tab",
    "close_window",
    "cycle_background_shader",
    "cycle_cursor_style",
    "decrease_font_size",
    "demote_tab_to_pane",
    "duplicate_tab",
    "enter_copy_mode",
    "increase_font_size",
    "maximize_vertically",
    "move_tab_left",
    "move_tab_right",
    "move_tab_to_new_window",
    "navigate_pane_down",
    "navigate_pane_left",
    "navigate_pane_right",
    "navigate_pane_up",
    "new_tab",
    "new_window",
    "next_tab",
    "open_settings",
    "paste_special",
    "prev_tab",
    "promote_pane_to_tab",
    "quit",
    "reload_config",
    "reload_dynamic_profiles",
    "reopen_closed_tab",
    "reset_font_size",
    "resize_pane_down",
    "resize_pane_left",
    "resize_pane_right",
    "resize_pane_up",
    "save_arrangement",
    "select_all",
    "split_horizontal",
    "split_vertical",
    "ssh_quick_connect",
    "switch_to_tab_1",
    "switch_to_tab_2",
    "switch_to_tab_3",
    "switch_to_tab_4",
    "switch_to_tab_5",
    "switch_to_tab_6",
    "switch_to_tab_7",
    "switch_to_tab_8",
    "switch_to_tab_9",
    "toggle_ai_inspector",
    "toggle_background_shader",
    "toggle_broadcast_input",
    "toggle_clipboard_history",
    "toggle_command_history",
    "toggle_copy_mode",
    "toggle_cursor_shader",
    "toggle_fps_overlay",
    "toggle_fullscreen",
    "toggle_help",
    "toggle_menu",
    "toggle_profile_drawer",
    "toggle_search",
    "toggle_session_logging",
    "toggle_shader_animation",
    "toggle_shader_readability_mode",
    "toggle_throughput_mode",
    "toggle_tmux_session_picker",
];

/// Precedence order of the uniform shortcut layers in `handle_key_event`.
///
/// Same rule as the inventory above: this is a record, not a derivation. An
/// earlier layer pre-empts a later one for the same chord, so a reordering is
/// a behavior change and must be made on purpose.
const FROZEN_LAYER_ORDER: &[&str] = &[
    "scroll_keys",
    "config_reload",
    "clipboard_history",
    "command_history",
    "paste_special",
    "search",
    "ai_inspector_toggle",
    "fullscreen_toggle",
    "help_toggle",
    "settings_toggle",
    "shader_editor_toggle",
    "fps_overlay_toggle",
    "profile_drawer_toggle",
    "profile_shortcuts",
];

fn action_keys() -> Vec<&'static str> {
    ACTION_HANDLERS.iter().map(|(name, _)| *name).collect()
}

fn display_keys() -> Vec<&'static str> {
    DISPLAY_ACTION_HANDLERS
        .iter()
        .map(|(name, _)| *name)
        .collect()
}

fn assert_no_duplicates(table: &str, mut keys: Vec<&'static str>) {
    keys.sort_unstable();
    let total = keys.len();
    keys.dedup();
    assert_eq!(
        total,
        keys.len(),
        "{table} has a duplicate key — a linear-scan table silently uses the \
         first entry, so the later one is dead code"
    );
}

/// Replaces the `unreachable_patterns` warning the `match` form provided.
#[test]
fn action_table_has_no_duplicate_keys() {
    assert_no_duplicates("ACTION_HANDLERS", action_keys());
}

/// Replaces the `unreachable_patterns` warning the `match` form provided.
#[test]
fn display_action_table_has_no_duplicate_keys() {
    assert_no_duplicates("DISPLAY_ACTION_HANDLERS", display_keys());
}

/// A guarantee that never existed: the display table is only consulted when
/// the main table misses, so a name present in both would have its display
/// entry silently shadowed.
#[test]
fn action_tables_are_disjoint() {
    let main = action_keys();
    let shadowed: Vec<&str> = display_keys()
        .into_iter()
        .filter(|k| main.contains(k))
        .collect();
    assert!(
        shadowed.is_empty(),
        "these names appear in both ACTION_HANDLERS and DISPLAY_ACTION_HANDLERS, \
         so the display entries are unreachable: {shadowed:?}"
    );
}

/// The anti-drop guard: a dispatch-table refactor fails silently, and this is
/// what makes it fail loudly instead.
#[test]
fn dispatch_tables_cover_the_frozen_action_inventory() {
    let mut live: Vec<&str> = action_keys();
    live.extend(display_keys());
    live.sort_unstable();

    let missing: Vec<&&str> = FROZEN_ACTION_INVENTORY
        .iter()
        .filter(|name| !live.contains(name))
        .collect();
    assert!(
        missing.is_empty(),
        "actions the terminal used to handle are no longer dispatchable: {missing:?}"
    );

    let added: Vec<&str> = live
        .iter()
        .filter(|name| !FROZEN_ACTION_INVENTORY.contains(name))
        .copied()
        .collect();
    assert!(
        added.is_empty(),
        "new actions are dispatchable but absent from FROZEN_ACTION_INVENTORY — \
         add them there deliberately: {added:?}"
    );
}

/// Driven from the shipped defaults rather than a copied list, so it cannot
/// drift: any default chord whose action no handler claims would be a shortcut
/// that does nothing.
#[test]
fn every_default_keybinding_resolves_to_a_handler() {
    let mut live: Vec<&str> = action_keys();
    live.extend(display_keys());

    // Prefix forms are resolved at runtime against the user's snippets,
    // actions, and arrangements, so they are dispatchable without a table
    // entry. No shipped default uses one today.
    const PREFIXES: &[&str] = &["snippet:", "action:", "restore_arrangement:"];

    let defaults = crate::config::Config::default().keybindings;
    assert!(
        !defaults.is_empty(),
        "Config::default() shipped no keybindings — this test would be vacuous"
    );

    let unhandled: Vec<String> = defaults
        .iter()
        .filter(|kb| {
            !live.contains(&kb.action.as_str())
                && !PREFIXES.iter().any(|p| kb.action.starts_with(p))
        })
        .map(|kb| format!("{} -> {}", kb.key, kb.action))
        .collect();
    assert!(
        unhandled.is_empty(),
        "default keybindings whose action has no handler (the chord would do \
         nothing): {unhandled:?}"
    );
}

/// Ordering tripwire, deliberately a change-detector: `KEY_LAYERS` is ordered
/// dispatch, so silently reordering it changes which shortcut wins a chord.
#[test]
fn key_layer_precedence_is_unchanged() {
    let live: Vec<&str> = KEY_LAYERS.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        live, FROZEN_LAYER_ORDER,
        "KEY_LAYERS precedence changed — an earlier layer pre-empts a later one \
         for the same chord, so update FROZEN_LAYER_ORDER only alongside a \
         deliberate precedence change"
    );
}

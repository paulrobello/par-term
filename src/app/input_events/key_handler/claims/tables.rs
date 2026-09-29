//! The declared claim tables, one per dispatch source.
//!
//! Split from the parent module when it crossed the 800-line gate: the
//! parent holds the vocabulary (`Mods`, `ModSpec`, `Chord`, `Claim`) and the
//! precedence chain; this file is the data those describe.

use super::*;

/// `clipboard.rs`. State-conditional only: the panel is opened by the
/// configured `toggle_clipboard_history` keybinding, and while it is open the
/// layer consumes keys wholesale.
pub(crate) const CLIPBOARD_HISTORY: &[Claim] = &[];

/// `command_history.rs` claims no chord at all — the toggle is driven entirely
/// by the configured `toggle_command_history` keybinding. Only in-panel
/// navigation lives there, and that is state-conditional.
const COMMAND_HISTORY: &[Claim] = &[];

/// `clipboard.rs`, paste-special branch: state-conditional only.
const PASTE_SPECIAL_UI: &[Claim] = &[];

/// `command_palette.rs` claims no chord at all — it is opened by the configured
/// `toggle_command_palette` keybinding, and its only in-panel key (Escape) is
/// state-conditional.
const COMMAND_PALETTE: &[Claim] = &[];

/// `agent_usage_panel.rs`. State-conditional (Esc/h/l while the panel is
/// visible); it opens from a widget click or the `toggle_agent_usage_panel`
/// action, never a chord of its own.
const AGENT_USAGE_PANEL: &[Claim] = &[];

/// `search.rs`. State-conditional only: the search bar is opened by the
/// configured `toggle_search` keybinding; while it is visible, Escape closes
/// it and everything else propagates to egui.
pub(crate) const SEARCH: &[Claim] = &[];

/// `window_state/keyboard_handlers.rs`. State-conditional only: Escape closes
/// the help/shader-install/integrations overlays; the panel itself opens via
/// the configured `toggle_help` keybinding.
const HELP_TOGGLE: &[Claim] = &[];

/// `profiles.rs` matches against the user's `profiles.yaml`, so what it claims
/// is not knowable at build time. Deliberately empty; see the coverage note in
/// `chord_tests`.
const PROFILE_SHORTCUTS: &[Claim] = &[];

/// `utility.rs` — mirrored (the font and cursor-style branches accept several
/// keys and tolerate extra modifiers, so converting them to driven matching
/// would risk changing which chord does what).
const UTILITY: &[Claim] = &[
    Claim {
        action: "clear_scrollback",
        mac: Some(PRIMARY_SHIFT_MAC),
        other: Some(PRIMARY_SHIFT_OTHER),
        keys: &[ch('K')],
    },
    Claim {
        action: "internal:clear_screen",
        mac: Some(ModSpec::loose(
            mods(true, false, false, false),
            mods(false, false, true, false),
        )),
        other: Some(ModSpec::loose(
            mods(true, false, false, false),
            mods(false, false, true, false),
        )),
        keys: &[ch('L')],
    },
    // `font_mod` is the super key on macOS and Ctrl elsewhere, with nothing
    // excluded. Both `+` and `=` are accepted so a shifted `=` cannot leak.
    Claim {
        action: "increase_font_size",
        mac: Some(ModSpec::loose(mods(false, false, false, true), NO_MODS)),
        other: Some(ModSpec::loose(mods(true, false, false, false), NO_MODS)),
        keys: &[ch('+'), ch('=')],
    },
    Claim {
        action: "decrease_font_size",
        mac: Some(ModSpec::loose(mods(false, false, false, true), NO_MODS)),
        other: Some(ModSpec::loose(mods(true, false, false, false), NO_MODS)),
        keys: &[ch('-'), ch('_')],
    },
    Claim {
        action: "reset_font_size",
        mac: Some(ModSpec::loose(
            mods(false, false, false, true),
            mods(false, false, true, false),
        )),
        other: Some(ModSpec::loose(
            mods(true, false, false, false),
            mods(false, false, true, false),
        )),
        keys: &[ch('0')],
    },
    // `ctrl || super_key`, so this is two predicates, not one.
    Claim {
        action: "cycle_cursor_style",
        mac: Some(ModSpec::loose(
            mods(true, false, false, false),
            mods(false, false, true, false),
        )),
        other: Some(ModSpec::loose(
            mods(true, false, false, false),
            mods(false, false, true, false),
        )),
        keys: &[ch(',')],
    },
    Claim {
        action: "cycle_cursor_style",
        mac: Some(ModSpec::loose(
            mods(false, false, false, true),
            mods(false, false, true, false),
        )),
        other: Some(ModSpec::loose(
            mods(false, false, false, true),
            mods(false, false, true, false),
        )),
        keys: &[ch(',')],
    },
];

/// `tabs.rs` — mirrored.
const TABS: &[Claim] = &[
    Claim {
        action: "new_tab",
        mac: Some(PRIMARY_MAC),
        other: Some(PRIMARY_SHIFT_OTHER),
        keys: &[ch('T')],
    },
    Claim {
        action: "close_tab",
        mac: Some(PRIMARY_MAC),
        other: Some(PRIMARY_SHIFT_OTHER),
        keys: &[ch('W')],
    },
    Claim {
        action: "next_tab",
        mac: Some(PRIMARY_SHIFT_MAC),
        other: Some(PRIMARY_SHIFT_OTHER),
        keys: &[ch(']')],
    },
    Claim {
        action: "prev_tab",
        mac: Some(PRIMARY_SHIFT_MAC),
        other: Some(PRIMARY_SHIFT_OTHER),
        keys: &[ch('[')],
    },
    Claim {
        action: "next_tab",
        mac: Some(ModSpec::loose(
            mods(true, false, false, false),
            mods(false, false, true, false),
        )),
        other: Some(ModSpec::loose(
            mods(true, false, false, false),
            mods(false, false, true, false),
        )),
        keys: &[named(NamedKey::Tab)],
    },
    Claim {
        action: "prev_tab",
        mac: Some(ModSpec::loose(mods(true, false, true, false), NO_MODS)),
        other: Some(ModSpec::loose(mods(true, false, true, false), NO_MODS)),
        keys: &[named(NamedKey::Tab)],
    },
    Claim {
        action: "move_tab_left",
        mac: Some(PRIMARY_SHIFT_MAC),
        other: Some(PRIMARY_SHIFT_OTHER),
        keys: &[named(NamedKey::ArrowLeft)],
    },
    Claim {
        action: "move_tab_right",
        mac: Some(PRIMARY_SHIFT_MAC),
        other: Some(PRIMARY_SHIFT_OTHER),
        keys: &[named(NamedKey::ArrowRight)],
    },
    // Number switching: `primary_modifier` on macOS, `alt && !shift && !ctrl`
    // (Super not excluded) elsewhere.
    Claim {
        action: "switch_to_tab_1",
        mac: Some(PRIMARY_MAC),
        other: Some(TAB_SWITCH_OTHER),
        keys: &[ch('1')],
    },
    Claim {
        action: "switch_to_tab_2",
        mac: Some(PRIMARY_MAC),
        other: Some(TAB_SWITCH_OTHER),
        keys: &[ch('2')],
    },
    Claim {
        action: "switch_to_tab_3",
        mac: Some(PRIMARY_MAC),
        other: Some(TAB_SWITCH_OTHER),
        keys: &[ch('3')],
    },
    Claim {
        action: "switch_to_tab_4",
        mac: Some(PRIMARY_MAC),
        other: Some(TAB_SWITCH_OTHER),
        keys: &[ch('4')],
    },
    Claim {
        action: "switch_to_tab_5",
        mac: Some(PRIMARY_MAC),
        other: Some(TAB_SWITCH_OTHER),
        keys: &[ch('5')],
    },
    Claim {
        action: "switch_to_tab_6",
        mac: Some(PRIMARY_MAC),
        other: Some(TAB_SWITCH_OTHER),
        keys: &[ch('6')],
    },
    Claim {
        action: "switch_to_tab_7",
        mac: Some(PRIMARY_MAC),
        other: Some(TAB_SWITCH_OTHER),
        keys: &[ch('7')],
    },
    Claim {
        action: "switch_to_tab_8",
        mac: Some(PRIMARY_MAC),
        other: Some(TAB_SWITCH_OTHER),
        keys: &[ch('8')],
    },
    Claim {
        action: "switch_to_tab_9",
        mac: Some(PRIMARY_MAC),
        other: Some(TAB_SWITCH_OTHER),
        keys: &[ch('9')],
    },
];

pub(super) const TAB_SWITCH_OTHER: ModSpec = ModSpec::loose(
    mods(false, true, false, false),
    mods(true, false, true, false),
);

/// The paste and copy branches inlined at the end of `handle_key_event`.
/// Mirrored. Neither has an `AVAILABLE_ACTIONS` row.
const PASTE_COPY: &[Claim] = &[
    Claim {
        action: "internal:paste",
        // macOS tests only `cmd`, so Cmd+Shift+V is inside this claim; the
        // shipped `paste_special` default consumes it first.
        mac: Some(ModSpec::loose(mods(false, false, false, true), NO_MODS)),
        other: Some(ModSpec::loose(mods(true, false, true, false), NO_MODS)),
        keys: &[ch('V')],
    },
    Claim {
        action: "internal:paste",
        mac: Some(ANY_MODS),
        other: Some(ANY_MODS),
        keys: &[named(NamedKey::Paste)],
    },
    Claim {
        action: "internal:paste",
        mac: None,
        other: Some(ModSpec::loose(mods(false, false, true, false), NO_MODS)),
        keys: &[named(NamedKey::Insert)],
    },
    Claim {
        action: "internal:copy",
        // Same superset shape: Cmd+Shift+C is inside it, and the shipped
        // `toggle_copy_mode` default consumes it first.
        mac: Some(ModSpec::loose(mods(false, false, false, true), NO_MODS)),
        other: Some(ModSpec::loose(mods(true, false, true, false), NO_MODS)),
        keys: &[ch('C')],
    },
    Claim {
        action: "internal:copy",
        mac: Some(ANY_MODS),
        other: Some(ANY_MODS),
        keys: &[named(NamedKey::Copy)],
    },
];

/// macOS application-menu accelerators, from `crate::menu::macos`. Mirrored —
/// that module builds `muda` items directly rather than going through
/// `menu_model`, and it is outside this module's ownership.
pub(super) const MACOS_APP_MENU: &[Claim] = &[
    Claim {
        action: "open_settings",
        mac: Some(ModSpec::exact(mods(false, false, false, true))),
        other: None,
        keys: &[ch(',')],
    },
    Claim {
        action: "quit",
        mac: Some(ModSpec::exact(mods(false, false, false, true))),
        other: None,
        keys: &[ch('Q')],
    },
    Claim {
        action: "internal:minimize",
        mac: Some(ModSpec::exact(mods(false, false, false, true))),
        other: None,
        keys: &[ch('M')],
    },
];

/// The hardcoded layers, in the order `handle_key_event` consults them.
///
/// The first eight mirror [`super::KEY_LAYERS`] one-for-one — `chord_tests`
/// asserts that correspondence so a new layer cannot be added without declaring
/// what it claims. The last three continue the same chain but are invoked
/// directly (two need the `ActiveEventLoop`; the paste/copy branch is inline).
///
/// The chord-only layers (scroll, config reload, the UI toggles) dissolved into
/// registry defaults (`defaults::layer_chords`, UX K2): their chords arrive in
/// the chain as `config_keybindings` rules, derived from the shipped defaults.
pub(crate) const LAYER_CLAIMS: &[(&str, &[Claim])] = &[
    ("clipboard_history", CLIPBOARD_HISTORY),
    ("command_history", COMMAND_HISTORY),
    ("paste_special", PASTE_SPECIAL_UI),
    ("agent_usage_panel", AGENT_USAGE_PANEL),
    ("command_palette", COMMAND_PALETTE),
    ("search", SEARCH),
    ("help_toggle", HELP_TOGGLE),
    ("profile_shortcuts", PROFILE_SHORTCUTS),
    ("utility_shortcuts", UTILITY),
    ("tab_shortcuts", TABS),
    ("paste_copy", PASTE_COPY),
];

/// Number of [`LAYER_CLAIMS`] entries that correspond to [`super::KEY_LAYERS`].
pub(crate) const UNIFORM_LAYER_COUNT: usize = 8;

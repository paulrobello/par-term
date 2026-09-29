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

// `utility.rs` and `tabs.rs` are gone (UX K2 dissolution): every chord they
// claimed is a registry default in `defaults::menu_chords` /
// `defaults::layer_chords` and arrives in the chain as a
// `config_keybindings` rule.

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

/// The hardcoded dispatch sources that remain, in the order
/// `handle_key_event` consults them.
///
/// The first eight mirror [`super::KEY_LAYERS`] one-for-one — `chord_tests`
/// asserts that correspondence so a new layer cannot be added without declaring
/// what it claims. The last is the inline paste/copy branch, the one
/// deliberate chord exemption.
///
/// Every chord-only layer (scroll, config reload, the UI toggles, utility,
/// tabs) dissolved into registry defaults (`defaults::layer_chords` and
/// `defaults::menu_chords`, UX K2): their chords arrive in the chain as
/// `config_keybindings` rules, derived from the shipped defaults.
pub(crate) const LAYER_CLAIMS: &[(&str, &[Claim])] = &[
    ("clipboard_history", CLIPBOARD_HISTORY),
    ("command_history", COMMAND_HISTORY),
    ("paste_special", PASTE_SPECIAL_UI),
    ("agent_usage_panel", AGENT_USAGE_PANEL),
    ("command_palette", COMMAND_PALETTE),
    ("search", SEARCH),
    ("help_toggle", HELP_TOGGLE),
    ("profile_shortcuts", PROFILE_SHORTCUTS),
    ("paste_copy", PASTE_COPY),
];

/// Number of [`LAYER_CLAIMS`] entries that correspond to [`super::KEY_LAYERS`].
pub(crate) const UNIFORM_LAYER_COUNT: usize = 8;

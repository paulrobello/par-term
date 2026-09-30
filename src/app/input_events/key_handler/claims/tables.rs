//! The declared claim tables, one per dispatch source.
//!
//! Split from the parent module when it crossed the 800-line gate: the
//! parent holds the vocabulary (`Mods`, `ModSpec`, `Chord`, `Claim`) and the
//! precedence chain; this file is the data those describe.

use super::*;

// The per-overlay state-machine layers (clipboard history, command history,
// paste special, agent usage, palette, search, help) are gone (MP1 Q4): their
// overlays are Popups/Modals in the overlay stack, which consumes every key
// they do not close on before `handle_key_event` runs, so those layers were
// unreachable. Their claim slices were empty — no chord was lost with them.

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
/// The first mirrors [`super::KEY_LAYERS`] one-for-one — `chord_tests`
/// asserts that correspondence so a new layer cannot be added without declaring
/// what it claims. The last is the inline paste/copy branch, the one
/// deliberate chord exemption.
///
/// Every chord-only layer (scroll, config reload, the UI toggles, utility,
/// tabs) dissolved into registry defaults (`defaults::layer_chords` and
/// `defaults::menu_chords`, UX K2): their chords arrive in the chain as
/// `config_keybindings` rules, derived from the shipped defaults.
pub(crate) const LAYER_CLAIMS: &[(&str, &[Claim])] = &[
    ("profile_shortcuts", PROFILE_SHORTCUTS),
    ("paste_copy", PASTE_COPY),
];

/// Number of [`LAYER_CLAIMS`] entries that correspond to [`super::KEY_LAYERS`].
pub(crate) const UNIFORM_LAYER_COUNT: usize = 1;

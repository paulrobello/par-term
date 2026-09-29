//! The gate over `AVAILABLE_ACTIONS`' advertised-chord column.
//!
//! # What is covered
//!
//! For every row of `par_term_settings_ui::input_tab::actions_table::AVAILABLE_ACTIONS`
//! that advertises a chord, [`advertised_chord_is_dispatched_by_its_own_action`]
//! resolves that chord against [`claims::claim_chain`] — the ordered list of
//! everything that can consume a key press before the terminal sees it — and
//! asserts the *first* source to claim it runs the action that advertises it.
//! That ordering clause is the point: it is what catches an advertised chord
//! that a higher-precedence layer silently eats.
//!
//! # What is not covered, and why
//!
//! - **Runtime-dynamic sources are excluded entirely**: the tmux prefix, the
//!   custom-action prefix, and `profile_shortcuts` (which matches against the
//!   user's `profiles.yaml`). Any of them can shadow a chord at runtime and this
//!   gate will not notice.
//! - **State-conditional branches are excluded**: while the clipboard-history,
//!   paste-special, command-history or search panels are open they swallow keys
//!   wholesale. That is modal behaviour, not a chord claim.
//! - **Layer claims are all state-conditional** — every uniform layer's claim
//!   slice is empty (K2: no layer holds a chord; `uniform_layers_claim_no_chords`
//!   gates it), and the macOS application menu lives in `menu/macos.rs`,
//!   outside this module.
//! - **The menu and the config defaults are derived, not mirrored** — they read
//!   `crate::menu::model::menu_model` and `Config::default().keybindings` live.
//! - **One platform per run.** `AVAILABLE_ACTIONS` and `Config::default()` are
//!   both `#[cfg]`-selected, so a run only ever sees the host's table. CI runs
//!   `cargo test --workspace` on Linux, macOS and Windows, so all three are
//!   covered across the matrix, never within one run.
//! - **The gate is one-directional.** It proves advertised → dispatched. The
//!   reverse (something dispatchable that the table advertises as having no
//!   default) is reported by [`claimed_actions_advertised_as_having_no_default`]
//!   against a frozen list, not gated.

use super::claims::{self, Chord, Platform};
use par_term_settings_ui::input_tab::actions_table::AVAILABLE_ACTIONS;

/// Advertised chords whose first claimer runs a different action.
///
/// Frozen deliberately: these are pre-existing and this gate exists to stop new
/// ones, not to change what any chord does today. Each entry is
/// `action -> first claiming source / action it actually runs`.
#[cfg(target_os = "macos")]
const KNOWN_MISMATCHES: &[(&str, &str)] = &[
    // The `Close` item in the File menu is a deliberate "smart close": it closes
    // the active tab when more than one is open and the window otherwise. The
    // native accelerator therefore claims Cmd+W before `tab_shortcuts` does, and
    // its `MenuAction` is named for the window case.
    ("close_tab", "native_menu/close_window"),
];

/// Windows attaches a native muda menu bar too, so it has the same smart-close
/// conflict; `cmd_or_ctrl` is Ctrl+Shift there, making the chord Ctrl+Shift+W.
#[cfg(target_os = "windows")]
const KNOWN_MISMATCHES: &[(&str, &str)] = &[("close_tab", "native_menu/close_window")];

/// Linux/BSD cannot attach a native menu bar (muda needs a `gtk::Window` winit
/// never creates), so the in-app egui menu only *draws* accelerator labels and
/// claims nothing. The smart-close conflict is therefore absent here.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const KNOWN_MISMATCHES: &[(&str, &str)] = &[];

/// Actions the table advertises as having no default, which some modeled source
/// nevertheless claims a chord for.
///
/// Reported, not gated: under-advertisement is a documentation gap, not a
/// dispatch defect, and correcting the table is a separate decision.
///
/// The menu-advertised chords became registry defaults (`defaults::menu_chords`,
/// UX K2), so every action the menus accelerate is now advertised with its
/// default and this set is empty on macOS.
#[cfg(target_os = "macos")]
const CLAIMED_BUT_ADVERTISED_AS_NONE: &[&str] = &[];

/// Only `close_window` remains: no non-macOS default ships for it, so its
/// native menu-bar accelerator is still a chord par-term's own key handling
/// never dispatches.
#[cfg(target_os = "windows")]
const CLAIMED_BUT_ADVERTISED_AS_NONE: &[&str] = &["close_window (native_menu)"];

/// No native menu here, so every menu-only entry disappears and nothing modeled
/// claims a chord the table advertises as having none.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const CLAIMED_BUT_ADVERTISED_AS_NONE: &[&str] = &[];

fn advertised() -> Vec<(&'static str, Chord)> {
    AVAILABLE_ACTIONS
        .iter()
        .filter_map(|(action, _, default)| default.map(|d| (*action, d)))
        .map(|(action, d)| {
            let chord = claims::parse_chord(d)
                .unwrap_or_else(|e| panic!("AVAILABLE_ACTIONS row {action:?}: {e}"));
            (action, chord)
        })
        .collect()
}

/// A row whose chord cannot be parsed must fail, never be skipped — a
/// permissive parser would launder exactly the strings this gate exists to
/// catch.
#[test]
fn every_advertised_chord_parses() {
    let bad: Vec<String> = AVAILABLE_ACTIONS
        .iter()
        .filter_map(|(action, _, default)| {
            let d = (*default)?;
            claims::parse_chord(d)
                .err()
                .map(|e| format!("{action}: {e}"))
        })
        .collect();
    assert!(
        bad.is_empty(),
        "advertised default chords that do not parse: {bad:#?}"
    );
}

/// The invariant: an advertised chord reaches the action that advertises it,
/// and no higher-precedence source claims it first.
#[test]
fn advertised_chord_is_dispatched_by_its_own_action() {
    // Both failure modes go into one list so neither can mask the other: a row
    // nothing claims and a row the wrong layer claims are the same defect from
    // the user's side — the settings window names a chord that does not do what
    // it says.
    let mut violations: Vec<(String, String)> = Vec::new();

    for (action, chord) in advertised() {
        match claims::first_claimer(chord) {
            None => violations.push((action.to_string(), NOTHING_CLAIMS_IT.to_string())),
            Some((source, claimed)) if claimed != action => {
                violations.push((action.to_string(), format!("{source}/{claimed}")));
            }
            Some(_) => {}
        }
    }
    violations.sort();

    let mut expected: Vec<(String, String)> = KNOWN_MISMATCHES
        .iter()
        .map(|(a, s)| (a.to_string(), s.to_string()))
        .collect();
    expected.sort();

    assert_eq!(
        violations, expected,
        "advertised chords whose first claimer is not the advertising action \
         changed. {NOTHING_CLAIMS_IT:?} means the shortcut does nothing at all; \
         a `source/action` value means a higher-precedence layer eats the chord \
         first. A new entry is a defect; a disappeared entry means a known one \
         was fixed and KNOWN_MISMATCHES should shrink."
    );
}

/// Marker used in place of `source/action` when no source claims the chord.
const NOTHING_CLAIMS_IT: &str = "<nothing>";

/// Complementary and cheap: where an action *does* ship default keybindings,
/// the advertised string must name one of them. Compared as parsed chords,
/// so `CmdOrCtrl` expansion, modifier ordering, and the word/literal
/// punctuation spellings (`Ctrl+Comma` vs `Ctrl+,`) cannot cause a false
/// failure. An action may ship more than one chord (next_tab: the bracket
/// pair and Ctrl+Tab; the font keys: the base and shifted spellings); the
/// advertised column names the primary one, which is also the chord the menu
/// displays (`registry_accel` is first-wins).
#[test]
fn advertised_chord_agrees_with_the_shipped_default() {
    let defaults = par_term_config::Config::default().keybindings;
    assert!(
        !defaults.is_empty(),
        "Config::default() shipped no keybindings — this test would be vacuous"
    );

    let mut checked = 0usize;
    let mut wrong: Vec<String> = Vec::new();

    for (action, default) in AVAILABLE_ACTIONS
        .iter()
        .filter_map(|(action, _, default)| default.map(|default| (*action, default)))
    {
        let shipped: Vec<claims::Chord> = defaults
            .iter()
            .filter(|kb| kb.action == action)
            .map(|kb| {
                claims::parse_chord(&kb.key)
                    .unwrap_or_else(|e| panic!("shipped default {:?}: {e}", kb.key))
            })
            .collect();
        if shipped.is_empty() {
            continue;
        }
        let shown = claims::parse_chord(default)
            .unwrap_or_else(|e| panic!("AVAILABLE_ACTIONS row {action:?}: {e}"));
        checked += 1;
        if !shipped.contains(&shown) {
            wrong.push(format!(
                "{action}: table says {default:?}, shipped defaults are {:?}",
                defaults
                    .iter()
                    .filter(|kb| kb.action == action)
                    .map(|kb| kb.key.as_str())
                    .collect::<Vec<_>>()
            ));
        }
    }

    assert!(checked > 0, "no advertised action has a shipped default");
    assert!(
        wrong.is_empty(),
        "advertised chords that disagree with Config::default().keybindings: {wrong:#?}"
    );
}

/// Tripwire: a new entry in `KEY_LAYERS` must come with a claim declaration, or
/// the precedence answer silently omits a layer.
#[test]
fn every_key_layer_declares_its_claims() {
    // Pin the split point to KEY_LAYERS itself. Without this, adding a 15th
    // uniform layer plus a matching 15th claim entry would still compare
    // 14-against-14 and pass, while `utility_shortcuts` silently slid to
    // position 16 — leaving the model's precedence order wrong and the gate
    // confidently answering from it.
    assert_eq!(
        claims::UNIFORM_LAYER_COUNT,
        super::KEY_LAYERS.len(),
        "UNIFORM_LAYER_COUNT must track KEY_LAYERS::len()"
    );

    let layers: Vec<&str> = super::KEY_LAYERS.iter().map(|(name, _)| *name).collect();
    let declared: Vec<&str> = claims::LAYER_CLAIMS
        .iter()
        .take(claims::UNIFORM_LAYER_COUNT)
        .map(|(name, _)| *name)
        .collect();
    assert_eq!(
        layers, declared,
        "LAYER_CLAIMS must mirror KEY_LAYERS one-for-one and in order"
    );

    let tail: Vec<&str> = claims::LAYER_CLAIMS
        .iter()
        .skip(claims::UNIFORM_LAYER_COUNT)
        .map(|(name, _)| *name)
        .collect();
    assert_eq!(
        tail,
        ["paste_copy"],
        "the only source after KEY_LAYERS is the inline paste/copy branch, the \
         one deliberate chord exemption — the utility and tab chord layers \
         dissolved into registry defaults (UX K2), and a new entry here means \
         a hardcoded chord layer came back"
    );
}

/// K2: the uniform shortcut layers are state machines (dialog navigation,
/// consume-while-open), not chord sources — every shipped chord resolves
/// through the registry. A non-empty claim here means a hardcoded chord came
/// back from the dissolution.
#[test]
fn uniform_layers_claim_no_chords() {
    let claiming: Vec<&str> = claims::LAYER_CLAIMS
        .iter()
        .take(claims::UNIFORM_LAYER_COUNT)
        .filter(|(_, cs)| !cs.is_empty())
        .map(|(name, _)| *name)
        .collect();
    assert!(
        claiming.is_empty(),
        "KEY_LAYERS entries claiming hardcoded chords: {claiming:?} — every \
         shipped chord must resolve through the registry (UX K2)"
    );
}

/// Every claim must name either a real advertised action or an explicitly
/// internal one, so a typo cannot make a claim quietly unmatchable.
#[test]
fn claim_action_names_are_advertised_or_explicitly_internal() {
    let unknown: Vec<&str> = claims::LAYER_CLAIMS
        .iter()
        .flat_map(|(_, cs)| cs.iter())
        .map(|c| c.action)
        .filter(|a| {
            !a.starts_with("internal:") && !AVAILABLE_ACTIONS.iter().any(|(name, _, _)| name == a)
        })
        .collect();
    assert!(
        unknown.is_empty(),
        "layer claims naming an action that is neither advertised nor prefixed \
         `internal:`: {unknown:?}"
    );
}

/// Report-only: the reverse direction the gate does not prove.
#[test]
fn claimed_actions_advertised_as_having_no_default() {
    let unadvertised: Vec<&str> = AVAILABLE_ACTIONS
        .iter()
        .filter(|(_, _, default)| default.is_none())
        .map(|(action, _, _)| *action)
        .collect();

    let mut found: Vec<String> = claims::claim_chain()
        .iter()
        .filter(|rule| unadvertised.contains(&rule.action.as_str()))
        .map(|rule| format!("{} ({})", rule.action, rule.source))
        .collect();
    found.sort();
    found.dedup();

    let mut expected: Vec<String> = CLAIMED_BUT_ADVERTISED_AS_NONE
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    expected.sort();

    assert_eq!(
        found, expected,
        "the set of actions that are dispatchable but advertised as having no \
         default changed. This is under-advertisement, not a dispatch defect — \
         update the list deliberately."
    );
}

#[test]
fn chord_parser_rejects_what_it_does_not_understand() {
    assert!(claims::parse_chord("Cmd+Nonsense").is_err());
    assert!(claims::parse_chord("Cmd+").is_err());
    assert!(claims::parse_chord("Cmd+A+B").is_err());
    assert!(claims::parse_chord("Shift").is_err());

    let plus = claims::parse_chord("Cmd+Plus").expect("Plus is part of the vocabulary");
    let equal = claims::parse_chord("Cmd+Equal").expect("Equal is part of the vocabulary");
    assert_ne!(
        plus, equal,
        "`Plus` and `Equal` are different logical keys and must not collapse"
    );
}

/// `CmdOrCtrl` must resolve the way the keybinding matcher resolves it.
#[test]
fn cmd_or_ctrl_expands_per_platform() {
    let c = claims::parse_chord("CmdOrCtrl+Shift+B").expect("parses");
    if Platform::HOST.is_mac() {
        assert!(c.mods.sup && !c.mods.ctrl, "CmdOrCtrl is Cmd on macOS");
    } else {
        assert!(c.mods.ctrl && !c.mods.sup, "CmdOrCtrl is Ctrl elsewhere");
    }
    assert!(c.mods.shift);
}

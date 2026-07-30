//! What each keyboard dispatch layer *claims*, expressed as data.
//!
//! # Why this exists
//!
//! The third column of `AVAILABLE_ACTIONS` (in `par-term-settings-ui`) is shown
//! to the user as an action's default chord. It was hand-maintained with nothing
//! behind it, and it was wrong three separate times in one session:
//! `cycle_cursor_style` advertised a chord the settings layer claims first,
//! `duplicate_tab` advertised a chord nothing was bound to, and the font-size
//! rows named the wrong modifier on macOS.
//!
//! A cross-check against `Config::default().keybindings` cannot catch that
//! class: only 8 of 32 advertised macOS chords (3 of 26 elsewhere) correspond to
//! a config default. The rest come from the hardcoded layers below, and none of
//! the three defects had a config default.
//!
//! The invariant worth checking is therefore: *every advertised chord is
//! dispatched, by the action that advertises it, and no earlier layer claims it
//! first*. That needs the layers to expose their claims as data, which is what
//! this module is.
//!
//! # Why it is declarative and not a simulation
//!
//! The obvious alternative — synthesise a `winit::event::KeyEvent` per chord and
//! run it through the real chain — is not available. `KeyEvent` has a private
//! heap-owning field; fabricating one in a test previously produced a SIGSEGV on
//! Linux (see `par-term` project memory, "foreign struct fabrication UB"), and
//! `WindowState` needs a window and a GPU device besides. Every claim here is
//! therefore either derived from live data or mirrored from the handler by hand.
//! The `chord_tests` module states exactly which is which.
//!
//! # Precedence
//!
//! [`claim_chain`] returns the sources in the order `handle_key_event` consults
//! them. The first source whose claim admits a chord is the one that runs; every
//! later source never sees the key.

// Most of this module is a *declaration* of behaviour that only `chord_tests`
// reads; four layers drive their matching from it at runtime and the rest would
// otherwise read as dead. Under `--tests` — which is what `make lint` builds —
// the lint stays on, so anything reachable from `LAYER_CLAIMS` or `claim_chain`
// is still checked for orphans. Platform-gated sources (`MACOS_APP_MENU`) are
// only reachable on their own platform, so they are only checked there.
#![cfg_attr(not(test), allow(dead_code))]

use muda::accelerator::{Accelerator, Modifiers as MudaModifiers};
use winit::keyboard::{Key, ModifiersState, NamedKey};

// ───────────────────────── chord model ─────────────────────────

/// The four modifiers par-term distinguishes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// Super/Cmd/Win.
    pub sup: bool,
}

pub(crate) const fn mods(ctrl: bool, alt: bool, shift: bool, sup: bool) -> Mods {
    Mods {
        ctrl,
        alt,
        shift,
        sup,
    }
}

pub(super) const NO_MODS: Mods = mods(false, false, false, false);

/// A modifier *predicate*, not a modifier set.
///
/// Layers are not uniform: `primary_modifier` demands an exact set (it excludes
/// the cross modifier and Alt on purpose), while the font-size branches in
/// `utility.rs` only test that one modifier is held and ignore the rest. Both
/// shapes have to be representable or the precedence answer is wrong.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct ModSpec {
    /// Must all be held.
    pub required: Mods,
    /// Must all be absent.
    pub forbidden: Mods,
}

impl ModSpec {
    /// Held modifiers must be exactly `m` — the `primary_modifier` shape, and
    /// how the keybinding registry and menu accelerators match.
    pub(crate) const fn exact(m: Mods) -> Self {
        Self {
            required: m,
            forbidden: mods(!m.ctrl, !m.alt, !m.shift, !m.sup),
        }
    }

    /// `required` must be held; anything not in `forbidden` is tolerated.
    pub(crate) const fn loose(required: Mods, forbidden: Mods) -> Self {
        Self {
            required,
            forbidden,
        }
    }

    pub(crate) fn admits(&self, m: Mods) -> bool {
        let has_required = (!self.required.ctrl || m.ctrl)
            && (!self.required.alt || m.alt)
            && (!self.required.shift || m.shift)
            && (!self.required.sup || m.sup);
        let clear_of_forbidden = (!self.forbidden.ctrl || !m.ctrl)
            && (!self.forbidden.alt || !m.alt)
            && (!self.forbidden.shift || !m.shift)
            && (!self.forbidden.sup || !m.sup);
        has_required && clear_of_forbidden
    }
}

/// The non-modifier half of a chord. Characters are stored upper-cased.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ChordKey {
    Char(char),
    Named(NamedKey),
}

/// A fully resolved key combination.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Chord {
    pub mods: Mods,
    pub key: ChordKey,
}

/// Which build this is. Selects between the two shapes every layer has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Platform {
    MacOs,
    Windows,
    Linux,
}

impl Platform {
    pub(crate) const HOST: Platform = if cfg!(target_os = "macos") {
        Platform::MacOs
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Linux
    };

    pub(crate) const fn is_mac(self) -> bool {
        matches!(self, Platform::MacOs)
    }

    /// Whether muda attaches a real menu bar whose accelerators intercept keys
    /// before winit ever delivers them. Linux/BSD get the in-app egui menu
    /// instead, which only *draws* accelerator labels — see `crate::menu`.
    pub(crate) const fn has_native_menu(self) -> bool {
        matches!(self, Platform::MacOs | Platform::Windows)
    }
}

// ───────────────────────── claim data ─────────────────────────

/// One chord (or set of interchangeable keys) that a layer claims.
pub(crate) struct Claim {
    /// The action the layer performs. Names that exist in `AVAILABLE_ACTIONS`
    /// are used verbatim; internal-only behaviour is prefixed `internal:` so it
    /// can never accidentally satisfy an advertised row.
    pub action: &'static str,
    /// Modifier predicate on macOS. `None` = not claimed there.
    pub mac: Option<ModSpec>,
    /// Modifier predicate on Windows and Linux. `None` = not claimed there.
    pub other: Option<ModSpec>,
    /// Any one of these keys satisfies the claim.
    pub keys: &'static [ChordKey],
}

impl Claim {
    fn spec(&self, p: Platform) -> Option<ModSpec> {
        if p.is_mac() { self.mac } else { self.other }
    }

    fn admits(&self, p: Platform, chord: Chord) -> bool {
        self.spec(p).is_some_and(|s| s.admits(chord.mods)) && self.keys.contains(&chord.key)
    }

    /// Runtime form: does this claim admit the live event?
    ///
    /// Layers that call this are *driven* by their declaration, so the
    /// declaration cannot drift from the behaviour.
    pub(crate) fn matches_event(&self, state: &ModifiersState, key: &Key) -> bool {
        let Some(chord_key) = event_chord_key(key) else {
            return false;
        };
        self.admits(
            Platform::HOST,
            Chord {
                mods: event_mods(state),
                key: chord_key,
            },
        )
    }
}

pub(crate) fn event_mods(state: &ModifiersState) -> Mods {
    mods(
        state.control_key(),
        state.alt_key(),
        state.shift_key(),
        state.super_key(),
    )
}

fn event_chord_key(key: &Key) -> Option<ChordKey> {
    match key {
        Key::Character(c) => {
            let mut chars = c.chars();
            let first = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            Some(ChordKey::Char(first.to_ascii_uppercase()))
        }
        Key::Named(n) => Some(ChordKey::Named(*n)),
        _ => None,
    }
}

// Shorthands for the two modifier shapes `crate::platform` provides.
pub(super) const PRIMARY_MAC: ModSpec = ModSpec::exact(mods(false, false, false, true));
// No layer uses the bare `primary_modifier` shape off macOS — every shortcut
// that is Cmd+key there is Ctrl+Shift+key elsewhere — so there is no
// `PRIMARY_OTHER`.
pub(super) const PRIMARY_SHIFT_MAC: ModSpec = ModSpec::exact(mods(false, false, true, true));
pub(super) const PRIMARY_SHIFT_OTHER: ModSpec = ModSpec::exact(mods(true, false, true, false));
/// No modifier is required and none is rejected — an unguarded `matches!` on the
/// logical key, which is how the function-key layers are written.
pub(super) const ANY_MODS: ModSpec = ModSpec::loose(NO_MODS, NO_MODS);

pub(super) const fn ch(c: char) -> ChordKey {
    ChordKey::Char(c)
}
pub(super) const fn named(n: NamedKey) -> ChordKey {
    ChordKey::Named(n)
}
pub(super) mod tables;
pub(super) use tables::*;

// ───────────────────────── the chain ─────────────────────────

/// A claim resolved for one platform, tagged with where it came from.
pub(crate) struct Rule {
    pub source: &'static str,
    pub action: String,
    pub spec: ModSpec,
    pub keys: Vec<ChordKey>,
}

impl Rule {
    fn admits(&self, chord: Chord) -> bool {
        self.spec.admits(chord.mods) && self.keys.contains(&chord.key)
    }
}

/// Every claim source in precedence order, for the platform being built.
///
/// Sources, highest precedence first:
///
/// 1. `macos_app_menu` — NSApp menu key equivalents (macOS only).
/// 2. `native_menu` — the rest of the muda menu bar (macOS and Windows only).
/// 3. `config_keybindings` — `Config::default().keybindings`, consulted by
///    `handle_key_event` before any hardcoded layer.
/// 4. Everything in [`LAYER_CLAIMS`], in its declared order.
///
/// Excluded, because what they claim is only knowable at runtime: the tmux
/// prefix, the custom-action prefix, and `profile_shortcuts`.
pub(crate) fn claim_chain() -> Vec<Rule> {
    let p = Platform::HOST;
    let mut rules = Vec::new();

    if p.is_mac() {
        push_claims(&mut rules, p, "macos_app_menu", MACOS_APP_MENU);
    }

    if p.has_native_menu() {
        rules.extend(menu_rules(p));
    }

    rules.extend(config_default_rules());

    for (name, claims) in LAYER_CLAIMS {
        push_claims(&mut rules, p, name, claims);
    }

    rules
}

fn push_claims(out: &mut Vec<Rule>, p: Platform, source: &'static str, claims: &[Claim]) {
    for claim in claims {
        if let Some(spec) = claim.spec(p) {
            out.push(Rule {
                source,
                action: claim.action.to_string(),
                spec,
                keys: claim.keys.to_vec(),
            });
        }
    }
}

/// Derived from the live menu model — not a mirror.
fn menu_rules(p: Platform) -> Vec<Rule> {
    let mut rules = Vec::new();
    for section in crate::menu::model::menu_model(p.is_mac()) {
        for entry in &section.entries {
            let crate::menu::model::MenuEntry::Item(spec) = entry else {
                continue;
            };
            let Some(accel) = &spec.accelerator else {
                continue;
            };
            rules.push(Rule {
                source: "native_menu",
                action: menu_action_name(spec.action),
                spec: ModSpec::exact(accelerator_mods(accel)),
                keys: vec![accelerator_key(accel)],
            });
        }
    }
    rules
}

/// Derived from the shipped defaults — not a mirror.
///
/// The registry runs before every hardcoded layer, and matches modifiers
/// exactly (`par_term_keybindings::matcher::modifiers_match`).
fn config_default_rules() -> Vec<Rule> {
    par_term_config::Config::default()
        .keybindings
        .iter()
        .map(|kb| {
            let chord = parse_chord(&kb.key).unwrap_or_else(|e| {
                panic!(
                    "shipped default keybinding {:?} does not parse: {e}",
                    kb.key
                )
            });
            Rule {
                source: "config_keybindings",
                action: kb.action.clone(),
                spec: ModSpec::exact(chord.mods),
                keys: vec![chord.key],
            }
        })
        .collect()
}

/// The first source that would consume `chord`, and the action it runs.
pub(crate) fn first_claimer(chord: Chord) -> Option<(&'static str, String)> {
    claim_chain()
        .into_iter()
        .find(|rule| rule.admits(chord))
        .map(|rule| (rule.source, rule.action))
}

// ───────────────────────── menu accelerator decoding ─────────────────────────

fn accelerator_mods(accel: &Accelerator) -> Mods {
    let m = accel.modifiers();
    mods(
        m.contains(MudaModifiers::CONTROL),
        m.contains(MudaModifiers::ALT),
        m.contains(MudaModifiers::SHIFT),
        // `Accelerator::new` normalises META to SUPER; accept either.
        m.contains(MudaModifiers::SUPER) || m.contains(MudaModifiers::META),
    )
}

/// Panics on an unmapped `Code` rather than skipping it — a silently ignored
/// accelerator would make the menu look like it claims nothing.
fn accelerator_key(accel: &Accelerator) -> ChordKey {
    let code = accel.key();
    let raw = format!("{code:?}");
    match raw.as_str() {
        "Comma" => ch(','),
        "Period" => ch('.'),
        "Equal" => ch('='),
        "Minus" => ch('-'),
        "BracketLeft" => ch('['),
        "BracketRight" => ch(']'),
        "Space" => named(NamedKey::Space),
        "Enter" => named(NamedKey::Enter),
        "Tab" => named(NamedKey::Tab),
        "ArrowLeft" => named(NamedKey::ArrowLeft),
        "ArrowRight" => named(NamedKey::ArrowRight),
        "ArrowUp" => named(NamedKey::ArrowUp),
        "ArrowDown" => named(NamedKey::ArrowDown),
        other => {
            if let Some(letter) = other.strip_prefix("Key") {
                return ch(letter.chars().next().expect("KeyX code has a letter"));
            }
            if let Some(digit) = other.strip_prefix("Digit") {
                return ch(digit.chars().next().expect("DigitN code has a digit"));
            }
            if let Some(named_key) = parse_named_word(other) {
                return named_key;
            }
            panic!("menu accelerator uses an unmapped key code {other:?}");
        }
    }
}

/// Maps a menu command to the `AVAILABLE_ACTIONS` action it performs.
///
/// Exhaustive on purpose: a new `MenuAction` must state whether it corresponds
/// to an advertised action or is menu-only.
fn menu_action_name(action: crate::menu::MenuAction) -> String {
    use crate::menu::MenuAction as A;
    match action {
        A::NewWindow => "new_window".into(),
        A::CloseWindow => "close_window".into(),
        A::Quit => "quit".into(),
        A::ManageProfiles => "internal:manage_profiles".into(),
        A::ToggleProfileDrawer => "toggle_profile_drawer".into(),
        A::OpenProfile(_) => "internal:open_profile".into(),
        A::NewTab => "new_tab".into(),
        A::CloseTab => "close_tab".into(),
        A::NextTab => "next_tab".into(),
        A::PreviousTab => "prev_tab".into(),
        A::SwitchToTab(n) => format!("switch_to_tab_{n}"),
        A::MoveTabLeft => "move_tab_left".into(),
        A::MoveTabRight => "move_tab_right".into(),
        A::DuplicateTab => "duplicate_tab".into(),
        A::Copy => "internal:copy".into(),
        A::Paste => "internal:paste".into(),
        A::SelectAll => "select_all".into(),
        A::ClearScrollback => "clear_scrollback".into(),
        A::ClipboardHistory => "toggle_clipboard_history".into(),
        A::ToggleFullscreen => "toggle_fullscreen".into(),
        A::MaximizeVertically => "maximize_vertically".into(),
        A::IncreaseFontSize => "increase_font_size".into(),
        A::DecreaseFontSize => "decrease_font_size".into(),
        A::ResetFontSize => "reset_font_size".into(),
        A::ToggleFpsOverlay => "toggle_fps_overlay".into(),
        A::OpenSettings => "open_settings".into(),
        A::Minimize => "internal:minimize".into(),
        A::Zoom => "internal:zoom".into(),
        A::ShowHelp => "toggle_help".into(),
        A::About => "internal:about".into(),
        A::SaveArrangement => "save_arrangement".into(),
        A::InstallShellIntegrationRemote => "internal:install_shell_integration_remote".into(),
        A::ToggleBackgroundShader => "toggle_background_shader".into(),
        A::ToggleCursorShader => "toggle_cursor_shader".into(),
        A::ReloadConfig => "reload_config".into(),
    }
}

// ───────────────────────── chord parsing ─────────────────────────

/// Parse a chord written the way `AVAILABLE_ACTIONS` and `Config` write them.
///
/// Deliberately strict: an unrecognised spelling is an error, never a skipped
/// row. A permissive parser is the easiest way to launder a wrong advertised
/// string past the gate.
///
/// `CmdOrCtrl` expands to Cmd on macOS and Ctrl elsewhere, matching
/// `par_term_keybindings::platform::resolve_cmd_or_ctrl`.
pub(crate) fn parse_chord(s: &str) -> Result<Chord, String> {
    let parts: Vec<&str> = s.split('+').map(str::trim).collect();
    if parts.iter().any(|p| p.is_empty()) {
        return Err(format!("{s:?} has an empty component"));
    }

    let mut m = NO_MODS;
    let mut key: Option<ChordKey> = None;

    for part in parts {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => m.ctrl = true,
            "alt" | "option" => m.alt = true,
            "shift" => m.shift = true,
            "cmd" | "command" | "meta" | "super" | "win" => m.sup = true,
            "cmdorctrl" => {
                if Platform::HOST.is_mac() {
                    m.sup = true;
                } else {
                    m.ctrl = true;
                }
            }
            _ => {
                if key.is_some() {
                    return Err(format!("{s:?} names more than one key"));
                }
                key = Some(
                    parse_key_word(part)
                        .ok_or_else(|| format!("{s:?} uses an unrecognised key name {part:?}"))?,
                );
            }
        }
    }

    let key = key.ok_or_else(|| format!("{s:?} has modifiers but no key"))?;
    Ok(Chord { mods: m, key })
}

fn parse_key_word(s: &str) -> Option<ChordKey> {
    if let Some(k) = parse_named_word(s) {
        return Some(k);
    }
    match s.to_ascii_lowercase().as_str() {
        "plus" => return Some(ch('+')),
        "minus" => return Some(ch('-')),
        "comma" => return Some(ch(',')),
        "equal" | "equals" => return Some(ch('=')),
        _ => {}
    }
    let mut chars = s.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    Some(ch(first.to_ascii_uppercase()))
}

fn parse_named_word(s: &str) -> Option<ChordKey> {
    let n = match s.to_ascii_lowercase().as_str() {
        "f1" => NamedKey::F1,
        "f2" => NamedKey::F2,
        "f3" => NamedKey::F3,
        "f4" => NamedKey::F4,
        "f5" => NamedKey::F5,
        "f6" => NamedKey::F6,
        "f7" => NamedKey::F7,
        "f8" => NamedKey::F8,
        "f9" => NamedKey::F9,
        "f10" => NamedKey::F10,
        "f11" => NamedKey::F11,
        "f12" => NamedKey::F12,
        "enter" | "return" => NamedKey::Enter,
        "escape" | "esc" => NamedKey::Escape,
        "space" => NamedKey::Space,
        "tab" => NamedKey::Tab,
        "backspace" => NamedKey::Backspace,
        "delete" | "del" => NamedKey::Delete,
        "insert" | "ins" => NamedKey::Insert,
        "home" => NamedKey::Home,
        "end" => NamedKey::End,
        "pageup" | "pgup" => NamedKey::PageUp,
        "pagedown" | "pgdn" => NamedKey::PageDown,
        "up" | "arrowup" => NamedKey::ArrowUp,
        "down" | "arrowdown" => NamedKey::ArrowDown,
        "left" | "arrowleft" => NamedKey::ArrowLeft,
        "right" | "arrowright" => NamedKey::ArrowRight,
        _ => return None,
    };
    Some(named(n))
}

//! Registry-sourced menu accelerators (UX.md K2, MN3).
//!
//! Every registry-backed item takes its chord from the live
//! [`KeybindingRegistry`] through [`MenuAction::keybinding_action`]: a
//! rebound action shows its new chord, and an unbound action — or one whose
//! chord a `pass_to_terminal` row or another action claims — shows none, so
//! the native menu releases the chord (MN3). Only menu-only commands (Copy,
//! Paste, Minimize, which have no registry action) keep a fixed accelerator.
//!
//! An action that appears in two places (the profile drawer under both
//! Profiles and View) carries its chord on one item only — the one not
//! marked [`MenuItemSpec::alias`], else the first: two native items
//! registering one key equivalent would fire whichever AppKit finds first.
//!
//! [`MenuAction::keybinding_action`]: super::MenuAction::keybinding_action

use super::model::{MenuEntry, MenuItemSpec, MenuSection};
use muda::accelerator::{Accelerator, Code, Modifiers};
use par_term_keybindings::KeybindingRegistry;
use par_term_keybindings::parser::{KeyCombo, ParsedKey};
use std::collections::HashSet;
use winit::keyboard::NamedKey;

/// Set every registry-backed item's accelerator from `registry`.
pub fn apply_registry_accelerators(sections: &mut [MenuSection], registry: &KeybindingRegistry) {
    let mut seen: HashSet<String> = HashSet::new();
    for section in sections {
        apply_entries(&mut section.entries, registry, &mut seen);
    }
}

fn apply_entries(
    entries: &mut [MenuEntry],
    registry: &KeybindingRegistry,
    seen: &mut HashSet<String>,
) {
    for entry in entries {
        match entry {
            MenuEntry::Item(spec) => apply_item(spec, registry, seen),
            MenuEntry::Submenu(sub) => apply_entries(&mut sub.entries, registry, seen),
            _ => {}
        }
    }
}

fn apply_item(spec: &mut MenuItemSpec, registry: &KeybindingRegistry, seen: &mut HashSet<String>) {
    let Some(action) = spec.action.keybinding_action() else {
        return;
    };
    if spec.alias || !seen.insert(action.to_string()) {
        spec.accelerator = None;
        return;
    }
    // A shifted-symbol chord (iTerm2's Cmd+Shift+: for command history) has
    // no key code, so the item shows no chord; the registry still runs it.
    // Logged at debug: the menu is rebuilt on every binding change.
    spec.accelerator = registry.chord_for_action(&action).and_then(|combo| {
        let accelerator = accelerator_from_combo(&combo);
        if accelerator.is_none() {
            log::debug!(
                "menu: the chord bound to {action} has no menu key equivalent; the menu shows none"
            );
        }
        accelerator
    });
}

/// Parse a registry chord string into the muda accelerator the menu needs.
///
/// `None` when the chord does not parse or names a key the menu cannot
/// express (physical key codes are matcher-only).
#[cfg(test)]
pub(crate) fn accelerator_from_chord(chord: &str) -> Option<Accelerator> {
    accelerator_from_combo(&par_term_keybindings::parser::parse_key_combo(chord).ok()?)
}

/// Convert a parsed registry combo into a muda accelerator. `CmdOrCtrl` is
/// folded to the platform modifier first (`KeyCombo::platform_normalized`),
/// so both spellings of one chord produce the same accelerator.
pub(crate) fn accelerator_from_combo(combo: &KeyCombo) -> Option<Accelerator> {
    let combo = combo.clone().platform_normalized();
    let mut mods = Modifiers::empty();
    if combo.modifiers.ctrl {
        mods |= Modifiers::CONTROL;
    }
    if combo.modifiers.alt {
        mods |= Modifiers::ALT;
    }
    if combo.modifiers.shift {
        mods |= Modifiers::SHIFT;
    }
    if combo.modifiers.super_key {
        mods |= Modifiers::META;
    }
    let code = match combo.key {
        ParsedKey::Character(c) => char_code(c)?,
        ParsedKey::Named(named) => named_code(named)?,
        ParsedKey::Physical(_) => return None,
    };
    Some(Accelerator::new(mods, code))
}

/// Menu key code for a character chord key — the vocabulary the default
/// bindings spell (letters, digits, and the punctuation a chord can name).
/// Mirrors the inverse mapping in `key_handler::claims::accelerator_key`.
/// A shifted symbol (`:`, `{`) has no key code of its own and yields `None`.
fn char_code(c: char) -> Option<Code> {
    Some(match c.to_ascii_uppercase() {
        '0' => Code::Digit0,
        '1' => Code::Digit1,
        '2' => Code::Digit2,
        '3' => Code::Digit3,
        '4' => Code::Digit4,
        '5' => Code::Digit5,
        '6' => Code::Digit6,
        '7' => Code::Digit7,
        '8' => Code::Digit8,
        '9' => Code::Digit9,
        'A' => Code::KeyA,
        'B' => Code::KeyB,
        'C' => Code::KeyC,
        'D' => Code::KeyD,
        'E' => Code::KeyE,
        'F' => Code::KeyF,
        'G' => Code::KeyG,
        'H' => Code::KeyH,
        'I' => Code::KeyI,
        'J' => Code::KeyJ,
        'K' => Code::KeyK,
        'L' => Code::KeyL,
        'M' => Code::KeyM,
        'N' => Code::KeyN,
        'O' => Code::KeyO,
        'P' => Code::KeyP,
        'Q' => Code::KeyQ,
        'R' => Code::KeyR,
        'S' => Code::KeyS,
        'T' => Code::KeyT,
        'U' => Code::KeyU,
        'V' => Code::KeyV,
        'W' => Code::KeyW,
        'X' => Code::KeyX,
        'Y' => Code::KeyY,
        'Z' => Code::KeyZ,
        '[' => Code::BracketLeft,
        ']' => Code::BracketRight,
        '=' => Code::Equal,
        '-' => Code::Minus,
        ',' => Code::Comma,
        '.' => Code::Period,
        _ => return None,
    })
}

/// Menu key code for a named key the bindings use.
fn named_code(named: NamedKey) -> Option<Code> {
    Some(match named {
        NamedKey::F1 => Code::F1,
        NamedKey::F2 => Code::F2,
        NamedKey::F3 => Code::F3,
        NamedKey::F4 => Code::F4,
        NamedKey::F5 => Code::F5,
        NamedKey::F6 => Code::F6,
        NamedKey::F7 => Code::F7,
        NamedKey::F8 => Code::F8,
        NamedKey::F9 => Code::F9,
        NamedKey::F10 => Code::F10,
        NamedKey::F11 => Code::F11,
        NamedKey::F12 => Code::F12,
        NamedKey::ArrowLeft => Code::ArrowLeft,
        NamedKey::ArrowRight => Code::ArrowRight,
        NamedKey::ArrowUp => Code::ArrowUp,
        NamedKey::ArrowDown => Code::ArrowDown,
        NamedKey::Space => Code::Space,
        NamedKey::Enter => Code::Enter,
        NamedKey::Tab => Code::Tab,
        NamedKey::Home => Code::Home,
        NamedKey::End => Code::End,
        NamedKey::PageUp => Code::PageUp,
        NamedKey::PageDown => Code::PageDown,
        NamedKey::Insert => Code::Insert,
        NamedKey::Delete => Code::Delete,
        NamedKey::Backspace => Code::Backspace,
        _ => return None,
    })
}

#[cfg(test)]
#[path = "registry_accel_tests.rs"]
mod tests;

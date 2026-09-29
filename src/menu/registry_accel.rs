//! Registry-sourced menu accelerators (UX.md K2).
//!
//! The menu model's hardcoded accelerators are only the fallback: when the
//! model is built, [`apply_registry_accelerators`] re-reads each item's chord
//! from the keybinding registry through [`MenuAction::keybinding_action`], so
//! a rebind in `config.yaml` — or a future default change — is what the menu
//! displays. One source of truth for chords: the registry. Items whose action
//! has no registry binding (Copy, Paste, …) keep their hardcoded accelerator.

use super::model::{MenuEntry, MenuSection};
use muda::accelerator::{Accelerator, Code, Modifiers};
use par_term_config::KeyBinding;
use par_term_keybindings::parser::{ParsedKey, parse_key_combo};
use std::collections::HashMap;
use winit::keyboard::NamedKey;

/// Overwrite every menu item's accelerator with its registry binding.
///
/// The first binding for an action wins, mirroring
/// `KeybindingRegistry::from_config`'s first-wins duplicate handling. Items
/// whose action is unbound keep the model's hardcoded accelerator; so does a
/// binding whose chord does not parse, with a warning — a menu that silently
/// dropped an accelerator would look like it claims nothing.
pub fn apply_registry_accelerators(sections: &mut [MenuSection], keybindings: &[KeyBinding]) {
    let mut by_action: HashMap<&str, &str> = HashMap::new();
    for binding in keybindings {
        by_action
            .entry(binding.action.as_str())
            .or_insert(binding.key.as_str());
    }
    for section in sections {
        for entry in &mut section.entries {
            let MenuEntry::Item(spec) = entry else {
                continue;
            };
            let Some(action) = spec.action.keybinding_action() else {
                continue;
            };
            let Some(chord) = by_action.get(action.as_ref()) else {
                continue;
            };
            match accelerator_from_chord(chord) {
                Some(accelerator) => spec.accelerator = Some(accelerator),
                None => log::warn!(
                    "menu: registry chord {chord:?} for action {action} does not parse; keeping the hardcoded accelerator"
                ),
            }
        }
    }
}

/// Parse a registry chord string into the muda accelerator the menu needs.
///
/// `CmdOrCtrl` is folded to the platform modifier first
/// (`KeyCombo::platform_normalized`), so both spellings of one chord produce
/// the same accelerator. `None` when the chord does not parse or names a key
/// the menu cannot express (physical key codes are matcher-only).
pub(crate) fn accelerator_from_chord(chord: &str) -> Option<Accelerator> {
    let combo = parse_key_combo(chord).ok()?.platform_normalized();
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

/// Menu key code for a named key the default bindings use.
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
mod tests {
    use super::*;
    use crate::menu::model;
    use std::collections::HashSet;

    fn defaults() -> Vec<KeyBinding> {
        par_term_config::Config::default().keybindings.clone()
    }

    fn items(sections: &[MenuSection]) -> Vec<&super::super::model::MenuItemSpec> {
        sections
            .iter()
            .flat_map(|section| &section.entries)
            .filter_map(|entry| match entry {
                MenuEntry::Item(spec) => Some(spec),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn chord_conversion_covers_the_default_vocabulary() {
        let f11 = accelerator_from_chord("F11").unwrap();
        assert!(f11.modifiers().is_empty());
        assert_eq!(f11.key(), Code::F11);

        let shift_bracket = accelerator_from_chord("Ctrl+Shift+]");
        let shift_bracket = shift_bracket.expect("Ctrl+Shift+] parses");
        assert!(shift_bracket.modifiers().contains(Modifiers::CONTROL));
        assert!(shift_bracket.modifiers().contains(Modifiers::SHIFT));
        assert_eq!(shift_bracket.key(), Code::BracketRight);

        let alt_digit = accelerator_from_chord("Alt+1").unwrap();
        assert!(alt_digit.modifiers().contains(Modifiers::ALT));
        assert_eq!(alt_digit.key(), Code::Digit1);

        let shift_f11 = accelerator_from_chord("Shift+F11").unwrap();
        assert_eq!(shift_f11.key(), Code::F11);

        assert_eq!(
            accelerator_from_chord("Ctrl+Shift+=").unwrap().key(),
            Code::Equal
        );
        assert_eq!(accelerator_from_chord("Ctrl+-").unwrap().key(), Code::Minus);
        assert_eq!(accelerator_from_chord("Cmd+,").unwrap().key(), Code::Comma);
        assert_eq!(
            accelerator_from_chord("CmdOrCtrl+Shift+Left")
                .unwrap()
                .key(),
            Code::ArrowLeft
        );

        // CmdOrCtrl resolves per platform; assert both directions explicitly.
        let primary = accelerator_from_chord("CmdOrCtrl+N").unwrap();
        if cfg!(target_os = "macos") {
            assert!(primary.modifiers().contains(Modifiers::META));
            assert!(!primary.modifiers().contains(Modifiers::CONTROL));
        } else {
            assert!(primary.modifiers().contains(Modifiers::CONTROL));
            assert!(!primary.modifiers().contains(Modifiers::META));
        }

        assert!(accelerator_from_chord("NotAChord+X").is_none());
        assert!(accelerator_from_chord("Ctrl+[KeyZ]").is_none());
    }

    /// UX P2 acceptance: a test enumerates the menus and asserts their
    /// accelerators equal the registry bindings.
    #[test]
    fn menu_accelerators_equal_default_registry_bindings() {
        let defaults = defaults();
        let by_action: HashMap<String, &str> = defaults
            .iter()
            .map(|kb| (kb.action.clone(), kb.key.as_str()))
            .collect();
        for has_native_app_menu in [false, true] {
            let sections = model::menu_model_with(has_native_app_menu, &defaults);
            let mut checked = 0usize;
            for spec in items(&sections) {
                let Some(action) = spec.action.keybinding_action() else {
                    continue;
                };
                let Some(chord) = by_action.get(action.as_ref()) else {
                    continue;
                };
                let expected = accelerator_from_chord(chord).unwrap_or_else(|| {
                    panic!("default chord {chord:?} for {action} does not parse")
                });
                let actual = spec.accelerator.as_ref().unwrap_or_else(|| {
                    panic!(
                        "{action} is bound to {chord:?} but its menu item carries no accelerator"
                    )
                });
                assert_eq!(
                    format!("{actual:?}"),
                    format!("{expected:?}"),
                    "{action}: menu accelerator != registry binding {chord:?}"
                );
                checked += 1;
            }
            assert!(
                checked >= 25,
                "only {checked} menu items cross-checked — the enumeration went wrong"
            );
        }
    }

    /// Every menu chord is either registry-backed or deliberately exempt: no
    /// item may keep a hardcoded accelerator for an action the registry could
    /// carry. Exempt today: Copy/Paste (no registry action) and the two
    /// settings entries (Preferences carries a platform menu convention;
    /// `open_settings` stays on the hardcoded F12 layer until K23–K28).
    #[test]
    fn hardcoded_menu_accelerators_are_registry_backed_or_exempt() {
        let defaults = defaults();
        let bound: HashSet<&str> = defaults.iter().map(|kb| kb.action.as_str()).collect();
        let exempt = ["copy", "paste", "preferences", "settings"];
        for has_native_app_menu in [false, true] {
            let sections = model::menu_model_with(has_native_app_menu, &defaults);
            for spec in items(&sections) {
                let Some(action) = spec.action.keybinding_action() else {
                    continue;
                };
                if spec.accelerator.is_some() && !bound.contains(action.as_ref()) {
                    assert!(
                        exempt.contains(&spec.id),
                        "menu item {:?} has an accelerator but {action} is not in the default registry",
                        spec.id
                    );
                }
            }
        }
    }

    #[test]
    fn a_rebound_action_shows_its_new_chord_and_an_unbound_one_keeps_its_fallback() {
        let mut bindings = defaults();
        bindings.retain(|kb| kb.action != "new_tab");
        bindings.push(KeyBinding {
            key: "Ctrl+Alt+F2".to_string(),
            action: "new_tab".to_string(),
        });
        let sections = model::menu_model_with(false, &bindings);
        let new_tab = items(&sections)
            .into_iter()
            .find(|spec| spec.id == "new_tab")
            .expect("new_tab menu item");
        let expected = accelerator_from_chord("Ctrl+Alt+F2").unwrap();
        assert_eq!(
            format!("{:?}", new_tab.accelerator.as_ref().unwrap()),
            format!("{expected:?}")
        );

        // Unbound: the hardcoded fallback survives untouched.
        bindings.retain(|kb| kb.action != "clear_scrollback");
        let sections = model::menu_model_with(false, &bindings);
        let clear = items(&sections)
            .into_iter()
            .find(|spec| spec.id == "clear_scrollback")
            .expect("clear_scrollback menu item");
        assert!(
            clear.accelerator.is_some(),
            "unbound action lost its hardcoded fallback accelerator"
        );
    }
}

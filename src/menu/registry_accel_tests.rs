use super::*;
use crate::menu::model::{self, all_items};
use par_term_config::KeyBinding;

fn defaults() -> Vec<KeyBinding> {
    par_term_config::Config::default().keybindings.clone()
}

fn chord_of(sections: &[MenuSection], id: &str) -> Option<Accelerator> {
    all_items(sections)
        .into_iter()
        .find(|spec| spec.id == id)
        .unwrap_or_else(|| panic!("no menu item {id:?}"))
        .accelerator
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

/// UX P2/MP2 acceptance: every registry-backed menu item's accelerator
/// equals the live binding — the action's primary chord, or none.
fn assert_menu_matches_registry(bindings: &[KeyBinding]) -> usize {
    let registry = KeybindingRegistry::from_config(bindings);
    let mut checked = 0usize;
    for has_native_app_menu in [false, true] {
        let sections = model::menu_model_with(has_native_app_menu, bindings);
        let mut carried: HashSet<String> = HashSet::new();
        for spec in all_items(&sections) {
            let Some(action) = spec.action.keybinding_action() else {
                continue;
            };
            let expected = registry
                .chord_for_action(&action)
                .and_then(|combo| accelerator_from_combo(&combo));
            if spec.alias || carried.contains(action.as_ref()) {
                assert!(
                    spec.accelerator.is_none(),
                    "{}: a second home for {action} must not repeat the chord",
                    spec.id
                );
                continue;
            }
            carried.insert(action.to_string());
            assert_eq!(
                spec.accelerator, expected,
                "{}: menu accelerator != registry binding for {action}",
                spec.id
            );
            checked += 1;
        }
    }
    checked
}

#[test]
fn menu_accelerators_equal_default_registry_bindings() {
    let checked = assert_menu_matches_registry(&defaults());
    assert!(
        checked >= 150,
        "only {checked} menu items cross-checked — the enumeration went wrong"
    );
}

/// UX.md MN3: after a rebind the menu shows the new chord; an unbound action
/// shows none (the native menu releases it); a chord taken by a
/// `pass_to_terminal` row is released too.
#[test]
fn a_rebound_action_shows_its_new_chord_and_an_unbound_one_shows_none() {
    let mut bindings = defaults();
    bindings.retain(|kb| kb.action != "new_tab");
    bindings.insert(
        0,
        KeyBinding {
            key: "Ctrl+Alt+F2".to_string(),
            action: "new_tab".to_string(),
        },
    );
    let sections = model::menu_model_with(false, &bindings);
    assert_eq!(
        chord_of(&sections, "new_tab"),
        accelerator_from_chord("Ctrl+Alt+F2")
    );

    // Unbound: no chord at all, not the old one.
    bindings.retain(|kb| kb.action != "clear_scrollback");
    let sections = model::menu_model_with(false, &bindings);
    assert_eq!(chord_of(&sections, "clear_scrollback"), None);

    // A pass_to_terminal row ahead of the default claims the chord for the
    // shell; the menu must let it go.
    let mut bindings = defaults();
    let reopen_chord = bindings
        .iter()
        .find(|kb| kb.action == "split_right")
        .map(|kb| kb.key.clone())
        .expect("split_right ships a default");
    bindings.insert(
        0,
        KeyBinding {
            key: reopen_chord,
            action: par_term_keybindings::PASS_TO_TERMINAL.to_string(),
        },
    );
    let sections = model::menu_model_with(false, &bindings);
    assert_eq!(chord_of(&sections, "split_right"), None);

    assert_menu_matches_registry(&bindings);
}

/// Every chord the model hardcodes belongs to a menu-only command (no
/// registry action) — a registry action's chord always comes from the
/// registry.
#[test]
fn hardcoded_accelerators_are_menu_only() {
    let no_bindings: Vec<KeyBinding> = Vec::new();
    for has_native_app_menu in [false, true] {
        let sections = model::menu_model_with(has_native_app_menu, &no_bindings);
        for spec in all_items(&sections) {
            if spec.accelerator.is_some() {
                assert!(
                    spec.action.keybinding_action().is_none(),
                    "{} keeps a hardcoded chord for a registry action",
                    spec.id
                );
            }
        }
    }
}

/// Every chord the shipped defaults bind to a menu item converts to a menu
/// key equivalent the native backend can register (a `None` here would show
/// the item without its chord).
#[test]
fn every_default_menu_chord_has_a_key_equivalent() {
    let registry = KeybindingRegistry::from_config(&defaults());
    let mut missing = Vec::new();
    for spec in all_items(&model::menu_model(false)) {
        let Some(action) = spec.action.keybinding_action() else {
            continue;
        };
        if let Some(combo) = registry.chord_for_action(&action)
            && accelerator_from_combo(&combo).is_none()
        {
            missing.push(format!("{action}: {combo:?}"));
        }
    }
    // Cmd+Shift+: (iTerm2's command history) is spelled with the shifted
    // character, which has no key code of its own.
    missing.retain(|m| !m.starts_with("toggle_command_history"));
    assert!(missing.is_empty(), "{missing:?}");
}

//! Keybindings editor section.
//!
//! Contains the keybindings grid and the `capture_key_combo` / `display_key_combo`
//! utilities that are also re-used by `actions_tab` and `snippets_tab`.
//!
//! The `AVAILABLE_ACTIONS` lookup table lives in [`super::actions_table`].

use crate::SettingsUI;
use crate::search::SearchTag;
use crate::section::keyword_section;
use par_term_config::KeyBinding;
use std::collections::HashSet;

/// Words people search for an action by that are not in its name.
fn action_synonyms(action: &str) -> &'static [&'static str] {
    match action {
        "move_tab_to_new_window" => &["detach", "popout", "tear off"],
        "select_pane_hint" => &["pane hint"],
        _ => &[],
    }
}

use super::actions_table::AVAILABLE_ACTIONS;

/// Type alias for keybinding info: (index, action_name, display_name, custom_binding, default_key, is_custom)
type BindingInfo<'a> = (
    usize,
    &'a str,
    &'a str,
    Option<String>,
    Option<&'a str>,
    bool,
);

// ============================================================================
// Keybindings Section
// ============================================================================

pub(crate) fn show_keybindings_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "Keybindings",
        "input_keybindings",
        &["hotkey", "shortcut", "keybindings"],
        true,
        collapsed,
        |ui| {
            ui.label(
            "Configure custom keyboard shortcuts. Click 'Record' to capture a new key combination.",
        );
            ui.colored_label(
                egui::Color32::from_rgb(128, 128, 128),
                "Gray bindings are defaults. Custom bindings appear in white.",
            );
            ui.add_space(4.0);

            // Check for key events if recording
            if let Some(recording_idx) = settings.keybinding_recording_index {
                let recorded = capture_key_combo(ui);
                if let Some(combo) = recorded {
                    settings.keybinding_recorded_combo = Some(combo.clone());

                    if recording_idx < AVAILABLE_ACTIONS.len() {
                        let (action_name, _, _) = AVAILABLE_ACTIONS[recording_idx];

                        settings.keybinding_conflict =
                            settings.check_recorded_chord_conflict(&combo, action_name);

                        let binding_idx = settings
                            .config
                            .keybindings
                            .iter()
                            .position(|b| b.action == action_name);

                        if let Some(idx) = binding_idx {
                            settings.config.keybindings[idx].key = combo;
                        } else {
                            settings.config.keybindings.push(KeyBinding {
                                key: combo,
                                action: action_name.to_string(),
                            });
                        }

                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }

                    settings.keybinding_recording_index = None;
                }

                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    settings.keybinding_recording_index = None;
                    settings.keybinding_recorded_combo = None;
                    settings.keybinding_conflict = None;
                }
            }

            if let Some(conflict) = &settings.keybinding_conflict {
                ui.colored_label(
                    egui::Color32::RED,
                    format!("Keybinding conflict: {conflict}"),
                );
            }

            // Collect binding info
            let binding_info: Vec<BindingInfo<'_>> = AVAILABLE_ACTIONS
                .iter()
                .enumerate()
                .map(|(idx, (action_name, display_name, default_key))| {
                    let custom_binding = settings
                        .config
                        .keybindings
                        .iter()
                        .find(|b| b.action == *action_name)
                        .map(|b| b.key.clone());
                    let is_custom = custom_binding.is_some();
                    (
                        idx,
                        *action_name,
                        *display_name,
                        custom_binding,
                        *default_key,
                        is_custom,
                    )
                })
                .collect();

            let mut action_to_clear: Option<&str> = None;
            let mut start_recording: Option<usize> = None;
            let mut cancel_recording = false;

            ui.scope(|ui| {
                egui::Grid::new("input_keybindings_grid")
                    .num_columns(3)
                    .spacing([20.0, 8.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("Action");
                        ui.strong("Key Combo");
                        ui.strong("");
                        ui.end_row();

                        for (
                            idx,
                            action_name,
                            display_name,
                            custom_binding,
                            default_binding,
                            is_custom,
                        ) in &binding_info
                        {
                            let (binding_display, show_as_default) =
                                if let Some(custom) = custom_binding {
                                    (display_key_combo(custom), false)
                                } else if let Some(default) = default_binding {
                                    (display_key_combo(default), true)
                                } else {
                                    ("(not set)".to_string(), false)
                                };

                            ui.label(*display_name)
                                .search_tag(&[action_name])
                                .search_tag(action_synonyms(action_name));

                            let is_recording = settings.keybinding_recording_index == Some(*idx);
                            if is_recording {
                                ui.colored_label(
                                    egui::Color32::YELLOW,
                                    "Press key combo... (Esc to cancel)",
                                );
                            } else if show_as_default {
                                ui.colored_label(
                                    egui::Color32::from_rgb(128, 128, 128),
                                    egui::RichText::new(&binding_display).monospace(),
                                );
                            } else {
                                ui.monospace(&binding_display);
                            }

                            ui.horizontal(|ui| {
                                let button_text = if is_recording { "Cancel" } else { "Record" };
                                if ui.button(button_text).clicked() {
                                    if is_recording {
                                        cancel_recording = true;
                                    } else {
                                        start_recording = Some(*idx);
                                    }
                                }

                                if *is_custom && !is_recording && ui.button("Clear").clicked() {
                                    action_to_clear = Some(*action_name);
                                }
                            });

                            ui.end_row();
                        }
                    });
            });

            if cancel_recording {
                settings.keybinding_recording_index = None;
                settings.keybinding_recorded_combo = None;
                settings.keybinding_conflict = None;
            }

            if let Some(idx) = start_recording {
                settings.keybinding_recording_index = Some(idx);
                settings.keybinding_recorded_combo = None;
                settings.keybinding_conflict = None;
            }

            if let Some(action_name) = action_to_clear {
                settings
                    .config
                    .keybindings
                    .retain(|b| b.action != action_name);
                settings.has_changes = true;
                *changes_this_frame = true;
                settings.keybinding_conflict = None;
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            ui.label("Key combo format: modifiers and a key joined with \"+\"");
            #[cfg(target_os = "macos")]
            ui.label("Available modifiers: Cmd, Ctrl, Alt, Shift");
            #[cfg(not(target_os = "macos"))]
            ui.label("Available modifiers: Ctrl, Alt, Shift, Super");
        },
    );
}

// ============================================================================
// Key-combo capture utilities (also used by actions_tab and snippets_tab)
// ============================================================================

/// Normalise a key combo string for display by resolving the `CmdOrCtrl`
/// cross-platform alias to the platform-specific modifier name.
pub(crate) fn display_key_combo(combo: &str) -> String {
    // Resolve CmdOrCtrl for any user-configured bindings that still use it
    #[cfg(target_os = "macos")]
    {
        combo.replace("CmdOrCtrl", "Cmd")
    }
    #[cfg(not(target_os = "macos"))]
    {
        combo.replace("CmdOrCtrl", "Ctrl")
    }
}

/// Capture a keyboard combination from user input.
///
/// Returns the key combo string (e.g., "Ctrl+Shift+T") if a key was pressed,
/// or None if no valid key combo was detected.
pub fn capture_key_combo(ui: &egui::Ui) -> Option<String> {
    ui.input(|input| {
        let ctrl = input.modifiers.ctrl;
        let alt = input.modifiers.alt;
        let shift = input.modifiers.shift;
        let cmd = input.modifiers.mac_cmd || input.modifiers.command;

        for event in &input.events {
            if let egui::Event::Key {
                key, pressed: true, ..
            } = event
            {
                if matches!(
                    key,
                    egui::Key::Tab | egui::Key::Escape | egui::Key::Backspace | egui::Key::Delete
                ) {
                    continue;
                }

                let mut parts = Vec::new();

                #[cfg(target_os = "macos")]
                {
                    // Independent, not else-if: Cmd+Ctrl+… (the pane-swap
                    // defaults) must keep both modifiers or the chord can
                    // never be re-recorded (B18).
                    if cmd {
                        parts.push("CmdOrCtrl");
                    }
                    if ctrl {
                        parts.push("Ctrl");
                    }
                }

                #[cfg(not(target_os = "macos"))]
                {
                    if ctrl {
                        parts.push("CmdOrCtrl");
                    }
                    // Super cannot be recorded here: egui's Modifiers expose
                    // no super state off macOS (`command` mirrors `ctrl`), so
                    // `cmd` carries no extra information on this branch.
                    let _ = cmd;
                }

                if alt {
                    parts.push("Alt");
                }
                if shift {
                    parts.push("Shift");
                }

                let key_str = key_to_string(*key);
                if let Some(key_name) = key_str {
                    let is_fkey = key_name.starts_with('F') && key_name.len() <= 3;
                    if parts.is_empty() && !is_fkey {
                        continue;
                    }

                    parts.push(key_name);
                    return Some(parts.join("+"));
                }
            }
        }

        None
    })
}

/// Recordable keys and the chord spelling the keybinding parser accepts
/// for each. Keys absent here (Escape, Tab, Backspace, F13+, …) cannot be
/// recorded.
const RECORDABLE_KEYS: &[(egui::Key, &str)] = &[
    (egui::Key::A, "A"),
    (egui::Key::B, "B"),
    (egui::Key::C, "C"),
    (egui::Key::D, "D"),
    (egui::Key::E, "E"),
    (egui::Key::F, "F"),
    (egui::Key::G, "G"),
    (egui::Key::H, "H"),
    (egui::Key::I, "I"),
    (egui::Key::J, "J"),
    (egui::Key::K, "K"),
    (egui::Key::L, "L"),
    (egui::Key::M, "M"),
    (egui::Key::N, "N"),
    (egui::Key::O, "O"),
    (egui::Key::P, "P"),
    (egui::Key::Q, "Q"),
    (egui::Key::R, "R"),
    (egui::Key::S, "S"),
    (egui::Key::T, "T"),
    (egui::Key::U, "U"),
    (egui::Key::V, "V"),
    (egui::Key::W, "W"),
    (egui::Key::X, "X"),
    (egui::Key::Y, "Y"),
    (egui::Key::Z, "Z"),
    (egui::Key::Num0, "0"),
    (egui::Key::Num1, "1"),
    (egui::Key::Num2, "2"),
    (egui::Key::Num3, "3"),
    (egui::Key::Num4, "4"),
    (egui::Key::Num5, "5"),
    (egui::Key::Num6, "6"),
    (egui::Key::Num7, "7"),
    (egui::Key::Num8, "8"),
    (egui::Key::Num9, "9"),
    (egui::Key::F1, "F1"),
    (egui::Key::F2, "F2"),
    (egui::Key::F3, "F3"),
    (egui::Key::F4, "F4"),
    (egui::Key::F5, "F5"),
    (egui::Key::F6, "F6"),
    (egui::Key::F7, "F7"),
    (egui::Key::F8, "F8"),
    (egui::Key::F9, "F9"),
    (egui::Key::F10, "F10"),
    (egui::Key::F11, "F11"),
    (egui::Key::F12, "F12"),
    (egui::Key::ArrowUp, "Up"),
    (egui::Key::ArrowDown, "Down"),
    (egui::Key::ArrowLeft, "Left"),
    (egui::Key::ArrowRight, "Right"),
    (egui::Key::Home, "Home"),
    (egui::Key::End, "End"),
    (egui::Key::PageUp, "PageUp"),
    (egui::Key::PageDown, "PageDown"),
    (egui::Key::Enter, "Enter"),
    (egui::Key::Space, "Space"),
    (egui::Key::Insert, "Insert"),
    (egui::Key::Minus, "Minus"),
    (egui::Key::Plus, "Plus"),
    (egui::Key::Equals, "Equals"),
    (egui::Key::OpenBracket, "BracketLeft"),
    (egui::Key::CloseBracket, "BracketRight"),
    (egui::Key::Backslash, "Backslash"),
    (egui::Key::Semicolon, "Semicolon"),
    (egui::Key::Colon, "Colon"),
    (egui::Key::Comma, "Comma"),
    (egui::Key::Period, "Period"),
    (egui::Key::Slash, "Slash"),
    (egui::Key::Backtick, "Backquote"),
];

fn key_to_string(key: egui::Key) -> Option<&'static str> {
    RECORDABLE_KEYS
        .iter()
        .find(|&&(k, _)| k == key)
        .map(|&(_, name)| name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drive `capture_key_combo` headlessly: a modifier-state change followed
    /// by one synthetic key event carrying the same modifiers (the recorder
    /// reads both the frame's modifier state and the event's).
    fn record(mods: egui::Modifiers, key: egui::Key) -> Option<String> {
        let ctx = egui::Context::default();
        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::ModifiersChanged(mods));
        raw.events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: mods,
        });
        let mut recorded = None;
        let mut output = ctx.run_ui(raw, |ctx| {
            recorded = egui::CentralPanel::default()
                .show(ctx, |ui| capture_key_combo(ui))
                .inner;
        });
        // Headless runs have no renderer to apply the font texture delta;
        // epaint panics on drop if it is not cleared first.
        output.textures_delta.clear();
        recorded
    }

    /// Independent oracle for [`key_to_string`], by key family. Every
    /// `egui::Key` outside these lists must map to `None`.
    const LETTERS: &[(egui::Key, &str)] = &[
        (egui::Key::A, "A"),
        (egui::Key::B, "B"),
        (egui::Key::C, "C"),
        (egui::Key::D, "D"),
        (egui::Key::E, "E"),
        (egui::Key::F, "F"),
        (egui::Key::G, "G"),
        (egui::Key::H, "H"),
        (egui::Key::I, "I"),
        (egui::Key::J, "J"),
        (egui::Key::K, "K"),
        (egui::Key::L, "L"),
        (egui::Key::M, "M"),
        (egui::Key::N, "N"),
        (egui::Key::O, "O"),
        (egui::Key::P, "P"),
        (egui::Key::Q, "Q"),
        (egui::Key::R, "R"),
        (egui::Key::S, "S"),
        (egui::Key::T, "T"),
        (egui::Key::U, "U"),
        (egui::Key::V, "V"),
        (egui::Key::W, "W"),
        (egui::Key::X, "X"),
        (egui::Key::Y, "Y"),
        (egui::Key::Z, "Z"),
    ];
    const DIGITS: &[(egui::Key, &str)] = &[
        (egui::Key::Num0, "0"),
        (egui::Key::Num1, "1"),
        (egui::Key::Num2, "2"),
        (egui::Key::Num3, "3"),
        (egui::Key::Num4, "4"),
        (egui::Key::Num5, "5"),
        (egui::Key::Num6, "6"),
        (egui::Key::Num7, "7"),
        (egui::Key::Num8, "8"),
        (egui::Key::Num9, "9"),
    ];
    const FUNCTION: &[(egui::Key, &str)] = &[
        (egui::Key::F1, "F1"),
        (egui::Key::F2, "F2"),
        (egui::Key::F3, "F3"),
        (egui::Key::F4, "F4"),
        (egui::Key::F5, "F5"),
        (egui::Key::F6, "F6"),
        (egui::Key::F7, "F7"),
        (egui::Key::F8, "F8"),
        (egui::Key::F9, "F9"),
        (egui::Key::F10, "F10"),
        (egui::Key::F11, "F11"),
        (egui::Key::F12, "F12"),
    ];
    const NAVIGATION: &[(egui::Key, &str)] = &[
        (egui::Key::ArrowUp, "Up"),
        (egui::Key::ArrowDown, "Down"),
        (egui::Key::ArrowLeft, "Left"),
        (egui::Key::ArrowRight, "Right"),
        (egui::Key::Home, "Home"),
        (egui::Key::End, "End"),
        (egui::Key::PageUp, "PageUp"),
        (egui::Key::PageDown, "PageDown"),
        (egui::Key::Enter, "Enter"),
        (egui::Key::Space, "Space"),
        (egui::Key::Insert, "Insert"),
    ];
    const PUNCTUATION: &[(egui::Key, &str)] = &[
        (egui::Key::Minus, "Minus"),
        (egui::Key::Plus, "Plus"),
        (egui::Key::Equals, "Equals"),
        (egui::Key::OpenBracket, "BracketLeft"),
        (egui::Key::CloseBracket, "BracketRight"),
        (egui::Key::Backslash, "Backslash"),
        (egui::Key::Semicolon, "Semicolon"),
        (egui::Key::Colon, "Colon"),
        (egui::Key::Comma, "Comma"),
        (egui::Key::Period, "Period"),
        (egui::Key::Slash, "Slash"),
        (egui::Key::Backtick, "Backquote"),
    ];
    const FAMILIES: &[&[(egui::Key, &str)]] = &[LETTERS, DIGITS, FUNCTION, NAVIGATION, PUNCTUATION];

    fn assert_family(entries: &[(egui::Key, &str)]) {
        for &(key, name) in entries {
            assert_eq!(key_to_string(key), Some(name), "{key:?}");
        }
    }

    #[test]
    fn key_to_string_letters() {
        assert_family(LETTERS);
    }

    #[test]
    fn key_to_string_digits() {
        assert_family(DIGITS);
    }

    #[test]
    fn key_to_string_function_keys() {
        assert_family(FUNCTION);
    }

    #[test]
    fn key_to_string_navigation() {
        assert_family(NAVIGATION);
    }

    #[test]
    fn key_to_string_punctuation() {
        assert_family(PUNCTUATION);
    }

    /// Every egui key outside the oracle is unrecordable (Escape, Tab,
    /// Backspace, Delete, F13+, and so on).
    #[test]
    fn key_to_string_rejects_every_other_key() {
        let mapped: Vec<egui::Key> = FAMILIES
            .iter()
            .flat_map(|f| f.iter().map(|&(k, _)| k))
            .collect();
        assert_eq!(mapped.len(), 71);
        for &key in egui::Key::ALL {
            if !mapped.contains(&key) {
                assert_eq!(key_to_string(key), None, "{key:?} must not record");
            }
        }
        for key in [
            egui::Key::Escape,
            egui::Key::Tab,
            egui::Key::Backspace,
            egui::Key::Delete,
            egui::Key::F13,
        ] {
            assert_eq!(key_to_string(key), None, "{key:?}");
        }
    }

    /// Recorded names must be accepted by the keybinding parser as the key
    /// part of a chord.
    #[test]
    fn key_to_string_names_parse_as_chords() {
        for family in FAMILIES {
            for &(_, name) in *family {
                let combo = format!("Ctrl+{name}");
                par_term_keybindings::parser::parse_key_combo(&combo)
                    .unwrap_or_else(|e| panic!("{combo:?} must parse: {e}"));
            }
        }
    }

    #[test]
    fn recorder_keeps_ctrl_when_cmd_held() {
        // B18: Cmd+Ctrl+Opt+Arrow (the shipped pane-swap default) must record
        // with BOTH command and ctrl, spelled as the defaults table spells it
        // (CmdOrCtrl+Ctrl+Alt+…). Ctrl used to be dropped whenever Cmd was
        // held, making the shipped default unrecordable.
        #[cfg(target_os = "macos")]
        {
            let mods = egui::Modifiers {
                ctrl: true,
                alt: true,
                mac_cmd: true,
                command: true,
                ..Default::default()
            };
            assert_eq!(
                record(mods, egui::Key::ArrowRight).as_deref(),
                Some("CmdOrCtrl+Ctrl+Alt+Right")
            );
        }
        #[cfg(not(target_os = "macos"))]
        {
            // egui reports command == ctrl off macOS; the recorder must not
            // emit a second modifier part for it.
            let mods = egui::Modifiers {
                ctrl: true,
                alt: true,
                command: true,
                ..Default::default()
            };
            assert_eq!(
                record(mods, egui::Key::ArrowRight).as_deref(),
                Some("CmdOrCtrl+Alt+Right")
            );
        }
    }

    #[test]
    fn recorder_punctuation_chord_parses() {
        // B19 / criterion 4: Cmd+Shift+] records to a chord string the
        // keybinding parser accepts (the next_tab default's chord).
        #[cfg(target_os = "macos")]
        let mods = egui::Modifiers {
            shift: true,
            mac_cmd: true,
            command: true,
            ..Default::default()
        };
        #[cfg(not(target_os = "macos"))]
        let mods = egui::Modifiers {
            ctrl: true,
            shift: true,
            command: true,
            ..Default::default()
        };
        let combo = record(mods, egui::Key::CloseBracket).expect("combo recorded");
        assert_eq!(combo, "CmdOrCtrl+Shift+BracketRight");
        par_term_keybindings::parser::parse_key_combo(&combo)
            .map(|_| ())
            .unwrap_or_else(|e| panic!("recorded chord must parse: {e}"));
    }
}

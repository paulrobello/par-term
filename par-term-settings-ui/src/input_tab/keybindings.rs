//! Keybindings editor section.
//!
//! Contains the keybindings grid and the `capture_key_combo` / `display_key_combo`
//! utilities that are also re-used by `actions_tab` and `snippets_tab`.
//!
//! The `AVAILABLE_ACTIONS` lookup table lives in [`super::actions_table`].

use crate::SettingsUI;
use crate::section::collapsing_section;
use par_term_config::KeyBinding;
use std::collections::HashSet;

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

pub(super) fn show_keybindings_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Keybindings",
        "input_keybindings",
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
                }
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

            egui::ScrollArea::vertical()
                .min_scrolled_height(600.0)
                .show(ui, |ui| {
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

                                ui.label(*display_name);

                                let is_recording =
                                    settings.keybinding_recording_index == Some(*idx);
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
                                    let button_text =
                                        if is_recording { "Cancel" } else { "Record" };
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
            }

            if let Some(idx) = start_recording {
                settings.keybinding_recording_index = Some(idx);
                settings.keybinding_recorded_combo = None;
            }

            if let Some(action_name) = action_to_clear {
                settings
                    .config
                    .keybindings
                    .retain(|b| b.action != action_name);
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            #[cfg(target_os = "macos")]
            {
                ui.label("Key combo format: Modifiers+Key (e.g., 'Cmd+Shift+B', 'Ctrl+T')");
                ui.label("Available modifiers: Cmd, Ctrl, Alt, Shift");
            }
            #[cfg(not(target_os = "macos"))]
            {
                ui.label("Key combo format: Modifiers+Key (e.g., 'Ctrl+Shift+B', 'Alt+T')");
                ui.label("Available modifiers: Ctrl, Alt, Shift, Super");
            }
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
                    if cmd {
                        parts.push("CmdOrCtrl");
                    } else if ctrl {
                        parts.push("Ctrl");
                    }
                }

                #[cfg(not(target_os = "macos"))]
                {
                    if ctrl {
                        parts.push("CmdOrCtrl");
                    }
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

fn key_to_string(key: egui::Key) -> Option<&'static str> {
    match key {
        egui::Key::A => Some("A"),
        egui::Key::B => Some("B"),
        egui::Key::C => Some("C"),
        egui::Key::D => Some("D"),
        egui::Key::E => Some("E"),
        egui::Key::F => Some("F"),
        egui::Key::G => Some("G"),
        egui::Key::H => Some("H"),
        egui::Key::I => Some("I"),
        egui::Key::J => Some("J"),
        egui::Key::K => Some("K"),
        egui::Key::L => Some("L"),
        egui::Key::M => Some("M"),
        egui::Key::N => Some("N"),
        egui::Key::O => Some("O"),
        egui::Key::P => Some("P"),
        egui::Key::Q => Some("Q"),
        egui::Key::R => Some("R"),
        egui::Key::S => Some("S"),
        egui::Key::T => Some("T"),
        egui::Key::U => Some("U"),
        egui::Key::V => Some("V"),
        egui::Key::W => Some("W"),
        egui::Key::X => Some("X"),
        egui::Key::Y => Some("Y"),
        egui::Key::Z => Some("Z"),
        egui::Key::Num0 => Some("0"),
        egui::Key::Num1 => Some("1"),
        egui::Key::Num2 => Some("2"),
        egui::Key::Num3 => Some("3"),
        egui::Key::Num4 => Some("4"),
        egui::Key::Num5 => Some("5"),
        egui::Key::Num6 => Some("6"),
        egui::Key::Num7 => Some("7"),
        egui::Key::Num8 => Some("8"),
        egui::Key::Num9 => Some("9"),
        egui::Key::F1 => Some("F1"),
        egui::Key::F2 => Some("F2"),
        egui::Key::F3 => Some("F3"),
        egui::Key::F4 => Some("F4"),
        egui::Key::F5 => Some("F5"),
        egui::Key::F6 => Some("F6"),
        egui::Key::F7 => Some("F7"),
        egui::Key::F8 => Some("F8"),
        egui::Key::F9 => Some("F9"),
        egui::Key::F10 => Some("F10"),
        egui::Key::F11 => Some("F11"),
        egui::Key::F12 => Some("F12"),
        egui::Key::ArrowUp => Some("Up"),
        egui::Key::ArrowDown => Some("Down"),
        egui::Key::ArrowLeft => Some("Left"),
        egui::Key::ArrowRight => Some("Right"),
        egui::Key::Home => Some("Home"),
        egui::Key::End => Some("End"),
        egui::Key::PageUp => Some("PageUp"),
        egui::Key::PageDown => Some("PageDown"),
        egui::Key::Enter => Some("Enter"),
        egui::Key::Space => Some("Space"),
        egui::Key::Insert => Some("Insert"),
        egui::Key::Minus => Some("Minus"),
        egui::Key::Plus => Some("Plus"),
        egui::Key::Equals => Some("Equals"),
        egui::Key::OpenBracket => Some("BracketLeft"),
        egui::Key::CloseBracket => Some("BracketRight"),
        egui::Key::Backslash => Some("Backslash"),
        egui::Key::Semicolon => Some("Semicolon"),
        egui::Key::Colon => Some("Colon"),
        egui::Key::Comma => Some("Comma"),
        egui::Key::Period => Some("Period"),
        egui::Key::Slash => Some("Slash"),
        egui::Key::Backtick => Some("Backquote"),
        _ => None,
    }
}

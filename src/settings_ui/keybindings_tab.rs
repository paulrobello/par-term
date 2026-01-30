//! Keybindings settings tab.
//!
//! This module provides the UI for configuring custom keybindings.

#![allow(clippy::type_complexity)]

use super::SettingsUI;
use crate::config::KeyBinding;

/// All available keybinding actions with their descriptions and default key combos.
/// Format: (action_name, display_name, default_key_combo or None)
const AVAILABLE_ACTIONS: &[(&str, &str, Option<&str>)] = &[
    (
        "toggle_background_shader",
        "Toggle Background Shader",
        Some("CmdOrCtrl+Shift+B"),
    ),
    (
        "toggle_cursor_shader",
        "Toggle Cursor Shader",
        Some("CmdOrCtrl+Shift+U"),
    ),
    ("reload_config", "Reload Configuration", Some("F5")),
    ("open_settings", "Open Settings", Some("F12")),
    ("toggle_fullscreen", "Toggle Fullscreen", Some("F11")),
    ("toggle_help", "Toggle Help Panel", Some("F1")),
    ("toggle_fps_overlay", "Toggle FPS Overlay", Some("F3")),
    ("new_tab", "New Tab", Some("CmdOrCtrl+T")),
    ("close_tab", "Close Tab", Some("CmdOrCtrl+W")),
    ("next_tab", "Next Tab", Some("CmdOrCtrl+Shift+]")),
    ("prev_tab", "Previous Tab", Some("CmdOrCtrl+Shift+[")),
];

/// Convert "CmdOrCtrl" to the platform-specific display string.
fn display_key_combo(combo: &str) -> String {
    #[cfg(target_os = "macos")]
    {
        combo.replace("CmdOrCtrl", "Cmd")
    }
    #[cfg(not(target_os = "macos"))]
    {
        combo.replace("CmdOrCtrl", "Ctrl")
    }
}

pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Keybindings", |ui| {
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
                // User pressed a valid key combo
                settings.keybinding_recorded_combo = Some(combo.clone());

                // Find the action for this index
                if recording_idx < AVAILABLE_ACTIONS.len() {
                    let (action_name, _, _) = AVAILABLE_ACTIONS[recording_idx];

                    // Update or add the keybinding
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
                    log::info!(
                        "Keybinding recorded for {}: {}",
                        action_name,
                        settings.keybinding_recorded_combo.as_ref().unwrap()
                    );
                }

                // Stop recording
                settings.keybinding_recording_index = None;
            }

            // Check for Escape to cancel recording
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                settings.keybinding_recording_index = None;
                settings.keybinding_recorded_combo = None;
            }
        }

        // Collect binding info first to avoid borrow conflicts
        // Tuple: (index, action_name, display_name, current_binding, default_binding, is_custom)
        let binding_info: Vec<(usize, &str, &str, Option<String>, Option<&str>, bool)> =
            AVAILABLE_ACTIONS
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

        // Track actions to perform after the grid
        let mut action_to_clear: Option<&str> = None;
        let mut start_recording: Option<usize> = None;
        let mut cancel_recording = false;

        // Render keybinding list
        egui::Grid::new("keybindings_grid")
            .num_columns(3)
            .spacing([20.0, 8.0])
            .striped(true)
            .show(ui, |ui| {
                // Header row
                ui.strong("Action");
                ui.strong("Key Combo");
                ui.strong("");
                ui.end_row();

                for (idx, action_name, display_name, custom_binding, default_binding, is_custom) in
                    &binding_info
                {
                    // Determine what to display: custom binding, default, or "(not set)"
                    let (binding_display, show_as_default) = if let Some(custom) = custom_binding {
                        (display_key_combo(custom), false)
                    } else if let Some(default) = default_binding {
                        (display_key_combo(default), true)
                    } else {
                        ("(not set)".to_string(), false)
                    };

                    ui.label(*display_name);

                    // Show current binding or recording indicator
                    let is_recording = settings.keybinding_recording_index == Some(*idx);
                    if is_recording {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            "Press key combo... (Esc to cancel)",
                        );
                    } else if show_as_default {
                        // Show default bindings in a dimmed color to distinguish from custom
                        ui.colored_label(
                            egui::Color32::from_rgb(128, 128, 128),
                            egui::RichText::new(&binding_display).monospace(),
                        );
                    } else {
                        ui.monospace(&binding_display);
                    }

                    // Record/Clear buttons
                    ui.horizontal(|ui| {
                        let button_text = if is_recording { "Cancel" } else { "Record" };
                        if ui.button(button_text).clicked() {
                            if is_recording {
                                cancel_recording = true;
                            } else {
                                start_recording = Some(*idx);
                            }
                        }

                        // Clear button (only show if there's a custom binding)
                        if *is_custom && !is_recording && ui.button("Clear").clicked() {
                            action_to_clear = Some(*action_name);
                        }
                    });

                    ui.end_row();
                }
            });

        // Apply deferred actions
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
            log::info!("Keybinding cleared for {}", action_name);
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Help text
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
    });
}

/// Capture key combo from egui input.
/// Returns Some(combo_string) when a valid key is pressed, None otherwise.
fn capture_key_combo(ui: &egui::Ui) -> Option<String> {
    ui.input(|input| {
        // Get current modifiers
        let ctrl = input.modifiers.ctrl;
        let alt = input.modifiers.alt;
        let shift = input.modifiers.shift;
        let cmd = input.modifiers.mac_cmd || input.modifiers.command;

        // Check for key press events
        for event in &input.events {
            if let egui::Event::Key {
                key, pressed: true, ..
            } = event
            {
                // Ignore modifier-only presses
                if matches!(
                    key,
                    egui::Key::Tab | egui::Key::Escape | egui::Key::Backspace | egui::Key::Delete
                ) {
                    continue;
                }

                // Build the combo string
                let mut parts = Vec::new();

                // On macOS, prefer CmdOrCtrl when Cmd is pressed
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
                    let _ = cmd; // silence unused warning
                }

                if alt {
                    parts.push("Alt");
                }
                if shift {
                    parts.push("Shift");
                }

                // Convert egui::Key to string
                let key_str = key_to_string(*key);
                if let Some(key_name) = key_str {
                    // Require at least one modifier for most keys (except F-keys)
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

/// Convert egui::Key to a string suitable for keybinding config.
fn key_to_string(key: egui::Key) -> Option<&'static str> {
    match key {
        // Letters
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

        // Numbers
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

        // Function keys
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

        // Navigation
        egui::Key::ArrowUp => Some("Up"),
        egui::Key::ArrowDown => Some("Down"),
        egui::Key::ArrowLeft => Some("Left"),
        egui::Key::ArrowRight => Some("Right"),
        egui::Key::Home => Some("Home"),
        egui::Key::End => Some("End"),
        egui::Key::PageUp => Some("PageUp"),
        egui::Key::PageDown => Some("PageDown"),

        // Special keys
        egui::Key::Enter => Some("Enter"),
        egui::Key::Space => Some("Space"),
        egui::Key::Insert => Some("Insert"),

        // Punctuation
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

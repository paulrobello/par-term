//! Keybindings editor section.
//!
//! Contains the keybindings grid, `AVAILABLE_ACTIONS` constant tables,
//! and the `capture_key_combo` / `display_key_combo` utilities that are
//! also re-used by `actions_tab` and `snippets_tab`.

use crate::SettingsUI;
use crate::section::collapsing_section;
use par_term_config::KeyBinding;
use std::collections::HashSet;

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
// AVAILABLE_ACTIONS lookup tables (platform-split)
// ============================================================================

/// All available keybinding actions with their descriptions and default key combos.
/// macOS uses Cmd as the primary modifier (safe for terminals).
/// Windows/Linux uses Ctrl+Shift to avoid conflicts with terminal control codes.
#[cfg(target_os = "macos")]
pub(super) const AVAILABLE_ACTIONS: &[(&str, &str, Option<&str>)] = &[
    ("toggle_help", "Toggle Help Panel", Some("F1")),
    ("toggle_fps_overlay", "Toggle FPS Overlay", Some("F3")),
    ("reload_config", "Reload Configuration", Some("F5")),
    ("toggle_fullscreen", "Toggle Fullscreen", Some("F11")),
    ("open_settings", "Open Settings", Some("F12")),
    ("toggle_search", "Toggle Search", Some("Cmd+F")),
    (
        "toggle_profile_drawer",
        "Toggle Profile Drawer",
        Some("Cmd+Shift+P"),
    ),
    (
        "toggle_clipboard_history",
        "Toggle Clipboard History",
        Some("Cmd+Shift+H"),
    ),
    ("maximize_vertically", "Maximize Vertically", None),
    (
        "toggle_background_shader",
        "Toggle Background Shader",
        Some("Cmd+Shift+B"),
    ),
    (
        "toggle_cursor_shader",
        "Toggle Cursor Shader",
        Some("Cmd+Shift+U"),
    ),
    ("new_tab", "New Tab", Some("Cmd+T")),
    ("close_tab", "Close Tab", Some("Cmd+W")),
    ("next_tab", "Next Tab", Some("Cmd+Shift+]")),
    ("prev_tab", "Previous Tab", Some("Cmd+Shift+[")),
    ("move_tab_left", "Move Tab Left", Some("Cmd+Shift+Left")),
    ("move_tab_right", "Move Tab Right", Some("Cmd+Shift+Right")),
    ("switch_to_tab_1", "Switch to Tab 1", Some("Cmd+1")),
    ("switch_to_tab_2", "Switch to Tab 2", Some("Cmd+2")),
    ("switch_to_tab_3", "Switch to Tab 3", Some("Cmd+3")),
    ("switch_to_tab_4", "Switch to Tab 4", Some("Cmd+4")),
    ("switch_to_tab_5", "Switch to Tab 5", Some("Cmd+5")),
    ("switch_to_tab_6", "Switch to Tab 6", Some("Cmd+6")),
    ("switch_to_tab_7", "Switch to Tab 7", Some("Cmd+7")),
    ("switch_to_tab_8", "Switch to Tab 8", Some("Cmd+8")),
    ("switch_to_tab_9", "Switch to Tab 9", Some("Cmd+9")),
    ("split_horizontal", "Split Pane Horizontal", Some("Cmd+D")),
    ("split_vertical", "Split Pane Vertical", Some("Cmd+Shift+D")),
    ("close_pane", "Close Pane", Some("Cmd+Shift+W")),
    (
        "navigate_pane_left",
        "Navigate Pane Left",
        Some("Cmd+Alt+Left"),
    ),
    (
        "navigate_pane_right",
        "Navigate Pane Right",
        Some("Cmd+Alt+Right"),
    ),
    ("navigate_pane_up", "Navigate Pane Up", Some("Cmd+Alt+Up")),
    (
        "navigate_pane_down",
        "Navigate Pane Down",
        Some("Cmd+Alt+Down"),
    ),
    (
        "resize_pane_left",
        "Resize Pane Left",
        Some("Cmd+Alt+Shift+Left"),
    ),
    (
        "resize_pane_right",
        "Resize Pane Right",
        Some("Cmd+Alt+Shift+Right"),
    ),
    ("resize_pane_up", "Resize Pane Up", Some("Cmd+Alt+Shift+Up")),
    (
        "resize_pane_down",
        "Resize Pane Down",
        Some("Cmd+Alt+Shift+Down"),
    ),
    (
        "increase_font_size",
        "Increase Font Size",
        Some("Ctrl+Plus"),
    ),
    (
        "decrease_font_size",
        "Decrease Font Size",
        Some("Ctrl+Minus"),
    ),
    ("reset_font_size", "Reset Font Size", Some("Ctrl+0")),
    ("clear_scrollback", "Clear Scrollback", Some("Cmd+Shift+K")),
    (
        "cycle_cursor_style",
        "Cycle Cursor Style",
        Some("Cmd+Comma"),
    ),
    (
        "paste_special",
        "Paste Special (Transform)",
        Some("Cmd+Shift+V"),
    ),
    (
        "toggle_session_logging",
        "Toggle Session Logging",
        Some("Cmd+Shift+R"),
    ),
    (
        "toggle_broadcast_input",
        "Toggle Broadcast Input",
        Some("Cmd+Alt+I"),
    ),
    (
        "toggle_throughput_mode",
        "Toggle Throughput Mode",
        Some("Cmd+Shift+T"),
    ),
    (
        "toggle_tmux_session_picker",
        "Toggle tmux Session Picker",
        Some("Cmd+Alt+T"),
    ),
    ("toggle_copy_mode", "Toggle Copy Mode", Some("Cmd+Shift+C")),
];

#[cfg(not(target_os = "macos"))]
pub(super) const AVAILABLE_ACTIONS: &[(&str, &str, Option<&str>)] = &[
    ("toggle_help", "Toggle Help Panel", Some("F1")),
    ("toggle_fps_overlay", "Toggle FPS Overlay", Some("F3")),
    ("reload_config", "Reload Configuration", Some("F5")),
    ("toggle_fullscreen", "Toggle Fullscreen", Some("F11")),
    ("open_settings", "Open Settings", Some("F12")),
    ("toggle_search", "Toggle Search", Some("Ctrl+Shift+F")),
    (
        "toggle_profile_drawer",
        "Toggle Profile Drawer",
        Some("Ctrl+Shift+P"),
    ),
    (
        "toggle_clipboard_history",
        "Toggle Clipboard History",
        Some("Ctrl+Shift+H"),
    ),
    ("maximize_vertically", "Maximize Vertically", None),
    (
        "toggle_background_shader",
        "Toggle Background Shader",
        Some("Ctrl+Shift+B"),
    ),
    (
        "toggle_cursor_shader",
        "Toggle Cursor Shader",
        Some("Ctrl+Shift+U"),
    ),
    ("new_tab", "New Tab", Some("Ctrl+Shift+T")),
    ("close_tab", "Close Tab", Some("Ctrl+Shift+W")),
    ("next_tab", "Next Tab", Some("Ctrl+Shift+]")),
    ("prev_tab", "Previous Tab", Some("Ctrl+Shift+[")),
    ("move_tab_left", "Move Tab Left", Some("Ctrl+Shift+Left")),
    ("move_tab_right", "Move Tab Right", Some("Ctrl+Shift+Right")),
    ("switch_to_tab_1", "Switch to Tab 1", Some("Alt+1")),
    ("switch_to_tab_2", "Switch to Tab 2", Some("Alt+2")),
    ("switch_to_tab_3", "Switch to Tab 3", Some("Alt+3")),
    ("switch_to_tab_4", "Switch to Tab 4", Some("Alt+4")),
    ("switch_to_tab_5", "Switch to Tab 5", Some("Alt+5")),
    ("switch_to_tab_6", "Switch to Tab 6", Some("Alt+6")),
    ("switch_to_tab_7", "Switch to Tab 7", Some("Alt+7")),
    ("switch_to_tab_8", "Switch to Tab 8", Some("Alt+8")),
    ("switch_to_tab_9", "Switch to Tab 9", Some("Alt+9")),
    (
        "split_horizontal",
        "Split Pane Horizontal",
        Some("Ctrl+Shift+D"),
    ),
    (
        "split_vertical",
        "Split Pane Vertical",
        Some("Ctrl+Shift+E"),
    ),
    ("close_pane", "Close Pane", Some("Ctrl+Shift+X")),
    (
        "navigate_pane_left",
        "Navigate Pane Left",
        Some("Ctrl+Alt+Left"),
    ),
    (
        "navigate_pane_right",
        "Navigate Pane Right",
        Some("Ctrl+Alt+Right"),
    ),
    ("navigate_pane_up", "Navigate Pane Up", Some("Ctrl+Alt+Up")),
    (
        "navigate_pane_down",
        "Navigate Pane Down",
        Some("Ctrl+Alt+Down"),
    ),
    (
        "resize_pane_left",
        "Resize Pane Left",
        Some("Ctrl+Alt+Shift+Left"),
    ),
    (
        "resize_pane_right",
        "Resize Pane Right",
        Some("Ctrl+Alt+Shift+Right"),
    ),
    (
        "resize_pane_up",
        "Resize Pane Up",
        Some("Ctrl+Alt+Shift+Up"),
    ),
    (
        "resize_pane_down",
        "Resize Pane Down",
        Some("Ctrl+Alt+Shift+Down"),
    ),
    (
        "increase_font_size",
        "Increase Font Size",
        Some("Ctrl+Plus"),
    ),
    (
        "decrease_font_size",
        "Decrease Font Size",
        Some("Ctrl+Minus"),
    ),
    ("reset_font_size", "Reset Font Size", Some("Ctrl+0")),
    ("clear_scrollback", "Clear Scrollback", Some("Ctrl+Shift+K")),
    (
        "cycle_cursor_style",
        "Cycle Cursor Style",
        Some("Ctrl+Comma"),
    ),
    (
        "paste_special",
        "Paste Special (Transform)",
        Some("Ctrl+Alt+V"),
    ),
    (
        "toggle_session_logging",
        "Toggle Session Logging",
        Some("Ctrl+Shift+R"),
    ),
    (
        "toggle_broadcast_input",
        "Toggle Broadcast Input",
        Some("Ctrl+Alt+I"),
    ),
    (
        "toggle_throughput_mode",
        "Toggle Throughput Mode",
        Some("Ctrl+Shift+M"),
    ),
    (
        "toggle_tmux_session_picker",
        "Toggle tmux Session Picker",
        Some("Ctrl+Alt+T"),
    ),
    (
        "toggle_copy_mode",
        "Toggle Copy Mode",
        Some("Ctrl+Shift+Space"),
    ),
];

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

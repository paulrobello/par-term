//! Input settings tab.
//!
//! Consolidates: keyboard_tab, mouse_tab, keybindings_tab
//!
//! Contains:
//! - Keyboard settings (Option/Alt key modes, modifier remapping, physical keys)
//! - Mouse behavior (scroll speed, click thresholds)
//! - Selection & Clipboard settings
//! - Keybindings editor

use super::SettingsUI;
use super::section::{SLIDER_WIDTH, collapsing_section};
use crate::config::{DroppedFileQuoteStyle, KeyBinding, ModifierTarget, OptionKeyMode};

const SLIDER_HEIGHT: f32 = 18.0;

/// Type alias for keybinding info: (index, action_name, display_name, custom_binding, default_key, is_custom)
type BindingInfo<'a> = (
    usize,
    &'a str,
    &'a str,
    Option<String>,
    Option<&'a str>,
    bool,
);

/// Show the input tab content.
pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    let query = settings.search_query.trim().to_lowercase();

    // Keyboard section
    if section_matches(
        &query,
        "Keyboard",
        &["option", "alt", "meta", "esc", "physical"],
    ) {
        show_keyboard_section(ui, settings, changes_this_frame);
    }

    // Modifier Remapping section
    if section_matches(
        &query,
        "Modifier Remapping",
        &["remap", "swap", "ctrl", "super", "cmd", "modifier"],
    ) {
        show_modifier_remapping_section(ui, settings, changes_this_frame);
    }

    // Mouse section
    if section_matches(
        &query,
        "Mouse",
        &["scroll", "double-click", "triple-click", "focus follows"],
    ) {
        show_mouse_section(ui, settings, changes_this_frame);
    }

    // Selection & Clipboard section
    if section_matches(
        &query,
        "Selection & Clipboard",
        &["copy", "paste", "middle-click", "auto-copy", "delay"],
    ) {
        show_selection_section(ui, settings, changes_this_frame);
    }

    // Clipboard Limits section (collapsed by default)
    if section_matches(&query, "Clipboard Limits", &["max", "sync", "bytes"]) {
        show_clipboard_limits_section(ui, settings, changes_this_frame);
    }

    // Word Selection section (collapsed by default)
    if section_matches(
        &query,
        "Word Selection",
        &["word characters", "smart selection"],
    ) {
        show_word_selection_section(ui, settings, changes_this_frame);
    }

    // Copy Mode section
    if section_matches(
        &query,
        "Copy Mode",
        &["copy mode", "vi", "vim", "yank", "visual", "selection mode"],
    ) {
        show_copy_mode_section(ui, settings, changes_this_frame);
    }

    // Keybindings section (takes most space)
    if section_matches(
        &query,
        "Keybindings",
        &["shortcut", "hotkey", "binding", "key"],
    ) {
        show_keybindings_section(ui, settings, changes_this_frame);
    }
}

fn section_matches(query: &str, title: &str, keywords: &[&str]) -> bool {
    if query.is_empty() {
        return true;
    }
    if title.to_lowercase().contains(query) {
        return true;
    }
    keywords.iter().any(|k| k.to_lowercase().contains(query))
}

// ============================================================================
// Keyboard Section
// ============================================================================

fn show_keyboard_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Keyboard", "input_keyboard", true, |ui| {
        ui.label("Option/Alt key behavior for emacs, vim, and other terminal applications.");
        ui.add_space(4.0);

        // Left Option/Alt key mode
        ui.horizontal(|ui| {
            ui.label("Left Option/Alt sends:");
            let current = settings.config.left_option_key_mode;
            egui::ComboBox::from_id_salt("input_left_option_key_mode")
                .selected_text(option_key_mode_label(current))
                .show_ui(ui, |ui| {
                    for mode in [
                        OptionKeyMode::Esc,
                        OptionKeyMode::Meta,
                        OptionKeyMode::Normal,
                    ] {
                        if ui
                            .selectable_value(
                                &mut settings.config.left_option_key_mode,
                                mode,
                                option_key_mode_label(mode),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        ui.indent("input_left_option_desc", |ui| {
            ui.label(
                egui::RichText::new(option_key_mode_description(
                    settings.config.left_option_key_mode,
                ))
                .weak()
                .small(),
            );
        });

        ui.add_space(8.0);

        // Right Option/Alt key mode
        ui.horizontal(|ui| {
            ui.label("Right Option/Alt sends:");
            let current = settings.config.right_option_key_mode;
            egui::ComboBox::from_id_salt("input_right_option_key_mode")
                .selected_text(option_key_mode_label(current))
                .show_ui(ui, |ui| {
                    for mode in [
                        OptionKeyMode::Esc,
                        OptionKeyMode::Meta,
                        OptionKeyMode::Normal,
                    ] {
                        if ui
                            .selectable_value(
                                &mut settings.config.right_option_key_mode,
                                mode,
                                option_key_mode_label(mode),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        ui.indent("input_right_option_desc", |ui| {
            ui.label(
                egui::RichText::new(option_key_mode_description(
                    settings.config.right_option_key_mode,
                ))
                .weak()
                .small(),
            );
        });

        ui.add_space(8.0);
        ui.separator();

        // Physical key preference
        if ui
            .checkbox(
                &mut settings.config.use_physical_keys,
                "Use physical key positions for keybindings",
            )
            .on_hover_text(
                "Match keybindings by key position (scan code) instead of character produced.\n\
                 This makes shortcuts like Ctrl+Z work consistently across keyboard layouts\n\
                 (QWERTY, AZERTY, Dvorak, etc.).",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);
        ui.separator();
        ui.label(egui::RichText::new("Tips:").strong());
        ui.label("• Use \"Esc\" mode for emacs Meta key (M-x, M-f, M-b, etc.)");
        ui.label("• Use \"Esc\" mode for vim Alt mappings");
        ui.label("• Use \"Normal\" to type special characters (ƒ, ∂, ß, etc.)");
        ui.label("• Enable physical keys if shortcuts feel wrong on non-US layouts");
    });
}

// ============================================================================
// Modifier Remapping Section
// ============================================================================

fn show_modifier_remapping_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(
        ui,
        "Modifier Remapping",
        "input_modifier_remapping",
        false,
        |ui| {
            ui.label("Remap modifier keys to different functions.");
            ui.label(
                egui::RichText::new(
                    "Note: Changes apply to par-term keybindings only, not system-wide.",
                )
                .weak()
                .small(),
            );
            ui.add_space(4.0);

            // Left Ctrl
            ui.horizontal(|ui| {
                ui.label("Left Ctrl acts as:");
                let current = settings.config.modifier_remapping.left_ctrl;
                egui::ComboBox::from_id_salt("input_remap_left_ctrl")
                    .selected_text(current.display_name())
                    .show_ui(ui, |ui| {
                        for target in ModifierTarget::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.modifier_remapping.left_ctrl,
                                    *target,
                                    target.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            // Right Ctrl
            ui.horizontal(|ui| {
                ui.label("Right Ctrl acts as:");
                let current = settings.config.modifier_remapping.right_ctrl;
                egui::ComboBox::from_id_salt("input_remap_right_ctrl")
                    .selected_text(current.display_name())
                    .show_ui(ui, |ui| {
                        for target in ModifierTarget::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.modifier_remapping.right_ctrl,
                                    *target,
                                    target.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            ui.add_space(4.0);

            // Left Alt
            ui.horizontal(|ui| {
                ui.label("Left Alt acts as:");
                let current = settings.config.modifier_remapping.left_alt;
                egui::ComboBox::from_id_salt("input_remap_left_alt")
                    .selected_text(current.display_name())
                    .show_ui(ui, |ui| {
                        for target in ModifierTarget::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.modifier_remapping.left_alt,
                                    *target,
                                    target.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            // Right Alt
            ui.horizontal(|ui| {
                ui.label("Right Alt acts as:");
                let current = settings.config.modifier_remapping.right_alt;
                egui::ComboBox::from_id_salt("input_remap_right_alt")
                    .selected_text(current.display_name())
                    .show_ui(ui, |ui| {
                        for target in ModifierTarget::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.modifier_remapping.right_alt,
                                    *target,
                                    target.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            ui.add_space(4.0);

            #[cfg(target_os = "macos")]
            let super_label = "Cmd";
            #[cfg(not(target_os = "macos"))]
            let super_label = "Super";

            // Left Super/Cmd
            ui.horizontal(|ui| {
                ui.label(format!("Left {} acts as:", super_label));
                let current = settings.config.modifier_remapping.left_super;
                egui::ComboBox::from_id_salt("input_remap_left_super")
                    .selected_text(current.display_name())
                    .show_ui(ui, |ui| {
                        for target in ModifierTarget::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.modifier_remapping.left_super,
                                    *target,
                                    target.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            // Right Super/Cmd
            ui.horizontal(|ui| {
                ui.label(format!("Right {} acts as:", super_label));
                let current = settings.config.modifier_remapping.right_super;
                egui::ComboBox::from_id_salt("input_remap_right_super")
                    .selected_text(current.display_name())
                    .show_ui(ui, |ui| {
                        for target in ModifierTarget::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.modifier_remapping.right_super,
                                    *target,
                                    target.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            ui.add_space(8.0);

            if ui.button("Reset to defaults").clicked() {
                settings.config.modifier_remapping = Default::default();
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        },
    );
}

fn option_key_mode_label(mode: OptionKeyMode) -> &'static str {
    match mode {
        OptionKeyMode::Normal => "Normal",
        OptionKeyMode::Meta => "Meta",
        OptionKeyMode::Esc => "Esc (Recommended)",
    }
}

fn option_key_mode_description(mode: OptionKeyMode) -> &'static str {
    match mode {
        OptionKeyMode::Normal => {
            "Sends special characters (e.g., Option+f → ƒ). Default macOS behavior."
        }
        OptionKeyMode::Meta => {
            "Sets high bit on character (e.g., Option+f → 0xE6). Legacy Meta key mode."
        }
        OptionKeyMode::Esc => {
            "Sends Escape prefix (e.g., Option+f → ESC f). Best for emacs/vim compatibility."
        }
    }
}

// ============================================================================
// Mouse Section
// ============================================================================

fn show_mouse_section(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    collapsing_section(ui, "Mouse", "input_mouse", true, |ui| {
        ui.horizontal(|ui| {
            ui.label("Scroll speed:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.mouse_scroll_speed, 0.1..=10.0),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Double-click threshold (ms):");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(
                        &mut settings.config.mouse_double_click_threshold,
                        100..=1000,
                    ),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Triple-click threshold (ms):");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(
                        &mut settings.config.mouse_triple_click_threshold,
                        100..=1000,
                    ),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.separator();
        ui.label("Advanced Mouse Features");

        #[cfg(target_os = "macos")]
        let option_click_label = "Option+Click moves cursor";
        #[cfg(not(target_os = "macos"))]
        let option_click_label = "Alt+Click moves cursor";

        if ui
            .checkbox(
                &mut settings.config.option_click_moves_cursor,
                option_click_label,
            )
            .on_hover_text("Position the text cursor at the clicked location")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.focus_follows_mouse,
                "Focus follows mouse",
            )
            .on_hover_text("Automatically focus the terminal window when the mouse enters it")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.report_horizontal_scroll,
                "Report horizontal scroll events",
            )
            .on_hover_text(
                "Report horizontal scroll to applications via mouse button codes 6 and 7",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

// ============================================================================
// Selection & Clipboard Section
// ============================================================================

fn show_selection_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Selection & Clipboard", "input_selection", true, |ui| {
        if ui
            .checkbox(
                &mut settings.config.auto_copy_selection,
                "Auto-copy selection",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.copy_trailing_newline,
                "Include trailing newline when copying",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.middle_click_paste,
                "Middle-click paste",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.horizontal(|ui| {
            ui.label("Paste delay (ms):");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.paste_delay_ms, 0..=500),
                )
                .on_hover_text(
                    "Delay between pasted lines in milliseconds (0 = no delay). \
                     Useful for slow terminals or remote connections.",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.separator();
        ui.label("Dropped Files");

        ui.horizontal(|ui| {
            ui.label("Quote style:");
            egui::ComboBox::from_id_salt("input_dropped_file_quote_style")
                .selected_text(settings.config.dropped_file_quote_style.display_name())
                .show_ui(ui, |ui| {
                    for style in DroppedFileQuoteStyle::all() {
                        if ui
                            .selectable_value(
                                &mut settings.config.dropped_file_quote_style,
                                *style,
                                style.display_name(),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        })
        .response
        .on_hover_text("How to quote file paths when dropped into the terminal");
    });
}

// ============================================================================
// Clipboard Limits Section
// ============================================================================

fn show_clipboard_limits_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(
        ui,
        "Clipboard Limits",
        "input_clipboard_limits",
        false,
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Max clipboard sync events:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(&mut settings.config.clipboard_max_sync_events, 8..=256),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Max clipboard event bytes:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.clipboard_max_event_bytes,
                            512..=16384,
                        ),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        },
    );
}

// ============================================================================
// Word Selection Section
// ============================================================================

fn show_word_selection_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Word Selection", "input_word_selection", false, |ui| {
        ui.horizontal(|ui| {
            ui.label("Word characters:");
            if ui
                .add(
                    egui::TextEdit::singleline(&mut settings.config.word_characters)
                        .hint_text("/-+\\~_.")
                        .desired_width(150.0),
                )
                .on_hover_text("Characters considered part of a word (in addition to alphanumeric)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.smart_selection_enabled,
                "Enable smart selection",
            )
            .on_hover_text("Double-click will try to match patterns like URLs, emails, paths")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if settings.config.smart_selection_enabled {
            ui.separator();
            ui.label("Smart Selection Rules");
            ui.label(
                egui::RichText::new("Higher precision rules are checked first")
                    .small()
                    .weak(),
            );

            egui::ScrollArea::vertical()
                .max_height(150.0)
                .show(ui, |ui| {
                    for rule in &mut settings.config.smart_selection_rules {
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut rule.enabled, "").changed() {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                            let label = egui::RichText::new(&rule.name);
                            let label = if rule.enabled {
                                label
                            } else {
                                label.strikethrough().weak()
                            };
                            ui.label(label).on_hover_ui(|ui| {
                                ui.label(format!("Pattern: {}", rule.regex));
                                ui.label(format!("Precision: {:?}", rule.precision));
                            });
                        });
                    }
                });

            if ui
                .button("Reset rules to defaults")
                .on_hover_text("Replace all rules with the default set")
                .clicked()
            {
                settings.config.smart_selection_rules =
                    crate::config::default_smart_selection_rules();
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        }
    });
}

// ============================================================================
// Keybindings Section
// ============================================================================

/// All available keybinding actions with their descriptions and default key combos.
/// macOS uses Cmd as the primary modifier (safe for terminals).
/// Windows/Linux uses Ctrl+Shift to avoid conflicts with terminal control codes.
#[cfg(target_os = "macos")]
const AVAILABLE_ACTIONS: &[(&str, &str, Option<&str>)] = &[
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
const AVAILABLE_ACTIONS: &[(&str, &str, Option<&str>)] = &[
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

pub(super) fn display_key_combo(combo: &str) -> String {
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

// ============================================================================
// Copy Mode Section
// ============================================================================

fn show_copy_mode_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Copy Mode", "input_copy_mode", true, |ui| {
        ui.label(
            egui::RichText::new(
                "Vi-style keyboard-driven text selection and navigation. \
                 Activate via the toggle_copy_mode keybinding action.",
            )
            .weak()
            .size(11.0),
        );
        ui.add_space(4.0);

        if ui
            .checkbox(&mut settings.config.copy_mode_enabled, "Enable copy mode")
            .on_hover_text(
                "Allow entering copy mode via the toggle_copy_mode keybinding action. \
                 When disabled, the keybinding action is ignored.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.copy_mode_auto_exit_on_yank,
                "Auto-exit on yank",
            )
            .on_hover_text(
                "Automatically exit copy mode after yanking (copying) selected text. \
                 When disabled, copy mode stays active after pressing y so you can \
                 continue selecting.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.copy_mode_show_status,
                "Show status bar",
            )
            .on_hover_text(
                "Display a status bar at the bottom of the terminal when copy mode is active. \
                 Shows the current mode (COPY/VISUAL/V-LINE/V-BLOCK/SEARCH) and cursor position.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Tip: Add a keybinding with action \"toggle_copy_mode\" to activate. \
                 In copy mode: hjkl to move, v/V/Ctrl+V for visual select, y to yank, \
                 /? to search, Esc/q to exit.",
            )
            .weak()
            .italics()
            .size(10.5),
        );
    });
}

fn show_keybindings_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Keybindings", "input_keybindings", true, |ui| {
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
    });
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

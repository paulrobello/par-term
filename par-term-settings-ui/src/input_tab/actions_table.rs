//! `AVAILABLE_ACTIONS` lookup table — platform-split keybinding action definitions.
//!
//! Each entry is `(action_id, display_name, default_key_combo)`.
//! macOS uses Cmd as the primary modifier; Windows/Linux uses Ctrl+Shift.

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
    ("duplicate_tab", "Duplicate Tab", Some("Cmd+Shift+N")),
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
    ("duplicate_tab", "Duplicate Tab", Some("Ctrl+Shift+N")),
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

//! `AVAILABLE_ACTIONS` lookup table — platform-split keybinding action definitions.
//!
//! Each entry is `(action_id, display_name, default_key_combo)`.
//! macOS uses Cmd as the primary modifier; Windows/Linux uses Ctrl+Shift.
//!
//! The third column is shown to the user as that action's default chord. It is
//! hand-maintained, and the root crate gates it: see
//! `par-term/src/app/input_events/key_handler/chord_tests.rs`, which resolves
//! every advertised chord against the real dispatch precedence and fails when
//! a higher-precedence layer claims it first. That is why this table is `pub`.
//!
//! # What earns a chord in the third column
//!
//! Only chords par-term's own key handling dispatches — a shipped
//! `Config::default().keybindings` entry, or a hardcoded key layer. The
//! menu-advertised chords became registry defaults (`defaults::menu_chords`,
//! UX K2), so they are real defaults the user can rebind and appear here.
//! `close_window` (the smart close) is `None` on both platforms: `Cmd+W` /
//! `Ctrl+Shift+W` belong to the pane-cascading `close_pane` (UX.md I15). A
//! chord that exists *only* as a native menu-bar accelerator is deliberately
//! left `None` — the not-macOS half is shared with Linux, whose in-app menu
//! only draws accelerator labels, so such a chord would do nothing there.

/// All available keybinding actions with their descriptions and default key combos.
/// macOS uses Cmd as the primary modifier (safe for terminals).
/// Windows/Linux uses Ctrl+Shift to avoid conflicts with terminal control codes.
#[cfg(target_os = "macos")]
pub const AVAILABLE_ACTIONS: &[(&str, &str, Option<&str>)] = &[
    ("toggle_help", "Toggle Help Panel", Some("F1")),
    ("new_window", "New Window", Some("Cmd+N")),
    // UX.md A14: `close_window` closes the whole window (all tabs); the
    // smart close (tab, else window) lives on as `close_tab_or_window`.
    // Neither has a chord: Cmd+W is iTerm2's pane-cascading Close
    // (`close_pane`, UX.md I15).
    ("close_window", "Close Window", None),
    ("close_tab_or_window", "Close Tab or Window", None),
    // UX.md A13: window cycling and window by number, palette and bindable.
    // No defaults: iTerm2's Cmd+Opt+digit has not been checked against the
    // registry's Option-key matching, so it ships unbound.
    ("next_window", "Next Window", None),
    ("prev_window", "Previous Window", None),
    ("switch_to_window_1", "Switch to Window 1", None),
    ("switch_to_window_2", "Switch to Window 2", None),
    ("switch_to_window_3", "Switch to Window 3", None),
    ("switch_to_window_4", "Switch to Window 4", None),
    ("switch_to_window_5", "Switch to Window 5", None),
    ("switch_to_window_6", "Switch to Window 6", None),
    ("switch_to_window_7", "Switch to Window 7", None),
    ("switch_to_window_8", "Switch to Window 8", None),
    ("switch_to_window_9", "Switch to Window 9", None),
    ("save_arrangement", "Save Window Arrangement", None),
    ("quit", "Quit par-term", Some("Cmd+Q")),
    ("select_all", "Select All", Some("Cmd+A")),
    ("toggle_menu", "Open Application Menu", None),
    ("toggle_fps_overlay", "Toggle FPS Overlay", Some("F3")),
    ("reload_config", "Reload Configuration", Some("F5")),
    ("toggle_fullscreen", "Toggle Fullscreen", Some("F11")),
    ("open_settings", "Open Settings", Some("F12")),
    ("toggle_search", "Toggle Search", Some("Cmd+F")),
    (
        "toggle_profile_drawer",
        "Toggle Profile Drawer",
        Some("Cmd+O"),
    ),
    ("reload_dynamic_profiles", "Reload Dynamic Profiles", None),
    (
        "toggle_clipboard_history",
        "Toggle Clipboard History",
        Some("Cmd+Shift+H"),
    ),
    // iTerm2's Cmd+Shift+; — spelled with the shifted character the key
    // press produces, which is what the registry matches.
    (
        "toggle_command_history",
        "Toggle Command History",
        Some("Cmd+Shift+:"),
    ),
    (
        "toggle_ai_inspector",
        "Toggle Assistant Panel",
        Some("Cmd+I"),
    ),
    (
        "maximize_vertically",
        "Maximize Vertically",
        Some("Shift+F11"),
    ),
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
    ("cycle_background_shader", "Cycle Background Shader", None),
    (
        "toggle_shader_animation",
        "Pause/Resume Shader Animation",
        None,
    ),
    (
        "toggle_shader_readability_mode",
        "Toggle Shader Readability Mode",
        None,
    ),
    ("new_tab", "New Tab", Some("Cmd+T")),
    ("close_tab", "Close Tab", Some("Cmd+Alt+W")),
    ("duplicate_tab", "Duplicate Tab", Some("Cmd+Shift+J")),
    (
        "reopen_closed_tab",
        "Reopen Closed Tab",
        Some("Cmd+Shift+T"),
    ),
    ("move_tab_to_new_window", "Move Tab to New Window", None),
    // UX.md A10-A12, A21: palette and bindable, no default chord.
    ("last_tab", "Last-Used Tab", None),
    ("go_to_last_tab", "Go to Last Tab", None),
    ("rename_tab", "Rename Tab", None),
    ("close_other_tabs", "Close Other Tabs", None),
    ("close_tabs_to_right", "Close Tabs to the Right", None),
    // UX.md A18: the keyboard version of the tab context submenu.
    ("move_tab_to_window_picker", "Move Tab to Window...", None),
    ("next_tab", "Next Tab", Some("Cmd+Shift+]")),
    ("prev_tab", "Previous Tab", Some("Cmd+Shift+[")),
    ("move_tab_left", "Move Tab Left", Some("Cmd+Alt+Shift+[")),
    ("move_tab_right", "Move Tab Right", Some("Cmd+Alt+Shift+]")),
    ("switch_to_tab_1", "Switch to Tab 1", Some("Cmd+1")),
    ("switch_to_tab_2", "Switch to Tab 2", Some("Cmd+2")),
    ("switch_to_tab_3", "Switch to Tab 3", Some("Cmd+3")),
    ("switch_to_tab_4", "Switch to Tab 4", Some("Cmd+4")),
    ("switch_to_tab_5", "Switch to Tab 5", Some("Cmd+5")),
    ("switch_to_tab_6", "Switch to Tab 6", Some("Cmd+6")),
    ("switch_to_tab_7", "Switch to Tab 7", Some("Cmd+7")),
    ("switch_to_tab_8", "Switch to Tab 8", Some("Cmd+8")),
    ("switch_to_tab_9", "Switch to Tab 9", Some("Cmd+9")),
    ("split_right", "Split Right", Some("Cmd+D")),
    ("split_down", "Split Down", Some("Cmd+Shift+D")),
    ("close_pane", "Close Pane", Some("Cmd+W")),
    ("promote_pane_to_tab", "Promote Pane to Tab", None),
    ("rename_pane", "Rename Pane", None),
    ("demote_tab_to_pane", "Demote Tab to Pane", None),
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
        "swap_pane_left",
        "Swap Pane Left",
        Some("Cmd+Alt+Shift+Left"),
    ),
    (
        "swap_pane_right",
        "Swap Pane Right",
        Some("Cmd+Alt+Shift+Right"),
    ),
    ("swap_pane_up", "Swap Pane Up", Some("Cmd+Alt+Shift+Up")),
    (
        "swap_pane_down",
        "Swap Pane Down",
        Some("Cmd+Alt+Shift+Down"),
    ),
    (
        "select_pane_hint",
        "Select Pane by Letter",
        Some("Cmd+Alt+P"),
    ),
    (
        "resize_pane_left",
        "Resize Pane Left",
        Some("Cmd+Ctrl+Left"),
    ),
    (
        "resize_pane_right",
        "Resize Pane Right",
        Some("Cmd+Ctrl+Right"),
    ),
    ("resize_pane_up", "Resize Pane Up", Some("Cmd+Ctrl+Up")),
    (
        "resize_pane_down",
        "Resize Pane Down",
        Some("Cmd+Ctrl+Down"),
    ),
    (
        "toggle_pane_zoom",
        "Zoom Pane (Maximize Active Pane)",
        Some("Cmd+Shift+Enter"),
    ),
    ("equalize_panes", "Equalize Panes", Some("Cmd+Alt+=")),
    ("next_pane", "Next Pane", Some("Cmd+]")),
    ("prev_pane", "Previous Pane", Some("Cmd+[")),
    ("last_pane", "Last-Focused Pane", None),
    ("restart_pane", "Restart Pane Process", None),
    ("cycle_layout", "Cycle Layout Presets", None),
    ("split_left", "Split Left", None),
    ("split_up", "Split Up", None),
    ("enter_resize_mode", "Resize Panes with Arrow Keys", None),
    // The font-size layer in the root crate uses the super key on macOS
    // (`font_mod = super_key` under `#[cfg(target_os = "macos")]`), so Ctrl does
    // not drive these actions here.
    ("increase_font_size", "Increase Font Size", Some("Cmd+=")),
    (
        "decrease_font_size",
        "Decrease Font Size",
        Some("Cmd+Minus"),
    ),
    ("reset_font_size", "Reset Font Size", Some("Cmd+0")),
    ("clear_scrollback", "Clear Scrollback", Some("Cmd+Shift+K")),
    ("clear_screen", "Clear Screen", Some("Ctrl+L")),
    // `Ctrl+,`, not `Cmd+,`, on macOS too. `Cmd+,` is the `Settings...` key
    // equivalent on the NSApp application menu (`menu/macos.rs`) and is
    // consumed before winit delivers a key event at all; `Ctrl+Comma` is the
    // shipped registry default on every platform. Same string as the
    // non-macOS table below.
    (
        "cycle_cursor_style",
        "Cycle Cursor Style",
        Some("Ctrl+Comma"),
    ),
    (
        "paste_special",
        "Paste Special (Transform)",
        Some("Cmd+Shift+V"),
    ),
    (
        "toggle_session_logging",
        "Toggle Output Recording",
        Some("Cmd+Shift+R"),
    ),
    (
        "toggle_broadcast_input",
        "Toggle Broadcast Input",
        Some("Cmd+Alt+I"),
    ),
    (
        "toggle_pane_broadcast",
        "Toggle Broadcast for Current Pane",
        Some("Cmd+Ctrl+Alt+I"),
    ),
    // Palette only: Cmd+Shift+T reopens a closed tab (UX.md I17).
    ("toggle_throughput_mode", "Toggle Throughput Mode", None),
    ("toggle_always_on_top", "Toggle Always on Top", None),
    (
        "toggle_session_picker",
        "Toggle Session Picker",
        Some("Cmd+Ctrl+S"),
    ),
    (
        "ssh_quick_connect",
        "SSH Quick Connect",
        Some("Cmd+Shift+S"),
    ),
    ("toggle_copy_mode", "Toggle Copy Mode", Some("Cmd+Shift+C")),
    ("enter_copy_mode", "Enter Copy Mode", None),
    (
        "toggle_command_palette",
        "Open Command Palette",
        Some("Cmd+Shift+P"),
    ),
    ("toggle_agent_usage_panel", "Toggle Agent Usage Panel", None),
    // UX.md A23: public detach id (`mux-detach` is an alias).
    ("detach", "Detach from par-mux Session", None),
    // UX.md A22: a new par-mux session without a profile.
    ("new_mux_session", "New par-mux Session", None),
    (
        "focus_next_attention_agent",
        "Next Agent Needing Attention",
        Some("Cmd+Alt+A"),
    ),
    (
        "toggle_tree_picker",
        "Open Quickly (Windows, Tabs, Panes)",
        Some("Cmd+Shift+O"),
    ),
];

#[cfg(not(target_os = "macos"))]
pub const AVAILABLE_ACTIONS: &[(&str, &str, Option<&str>)] = &[
    ("toggle_help", "Toggle Help Panel", Some("F1")),
    ("new_window", "New Window", Some("Ctrl+Shift+N")),
    ("close_window", "Close Window", None),
    ("close_tab_or_window", "Close Tab or Window", None),
    ("next_window", "Next Window", None),
    ("prev_window", "Previous Window", None),
    ("switch_to_window_1", "Switch to Window 1", None),
    ("switch_to_window_2", "Switch to Window 2", None),
    ("switch_to_window_3", "Switch to Window 3", None),
    ("switch_to_window_4", "Switch to Window 4", None),
    ("switch_to_window_5", "Switch to Window 5", None),
    ("switch_to_window_6", "Switch to Window 6", None),
    ("switch_to_window_7", "Switch to Window 7", None),
    ("switch_to_window_8", "Switch to Window 8", None),
    ("switch_to_window_9", "Switch to Window 9", None),
    ("save_arrangement", "Save Window Arrangement", None),
    ("quit", "Quit par-term", Some("Ctrl+Shift+Q")),
    ("select_all", "Select All", Some("Ctrl+Shift+A")),
    ("toggle_menu", "Open Application Menu", None),
    ("toggle_fps_overlay", "Toggle FPS Overlay", Some("F3")),
    ("reload_config", "Reload Configuration", Some("F5")),
    ("toggle_fullscreen", "Toggle Fullscreen", Some("F11")),
    ("open_settings", "Open Settings", Some("F12")),
    ("toggle_search", "Toggle Search", Some("Ctrl+Shift+F")),
    // No chord here: Ctrl+Shift+P is the command palette (UX.md D2) and the
    // K1 translation of iTerm2's Cmd+O, Ctrl+Shift+O, is split down. The
    // Profiles menu and the palette still reach the drawer.
    ("toggle_profile_drawer", "Toggle Profile Drawer", None),
    ("reload_dynamic_profiles", "Reload Dynamic Profiles", None),
    (
        "toggle_clipboard_history",
        "Toggle Clipboard History",
        Some("Ctrl+Shift+H"),
    ),
    (
        "toggle_command_history",
        "Toggle Command History",
        Some("Ctrl+Alt+R"),
    ),
    (
        "toggle_ai_inspector",
        "Toggle Assistant Panel",
        Some("Ctrl+Shift+I"),
    ),
    (
        "maximize_vertically",
        "Maximize Vertically",
        Some("Shift+F11"),
    ),
    (
        "toggle_background_shader",
        "Toggle Background Shader",
        Some("Ctrl+Alt+B"),
    ),
    (
        "toggle_cursor_shader",
        "Toggle Cursor Shader",
        Some("Ctrl+Shift+U"),
    ),
    ("cycle_background_shader", "Cycle Background Shader", None),
    (
        "toggle_shader_animation",
        "Pause/Resume Shader Animation",
        None,
    ),
    (
        "toggle_shader_readability_mode",
        "Toggle Shader Readability Mode",
        None,
    ),
    ("new_tab", "New Tab", Some("Ctrl+Shift+T")),
    ("close_tab", "Close Tab", Some("Ctrl+Alt+W")),
    ("duplicate_tab", "Duplicate Tab", Some("Ctrl+Shift+J")),
    (
        "reopen_closed_tab",
        "Reopen Closed Tab",
        Some("Ctrl+Shift+Z"),
    ),
    ("move_tab_to_new_window", "Move Tab to New Window", None),
    // UX.md A10-A12, A21: palette and bindable, no default chord.
    ("last_tab", "Last-Used Tab", None),
    ("go_to_last_tab", "Go to Last Tab", None),
    ("rename_tab", "Rename Tab", None),
    ("close_other_tabs", "Close Other Tabs", None),
    ("close_tabs_to_right", "Close Tabs to the Right", None),
    // UX.md A18: the keyboard version of the tab context submenu.
    ("move_tab_to_window_picker", "Move Tab to Window...", None),
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
    ("split_right", "Split Right", Some("Ctrl+Shift+E")),
    ("split_down", "Split Down", Some("Ctrl+Shift+O")),
    ("close_pane", "Close Pane", Some("Ctrl+Shift+W")),
    ("promote_pane_to_tab", "Promote Pane to Tab", None),
    ("rename_pane", "Rename Pane", None),
    ("demote_tab_to_pane", "Demote Tab to Pane", None),
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
        "select_pane_hint",
        "Select Pane by Letter",
        Some("Ctrl+Alt+P"),
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
        "toggle_pane_zoom",
        "Zoom Pane (Maximize Active Pane)",
        Some("Ctrl+Shift+Enter"),
    ),
    ("equalize_panes", "Equalize Panes", Some("Ctrl+Alt+=")),
    ("next_pane", "Next Pane", Some("Ctrl+Alt+]")),
    ("prev_pane", "Previous Pane", Some("Ctrl+Alt+[")),
    ("last_pane", "Last-Focused Pane", None),
    ("restart_pane", "Restart Pane Process", None),
    ("cycle_layout", "Cycle Layout Presets", None),
    ("split_left", "Split Left", None),
    ("split_up", "Split Up", None),
    ("enter_resize_mode", "Resize Panes with Arrow Keys", None),
    // Unbound here (UX.md K16): editors use Alt+Shift+Arrow and Windows
    // switches input language on Alt+Shift.
    ("swap_pane_left", "Swap Pane Left", None),
    ("swap_pane_right", "Swap Pane Right", None),
    ("swap_pane_up", "Swap Pane Up", None),
    ("swap_pane_down", "Swap Pane Down", None),
    (
        "increase_font_size",
        "Increase Font Size",
        Some("Ctrl+Shift+="),
    ),
    (
        "decrease_font_size",
        "Decrease Font Size",
        Some("Ctrl+Shift+-"),
    ),
    ("reset_font_size", "Reset Font Size", Some("Ctrl+Shift+0")),
    ("clear_scrollback", "Clear Scrollback", Some("Ctrl+Shift+K")),
    ("clear_screen", "Clear Screen", Some("Ctrl+L")),
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
        "Toggle Output Recording",
        Some("Ctrl+Shift+R"),
    ),
    (
        "toggle_broadcast_input",
        "Toggle Broadcast Input",
        Some("Ctrl+Alt+I"),
    ),
    (
        "toggle_pane_broadcast",
        "Toggle Broadcast for Current Pane",
        None,
    ),
    (
        "toggle_throughput_mode",
        "Toggle Throughput Mode",
        Some("Ctrl+Shift+M"),
    ),
    ("toggle_always_on_top", "Toggle Always on Top", None),
    (
        "toggle_session_picker",
        "Toggle Session Picker",
        Some("Ctrl+Alt+T"),
    ),
    (
        "ssh_quick_connect",
        "SSH Quick Connect",
        Some("Ctrl+Shift+S"),
    ),
    (
        "toggle_copy_mode",
        "Toggle Copy Mode",
        Some("Ctrl+Shift+Space"),
    ),
    ("enter_copy_mode", "Enter Copy Mode", None),
    (
        "toggle_command_palette",
        "Open Command Palette",
        Some("Ctrl+Shift+P"),
    ),
    ("toggle_agent_usage_panel", "Toggle Agent Usage Panel", None),
    // UX.md A23: public detach id (`mux-detach` is an alias).
    ("detach", "Detach from par-mux Session", None),
    // UX.md A22: a new par-mux session without a profile.
    ("new_mux_session", "New par-mux Session", None),
    (
        "focus_next_attention_agent",
        "Next Agent Needing Attention",
        Some("Ctrl+Alt+A"),
    ),
    (
        "toggle_tree_picker",
        "Open Quickly (Windows, Tabs, Panes)",
        Some("Ctrl+Alt+O"),
    ),
];

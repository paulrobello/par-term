//! The Shell, Edit, and View menus (UX.md 21.2).
//!
//! Every item that has a registry action runs it ([`MenuItemSpec::action`]),
//! so its accelerator is the live binding and its behavior is the chord's.
//! The few menu-only commands (Copy, Paste) carry the platform's fixed chord.

use super::actions::MenuAction;
use super::model::{MenuEntry, MenuItemSpec, primary, submenu};
use super::state::{Check, Requires};
use muda::accelerator::Code;

/// Shell (replaces File + Tab): windows, tabs, splits, closing, broadcast,
/// par-mux/tmux sessions, SSH, and Quit off macOS.
pub(super) fn shell(has_native_app_menu: bool) -> Vec<MenuEntry> {
    let mut entries: Vec<MenuEntry> = vec![
        MenuItemSpec::new("new_window", "New Window", MenuAction::NewWindow).into(),
        MenuItemSpec::action("new_tab", "New Tab").into(),
        MenuItemSpec::action("duplicate_tab", "Duplicate Tab").into(),
        MenuEntry::Separator,
        MenuItemSpec::action("split_right", "Split Right").into(),
        MenuItemSpec::action("split_down", "Split Down").into(),
        MenuItemSpec::action("split_left", "Split Left").into(),
        MenuItemSpec::action("split_up", "Split Up").into(),
        MenuEntry::Separator,
        // iTerm2's Close (UX.md I15): the focused pane, cascading to the tab
        // and then the window.
        MenuItemSpec::action("close_pane", "Close").into(),
        MenuItemSpec::action("close_tab", "Close Tab").into(),
        MenuItemSpec::new("close_window", "Close Window", MenuAction::CloseWholeWindow).into(),
        MenuItemSpec::action("close_other_tabs", "Close Other Tabs")
            .when(Requires::MultipleTabs)
            .into(),
        MenuItemSpec::action("close_tabs_to_right", "Close Tabs to the Right")
            .when(Requires::MultipleTabs)
            .into(),
        MenuItemSpec::action("reopen_closed_tab", "Reopen Closed Tab").into(),
        MenuEntry::Separator,
        submenu(
            "broadcast_input",
            "Broadcast Input",
            Requires::Always,
            vec![
                MenuItemSpec::action("toggle_broadcast_input", "All Panes in Current Tab")
                    .toggle(Check::BroadcastTab)
                    .into(),
                MenuItemSpec::action("toggle_pane_broadcast", "Exclude Current Pane")
                    .when(Requires::MultiplePanes)
                    .toggle(Check::PaneBroadcastExcluded)
                    .into(),
            ],
        ),
        MenuEntry::Separator,
        // Attaching and creating stay outside Session ▸: AppKit swallows the
        // key equivalent of every item in a disabled submenu, so the picker's
        // chord must not live in one.
        MenuItemSpec::action("toggle_session_picker", "Attach to Session...").into(),
        MenuItemSpec::action("new_mux_session", "New par-mux Session")
            .when(Requires::MuxAvailable)
            .into(),
        // UX.md MN2/MP2: disabled until the window is attached to a par-mux
        // or tmux session.
        submenu(
            "session_submenu",
            "Session",
            Requires::SessionAttached,
            vec![
                MenuItemSpec::new(
                    "switch_session",
                    "Switch Session...",
                    MenuAction::Action("toggle_session_picker"),
                )
                .alias()
                .into(),
                MenuItemSpec::action("detach", "Detach")
                    .when(Requires::MuxAttached)
                    .into(),
                MenuEntry::Separator,
                MenuItemSpec::action("focus_next_attention_agent", "Next Agent Needing Attention")
                    .when(Requires::MuxAttached)
                    .into(),
            ],
        ),
        submenu(
            "ssh_submenu",
            "SSH",
            Requires::Always,
            vec![
                MenuItemSpec::action("ssh_quick_connect", "Quick Connect...").into(),
                MenuItemSpec::new(
                    "install_remote_shell_integration",
                    "Install Shell Integration on Remote Host...",
                    MenuAction::InstallShellIntegrationRemote,
                )
                .into(),
            ],
        ),
    ];
    if !has_native_app_menu {
        entries.push(MenuEntry::Separator);
        entries.push(MenuItemSpec::new("quit", "Quit", MenuAction::Quit).into());
    }
    entries
}

/// Edit: clipboard, copy mode, find, marks, clearing (the focused pane, B58),
/// histories, and Settings off macOS.
pub(super) fn edit(has_native_app_menu: bool) -> Vec<MenuEntry> {
    let mut entries: Vec<MenuEntry> = vec![
        // Copy and Paste have no registry action: they run dedicated
        // clipboard paths that also serve egui text fields, so they keep
        // the platform's fixed chord.
        MenuItemSpec::new("copy", "Copy", MenuAction::Copy)
            .accel(primary(Code::KeyC))
            .into(),
        MenuItemSpec::new("paste", "Paste", MenuAction::Paste)
            .accel(primary(Code::KeyV))
            .into(),
        MenuItemSpec::action("paste_special", "Paste Special...").into(),
        MenuItemSpec::new("select_all", "Select All", MenuAction::SelectAll).into(),
        MenuEntry::Separator,
        MenuItemSpec::action("toggle_copy_mode", "Copy Mode")
            .toggle(Check::CopyMode)
            .into(),
        MenuItemSpec::action("toggle_search", "Find...").into(),
        submenu(
            "marks",
            "Marks",
            Requires::Always,
            vec![
                MenuItemSpec::action("scroll_to_previous_mark", "Previous Mark").into(),
                MenuItemSpec::action("scroll_to_next_mark", "Next Mark").into(),
            ],
        ),
        submenu(
            "scroll",
            "Scroll",
            Requires::Always,
            vec![
                MenuItemSpec::action("scroll_up_page", "Page Up").into(),
                MenuItemSpec::action("scroll_down_page", "Page Down").into(),
                MenuItemSpec::action("scroll_to_top", "To Top").into(),
                MenuItemSpec::action("scroll_to_bottom", "To Bottom").into(),
            ],
        ),
        MenuEntry::Separator,
        MenuItemSpec::action("clear_screen", "Clear Screen").into(),
        MenuItemSpec::action("clear_scrollback", "Clear Scrollback").into(),
        MenuEntry::Separator,
        MenuItemSpec::action("toggle_clipboard_history", "Clipboard History").into(),
        MenuItemSpec::action("toggle_command_history", "Command History").into(),
    ];
    if !has_native_app_menu {
        // Preferences belongs in Edit on Windows and Linux.
        entries.push(MenuEntry::Separator);
        entries.push(MenuItemSpec::new("settings", "Settings...", MenuAction::OpenSettings).into());
    }
    entries
}

/// View: pickers, full screen and zoom, panels, font size, shaders.
pub(super) fn view() -> Vec<MenuEntry> {
    vec![
        MenuItemSpec::action("toggle_command_palette", "Command Palette...").into(),
        MenuItemSpec::action("toggle_tree_picker", "Open Quickly...").into(),
        MenuEntry::Separator,
        MenuItemSpec::action("toggle_fullscreen", "Full Screen")
            .toggle(Check::Fullscreen)
            .into(),
        MenuItemSpec::action("maximize_vertically", "Maximize Vertically").into(),
        MenuItemSpec::action("toggle_pane_zoom", "Zoom Pane")
            .when(Requires::MultiplePanes)
            .toggle(Check::PaneZoom)
            .into(),
        MenuEntry::Separator,
        // The chord lives on Profiles › Open Profiles... (iTerm2's Cmd+O).
        MenuItemSpec::new(
            "view_profile_drawer",
            "Profile Drawer",
            MenuAction::Action("toggle_profile_drawer"),
        )
        .alias()
        .into(),
        MenuItemSpec::action("toggle_profiles_panel", "Profiles Panel")
            .toggle(Check::ProfileDrawer)
            .into(),
        MenuItemSpec::action("toggle_ai_inspector", "Assistant Panel")
            .toggle(Check::AssistantPanel)
            .into(),
        MenuItemSpec::action("toggle_agent_usage_panel", "Agent Usage")
            .toggle(Check::AgentUsage)
            .into(),
        MenuItemSpec::action("toggle_fps_overlay", "FPS Overlay")
            .toggle(Check::FpsOverlay)
            .into(),
        MenuEntry::Separator,
        MenuItemSpec::action("increase_font_size", "Make Text Bigger").into(),
        MenuItemSpec::action("reset_font_size", "Make Text Normal Size").into(),
        MenuItemSpec::action("decrease_font_size", "Make Text Smaller").into(),
        MenuItemSpec::action("cycle_cursor_style", "Cycle Cursor Style").into(),
        MenuEntry::Separator,
        MenuItemSpec::action("toggle_background_shader", "Background Shader")
            .toggle(Check::BackgroundShader)
            .into(),
        MenuItemSpec::action("cycle_background_shader", "Next Background Shader").into(),
        MenuItemSpec::action("toggle_shader_animation", "Animate Background Shader")
            .toggle(Check::ShaderAnimation)
            .into(),
        MenuItemSpec::action("toggle_shader_readability_mode", "Shader Readability Mode")
            .toggle(Check::ShaderReadability)
            .into(),
        MenuItemSpec::action("toggle_cursor_shader", "Cursor Shader")
            .toggle(Check::CursorShader)
            .into(),
        MenuItemSpec::action("toggle_throughput_mode", "Throughput Mode")
            .toggle(Check::Throughput)
            .into(),
    ]
}

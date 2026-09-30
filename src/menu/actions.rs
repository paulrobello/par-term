//! Menu action definitions for par-term
//!
//! This module defines the `MenuAction` enum that represents all possible
//! menu actions that can be triggered from the native menu system.

use crate::profile::ProfileId;

/// Actions that can be triggered from the menu system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    // File menu
    /// Create a new terminal window
    NewWindow,
    /// Smart close: the active tab when the window has several, else the
    /// window. Emitted by the `close_window` keybinding action; no menu item
    /// carries it since File › Close became [`Self::ClosePane`].
    CloseWindow,
    /// iTerm2's Close (UX.md I15): the focused pane, cascading to the tab
    /// when it was the tab's last pane and to the window after the last tab.
    ClosePane,
    /// Quit the application (only used on Windows/Linux - macOS handles quit via system menu)
    Quit,

    // Profiles menu
    /// Open the profile management modal
    ManageProfiles,
    /// Toggle the profile drawer visibility
    ToggleProfileDrawer,
    /// Open a specific profile (static menu entries for common profiles)
    OpenProfile(ProfileId),

    // Tab menu
    /// Create a new tab
    NewTab,
    /// Close the current tab
    CloseTab,
    /// Switch to next tab
    NextTab,
    /// Switch to previous tab
    PreviousTab,
    /// Switch to tab by index (1-9)
    SwitchToTab(usize),
    /// Move the current tab one position to the left
    MoveTabLeft,
    /// Move the current tab one position to the right
    MoveTabRight,
    /// Duplicate the current tab
    DuplicateTab,

    // Edit menu
    /// Copy selected text to clipboard
    Copy,
    /// Paste from clipboard
    Paste,
    /// Select all text (not typically used in terminals)
    SelectAll,
    /// Clear the scrollback buffer
    ClearScrollback,
    /// Show clipboard history panel
    ClipboardHistory,

    // View menu
    /// Toggle fullscreen mode
    ToggleFullscreen,
    /// Maximize window vertically only (span full screen height)
    MaximizeVertically,
    /// Increase font size
    IncreaseFontSize,
    /// Decrease font size
    DecreaseFontSize,
    /// Reset font size to default
    ResetFontSize,
    /// Toggle FPS overlay
    ToggleFpsOverlay,
    /// Open settings panel
    OpenSettings,

    // Window menu (macOS)
    /// Minimize the window
    Minimize,
    /// Zoom/maximize the window
    Zoom,
    /// Toggle the focused window's always-on-top level (UX.md A20)
    ToggleAlwaysOnTop,

    // Help menu
    /// Show keyboard shortcuts help
    ShowHelp,
    /// Show about dialog
    About,

    // Window Arrangements
    /// Save the current window layout as an arrangement
    SaveArrangement,

    // Shell menu
    /// Install shell integration on a remote host via curl
    InstallShellIntegrationRemote,

    // Keybinding actions (triggered by user-defined keybindings or menu)
    /// Toggle background/custom shader on/off
    ToggleBackgroundShader,
    /// Toggle cursor shader on/off
    ToggleCursorShader,
    /// Reload configuration from disk (same as F5)
    ReloadConfig,
}

impl MenuAction {
    /// The keybinding-registry action id this menu entry dispatches to, or
    /// `None` when no registry action exists for it.
    ///
    /// The menu model re-reads its accelerators from the registry through
    /// this mapping (`registry_accel::apply_registry_accelerators`), so a
    /// menu item and its registry binding cannot drift apart. `None` covers
    /// items with no registry action: Copy/Paste (dedicated clipboard paths,
    /// not `ACTION_HANDLERS`), profile entries, the macOS Window menu, About,
    /// and remote shell integration.
    pub fn keybinding_action(&self) -> Option<std::borrow::Cow<'static, str>> {
        let id = match self {
            Self::NewWindow => "new_window",
            Self::CloseWindow => "close_window",
            Self::ClosePane => "close_pane",
            Self::Quit => "quit",
            Self::ToggleProfileDrawer => "toggle_profile_drawer",
            Self::NewTab => "new_tab",
            Self::CloseTab => "close_tab",
            Self::NextTab => "next_tab",
            Self::PreviousTab => "prev_tab",
            Self::MoveTabLeft => "move_tab_left",
            Self::MoveTabRight => "move_tab_right",
            Self::DuplicateTab => "duplicate_tab",
            Self::SelectAll => "select_all",
            Self::ClearScrollback => "clear_scrollback",
            Self::ClipboardHistory => "toggle_clipboard_history",
            Self::ToggleFullscreen => "toggle_fullscreen",
            Self::MaximizeVertically => "maximize_vertically",
            Self::IncreaseFontSize => "increase_font_size",
            Self::DecreaseFontSize => "decrease_font_size",
            Self::ResetFontSize => "reset_font_size",
            Self::ToggleFpsOverlay => "toggle_fps_overlay",
            Self::OpenSettings => "open_settings",
            Self::ShowHelp => "toggle_help",
            Self::SaveArrangement => "save_arrangement",
            Self::ToggleBackgroundShader => "toggle_background_shader",
            Self::ToggleCursorShader => "toggle_cursor_shader",
            Self::ReloadConfig => "reload_config",
            Self::ToggleAlwaysOnTop => "toggle_always_on_top",
            Self::SwitchToTab(n) => {
                return Some(std::borrow::Cow::Owned(format!("switch_to_tab_{n}")));
            }
            Self::OpenProfile(_)
            | Self::ManageProfiles
            | Self::Copy
            | Self::Paste
            | Self::Minimize
            | Self::Zoom
            | Self::About
            | Self::InstallShellIntegrationRemote => return None,
        };
        Some(std::borrow::Cow::Borrowed(id))
    }
}

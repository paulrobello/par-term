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
    /// Close the current window
    CloseWindow,
    /// Quit the application (only used on Windows/Linux - macOS handles quit via system menu)
    #[allow(dead_code)]
    Quit,

    // Profiles menu
    /// Open the profile management modal
    ManageProfiles,
    /// Toggle the profile drawer visibility
    ToggleProfileDrawer,
    /// Open a specific profile (static menu entries for common profiles)
    #[allow(dead_code)]
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
    /// Move tab left (not yet implemented)
    #[allow(dead_code)]
    MoveTabLeft,
    /// Move tab right (not yet implemented)
    #[allow(dead_code)]
    MoveTabRight,
    /// Duplicate the current tab (not yet implemented)
    #[allow(dead_code)]
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

    // Help menu
    /// Show keyboard shortcuts help
    ShowHelp,
    /// Show about dialog
    About,

    // Window Arrangements
    /// Save the current window layout as an arrangement
    SaveArrangement,

    // Keybinding actions (triggered by user-defined keybindings or menu)
    /// Toggle background/custom shader on/off
    #[allow(dead_code)]
    ToggleBackgroundShader,
    /// Toggle cursor shader on/off
    #[allow(dead_code)]
    ToggleCursorShader,
    /// Reload configuration from disk (same as F5)
    #[allow(dead_code)]
    ReloadConfig,
}

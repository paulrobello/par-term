//! Menu action definitions for par-term
//!
//! This module defines the `MenuAction` enum that represents all possible
//! menu actions that can be triggered from the native menu system.

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
}

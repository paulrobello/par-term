//! Menu action definitions for par-term
//!
//! This module defines the `MenuAction` enum that represents all possible
//! menu actions that can be triggered from the native menu system.

use crate::arrangements::ArrangementId;
use crate::profile::ProfileId;

/// Actions that can be triggered from the menu system
///
/// Most menu items carry [`Self::Action`]: the item runs the registry
/// action of that id through `execute_keybinding_action`, the same handler
/// its chord runs, so a menu item and its keybinding cannot diverge (UX.md
/// MN1). The other variants are manager-level commands (they act on every
/// window, or on no window) and the few menu-only commands with no registry
/// action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// Create a new terminal window
    NewWindow,
    /// Smart close: the active tab when the window has several, else the
    /// window. Emitted by the `close_tab_or_window` keybinding action (UX.md
    /// A14 moved it off `close_window`); no menu item carries it.
    CloseWindow,
    /// Close the whole focused window with all its tabs (UX.md A14), through
    /// the title-bar close's confirmation.
    CloseWholeWindow,
    /// Focus the next (+1) or previous (-1) window in window-number order
    /// (UX.md A13).
    CycleWindow(i8),
    /// Focus the window holding number N (UX.md A13, iTerm2 Cmd+Opt+N).
    FocusWindowNumber(usize),
    /// Quit the application
    Quit,

    /// Open a specific profile in a new tab
    OpenProfile(ProfileId),

    /// Copy selected text to clipboard
    Copy,
    /// Paste from clipboard
    Paste,
    /// Select all text in the focused terminal or text field
    SelectAll,

    /// Open settings panel
    OpenSettings,

    /// Minimize the window
    Minimize,
    /// Zoom/maximize the window
    Zoom,

    /// Show about dialog
    About,
    /// Open the documentation in the browser
    OpenDocs,

    /// Save the current window layout as an arrangement
    SaveArrangement,
    /// Restore a saved arrangement
    RestoreArrangement(ArrangementId),

    /// Install shell integration on a remote host via curl
    InstallShellIntegrationRemote,

    /// Run a registry action by id in the focused window. The id is the
    /// item's registry action, so its accelerator comes from the registry
    /// like every other item.
    Action(&'static str),
}

impl MenuAction {
    /// The keybinding-registry action id this menu entry dispatches to, or
    /// `None` when no registry action exists for it.
    ///
    /// The menu model reads its accelerators from the registry through this
    /// mapping (`registry_accel::apply_registry_accelerators`, and the same
    /// chain inside `model::profile_entries` for the dynamic profile items),
    /// so a menu item and its registry binding cannot drift apart. `None`
    /// covers the menu-only commands: Copy/Paste (dedicated clipboard paths,
    /// not `ACTION_HANDLERS`), arrangement entries, the window-level
    /// Minimize/Zoom, About, the docs link, and remote shell integration.
    pub fn keybinding_action(&self) -> Option<std::borrow::Cow<'static, str>> {
        let id = match self {
            Self::NewWindow => "new_window",
            Self::CloseWindow => "close_tab_or_window",
            Self::CloseWholeWindow => "close_window",
            Self::CycleWindow(1) => "next_window",
            Self::CycleWindow(_) => "prev_window",
            Self::FocusWindowNumber(n) => {
                return Some(std::borrow::Cow::Owned(format!("switch_to_window_{n}")));
            }
            // The same id `ProfileAction::OpenTab(id).id()` dispatches, so a
            // profile item and its migrated chord run one handler (UX.md PR3).
            Self::OpenProfile(id) => {
                return Some(std::borrow::Cow::Owned(format!("open_profile:{id}")));
            }
            Self::Quit => "quit",
            Self::SelectAll => "select_all",
            Self::OpenSettings => "open_settings",
            Self::SaveArrangement => "save_arrangement",
            Self::Action(id) => id,
            Self::RestoreArrangement(_)
            | Self::Copy
            | Self::Paste
            | Self::Minimize
            | Self::Zoom
            | Self::About
            | Self::OpenDocs
            | Self::InstallShellIntegrationRemote => return None,
        };
        Some(std::borrow::Cow::Borrowed(id))
    }
}

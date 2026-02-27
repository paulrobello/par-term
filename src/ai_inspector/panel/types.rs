//! Public types for the AI Inspector panel.
//!
//! Contains `ViewMode`, `InspectorAction`, scope options, and color/layout
//! constants shared across panel sub-modules.

use egui::{Color32, Stroke};

use crate::ai_inspector::snapshot::SnapshotScope;
use crate::ui_constants::AI_PANEL_RESIZE_HANDLE_WIDTH;

/// View mode for displaying snapshot data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Cards,
    Timeline,
    Tree,
    ListDetail,
}

impl ViewMode {
    /// Human-readable label for this view mode.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Cards => "Cards",
            Self::Timeline => "Timeline",
            Self::Tree => "Tree",
            Self::ListDetail => "List Detail",
        }
    }

    /// All available view modes.
    pub fn all() -> &'static [ViewMode] {
        &[
            ViewMode::Cards,
            ViewMode::Timeline,
            ViewMode::Tree,
            ViewMode::ListDetail,
        ]
    }

    /// Parse a view mode from config string.
    pub(super) fn from_config_str(s: &str) -> Self {
        match s {
            "timeline" => Self::Timeline,
            "tree" => Self::Tree,
            "list_detail" => Self::ListDetail,
            _ => Self::Cards,
        }
    }
}

/// Actions returned from the inspector panel to the caller.
#[derive(Debug, Clone)]
pub enum InspectorAction {
    /// No action needed.
    None,
    /// Close the panel.
    Close,
    /// Copy JSON string to clipboard.
    CopyJson(String),
    /// Save JSON string to a file.
    SaveToFile(String),
    /// Write text into the active terminal.
    WriteToTerminal(String),
    /// Run a command in the terminal AND notify the agent it was executed.
    RunCommandAndNotify(String),
    /// Connect to an agent by identity string.
    ConnectAgent(String),
    /// Disconnect from the current agent.
    DisconnectAgent,
    /// Reconnect the current agent to clear session-scoped permission approvals
    /// (including "Always allow" selections).
    RevokeAlwaysAllowSelections,
    /// Send a user prompt to the connected agent.
    SendPrompt(String),
    /// Toggle agent terminal access.
    SetTerminalAccess(bool),
    /// Respond to an agent permission request.
    RespondPermission {
        request_id: u64,
        option_id: String,
        cancelled: bool,
    },
    /// Set the agent's session mode (e.g. "bypassPermissions").
    SetAgentMode(String),
    /// Cancel the current agent prompt.
    CancelPrompt,
    /// Cancel the most recent queued (unsent) user prompt.
    CancelQueuedPrompt,
    /// Clear all chat messages.
    ClearChat,
}

/// Predefined scope options for the dropdown.
pub(super) struct ScopeOption {
    pub(super) label: &'static str,
    pub(super) scope: SnapshotScope,
}

pub(super) const SCOPE_OPTIONS: &[ScopeOption] = &[
    ScopeOption {
        label: "Visible",
        scope: SnapshotScope::Visible,
    },
    ScopeOption {
        label: "Recent 5",
        scope: SnapshotScope::Recent(5),
    },
    ScopeOption {
        label: "Recent 10",
        scope: SnapshotScope::Recent(10),
    },
    ScopeOption {
        label: "Recent 25",
        scope: SnapshotScope::Recent(25),
    },
    ScopeOption {
        label: "Recent 50",
        scope: SnapshotScope::Recent(50),
    },
    ScopeOption {
        label: "Full",
        scope: SnapshotScope::Full,
    },
];

/// Width of the resize handle on the left edge of the panel.
/// Delegated to ui_constants::AI_PANEL_RESIZE_HANDLE_WIDTH.
pub(super) const RESIZE_HANDLE_WIDTH: f32 = AI_PANEL_RESIZE_HANDLE_WIDTH;

/// Panel background color (opaque dark).
pub(super) const PANEL_BG: Color32 = Color32::from_rgba_premultiplied(24, 24, 24, 255);

/// Card background color.
pub(super) const CARD_BG: Color32 = Color32::from_gray(32);

/// Card border stroke.
pub(super) const CARD_BORDER: Stroke = Stroke {
    width: 1.0,
    color: Color32::from_gray(50),
};

/// Exit code success color (green).
pub(super) const EXIT_SUCCESS: Color32 = Color32::from_rgb(76, 175, 80);

/// Exit code failure color (red).
pub(super) const EXIT_FAILURE: Color32 = Color32::from_rgb(244, 67, 54);

/// User message background.
pub(super) const USER_MSG_BG: Color32 = Color32::from_rgb(30, 50, 70);

/// Agent message background.
pub(super) const AGENT_MSG_BG: Color32 = Color32::from_rgb(35, 35, 40);

/// System message color.
pub(super) const SYSTEM_MSG_COLOR: Color32 = Color32::from_gray(110);

/// Command suggestion background.
pub(super) const CMD_SUGGEST_BG: Color32 = Color32::from_rgb(40, 45, 30);

/// Code block background.
pub(super) const CODE_BLOCK_BG: Color32 = Color32::from_rgb(18, 18, 24);

/// Code block language tag color.
pub(super) const CODE_LANG_COLOR: Color32 = Color32::from_gray(90);

/// Connected status color.
pub(super) const AGENT_CONNECTED: Color32 = Color32::from_rgb(76, 175, 80);

/// Disconnected status color.
pub(super) const AGENT_DISCONNECTED: Color32 = Color32::from_gray(100);

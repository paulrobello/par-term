//! Keybinding and modifier key types.

use serde::{Deserialize, Serialize};

// ============================================================================
// Keybinding Types
// ============================================================================

/// Keyboard modifier for keybindings.
///
/// This enum is exported for potential future use (e.g., custom keybinding UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyModifier {
    /// Control key
    Ctrl,
    /// Alt/Option key
    Alt,
    /// Shift key
    Shift,
    /// Cmd on macOS, Ctrl on other platforms (cross-platform convenience)
    CmdOrCtrl,
    /// Always the Cmd/Super/Windows key
    Super,
}

/// A keybinding configuration entry
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyBinding {
    /// Key combination string, e.g., "CmdOrCtrl+Shift+B"
    pub key: String,
    /// Action name, e.g., "toggle_background_shader"
    pub action: String,
}

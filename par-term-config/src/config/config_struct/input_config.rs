//! Keyboard input settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use crate::types::{ModifierRemapping, OptionKeyMode};
use serde::{Deserialize, Serialize};

/// Option/Alt key behaviour, modifier remapping and physical key positions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConfig {
    /// Left Option key (macOS) / Left Alt key (Linux/Windows) behavior
    /// - normal: Sends special characters (default macOS behavior)
    /// - meta: Sets the high bit (8th bit) on the character
    /// - esc: Sends Escape prefix before the character (most compatible for emacs/vim)
    #[serde(default)]
    pub left_option_key_mode: OptionKeyMode,

    /// Right Option key (macOS) / Right Alt key (Linux/Windows) behavior
    /// Can be configured independently from left Option key
    /// - normal: Sends special characters (default macOS behavior)
    /// - meta: Sets the high bit (8th bit) on the character
    /// - esc: Sends Escape prefix before the character (most compatible for emacs/vim)
    #[serde(default)]
    pub right_option_key_mode: OptionKeyMode,

    /// Modifier key remapping configuration
    /// Allows remapping modifier keys to different functions (e.g., swap Ctrl and Caps Lock)
    #[serde(default)]
    pub modifier_remapping: ModifierRemapping,

    /// Use physical key positions for keybindings instead of logical characters
    /// When enabled, keybindings work based on key position (scan code) rather than
    /// the character produced, making shortcuts consistent across keyboard layouts.
    /// For example, Ctrl+Z will always be the bottom-left key regardless of QWERTY/AZERTY/Dvorak.
    #[serde(default = "crate::defaults::bool_false")]
    pub use_physical_keys: bool,

    /// The par-term leader key (UX.md K4/K7): pressing it arms a one-key
    /// table of window, tab, pane, and par-mux session actions that works the
    /// same in local, par-mux, and tmux gateway tabs. Same chord format as
    /// `keybindings`. Empty disables the leader.
    #[serde(default = "crate::defaults::leader_key")]
    pub leader_key: String,

    /// How long the armed leader waits for its next key before cancelling.
    #[serde(default = "crate::defaults::leader_timeout_ms")]
    pub leader_timeout_ms: u64,

    /// How long after the leader the which-key overlay listing the table
    /// appears. A fast follow-up key never shows it.
    #[serde(default = "crate::defaults::leader_overlay_delay_ms")]
    pub leader_overlay_delay_ms: u64,

    /// Vim-style `h j k l` (focus) and `H J K L` (swap) in the leader table
    /// (UX.md K9a). Moves last tab from `l` to `Tab`.
    #[serde(default = "crate::defaults::bool_false")]
    pub leader_vim_keys: bool,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            left_option_key_mode: OptionKeyMode::default(),
            right_option_key_mode: OptionKeyMode::default(),
            modifier_remapping: ModifierRemapping::default(),
            use_physical_keys: crate::defaults::bool_false(),
            leader_key: crate::defaults::leader_key(),
            leader_timeout_ms: crate::defaults::leader_timeout_ms(),
            leader_overlay_delay_ms: crate::defaults::leader_overlay_delay_ms(),
            leader_vim_keys: crate::defaults::bool_false(),
        }
    }
}

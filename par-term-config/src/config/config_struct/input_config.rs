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
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            left_option_key_mode: OptionKeyMode::default(),
            right_option_key_mode: OptionKeyMode::default(),
            modifier_remapping: ModifierRemapping::default(),
            use_physical_keys: crate::defaults::bool_false(),
        }
    }
}

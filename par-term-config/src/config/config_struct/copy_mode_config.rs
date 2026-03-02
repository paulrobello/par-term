//! `CopyModeConfig` — vi-style copy mode settings.

use serde::{Deserialize, Serialize};

/// Settings for the vi-style keyboard-driven copy mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyModeConfig {
    /// Enable copy mode (vi-style keyboard-driven text selection and navigation).
    /// When enabled, users can enter copy mode via the `toggle_copy_mode` keybinding
    /// action to navigate the terminal buffer with vi keys and yank text.
    #[serde(default = "crate::defaults::bool_true")]
    pub copy_mode_enabled: bool,

    /// Automatically exit copy mode after yanking (copying) selected text.
    /// When true (default), pressing `y` in visual mode copies text and exits copy mode.
    /// When false, copy mode stays active after yanking so you can continue selecting.
    #[serde(default = "crate::defaults::bool_true")]
    pub copy_mode_auto_exit_on_yank: bool,

    /// Show a status bar at the bottom of the terminal when copy mode is active.
    /// The status bar displays the current mode (COPY/VISUAL/V-LINE/V-BLOCK/SEARCH)
    /// and cursor position information.
    #[serde(default = "crate::defaults::bool_true")]
    pub copy_mode_show_status: bool,
}

impl Default for CopyModeConfig {
    fn default() -> Self {
        Self {
            copy_mode_enabled: crate::defaults::bool_true(),
            copy_mode_auto_exit_on_yank: crate::defaults::bool_true(),
            copy_mode_show_status: crate::defaults::bool_true(),
        }
    }
}

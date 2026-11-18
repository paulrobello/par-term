//! Scrollback buffer settings for the terminal emulator.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file — existing
//! config files remain 100% compatible.

use serde::{Deserialize, Serialize};

/// Scrollback buffer configuration.
///
/// Controls the number of lines retained in the scrollback history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollbackConfig {
    /// Maximum number of lines to keep in scrollback buffer
    #[serde(default = "crate::defaults::scrollback", alias = "scrollback_size")]
    pub scrollback_lines: usize,

    /// Resize panes with the xterm.js-style reflow: the cursor tracks its row
    /// and scrollback lines return when the pane grows. Default (false) keeps
    /// par-term's legacy resize. Applies to terminals created after the
    /// setting (and live to running panes via config reload).
    #[serde(default)]
    pub xterm_resize_reflow: bool,
}

impl Default for ScrollbackConfig {
    fn default() -> Self {
        Self {
            scrollback_lines: crate::defaults::scrollback(),
            xterm_resize_reflow: false,
        }
    }
}

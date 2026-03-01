//! `SearchConfig` — search settings.

use serde::{Deserialize, Serialize};

/// Settings controlling terminal search behaviour and highlighting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Highlight color for search matches [R, G, B, A] (0-255)
    #[serde(default = "crate::defaults::search_highlight_color")]
    pub search_highlight_color: [u8; 4],

    /// Highlight color for the current/active search match [R, G, B, A] (0-255)
    #[serde(default = "crate::defaults::search_current_highlight_color")]
    pub search_current_highlight_color: [u8; 4],

    /// Default case sensitivity for search
    #[serde(default = "crate::defaults::bool_false")]
    pub search_case_sensitive: bool,

    /// Default regex mode for search
    #[serde(default = "crate::defaults::bool_false")]
    pub search_regex: bool,

    /// Wrap around when navigating search matches
    #[serde(default = "crate::defaults::bool_true")]
    pub search_wrap_around: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            search_highlight_color: crate::defaults::search_highlight_color(),
            search_current_highlight_color: crate::defaults::search_current_highlight_color(),
            search_case_sensitive: crate::defaults::bool_false(),
            search_regex: crate::defaults::bool_false(),
            search_wrap_around: crate::defaults::bool_true(),
        }
    }
}

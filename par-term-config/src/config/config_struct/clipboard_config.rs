//! Clipboard synchronisation limits and OSC 52 policy.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use serde::{Deserialize, Serialize};

/// Clipboard sync event retention and whether OSC 52 may set the system clipboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardConfig {
    /// Maximum clipboard sync events retained for diagnostics
    #[serde(
        default = "crate::defaults::clipboard_max_sync_events",
        alias = "max_clipboard_sync_events"
    )]
    pub clipboard_max_sync_events: usize,

    /// Maximum bytes stored per clipboard sync event
    #[serde(
        default = "crate::defaults::clipboard_max_event_bytes",
        alias = "max_clipboard_event_bytes"
    )]
    pub clipboard_max_event_bytes: usize,

    /// Whether OSC 52 clipboard-set sequences from programs are applied to the
    /// system clipboard. Enabled by default so remote programs (over SSH) can
    /// copy to the local clipboard via OSC 52.
    #[serde(default = "crate::defaults::osc52_clipboard")]
    pub osc52_clipboard: bool,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            clipboard_max_sync_events: crate::defaults::clipboard_max_sync_events(),
            clipboard_max_event_bytes: crate::defaults::clipboard_max_event_bytes(),
            osc52_clipboard: crate::defaults::osc52_clipboard(),
        }
    }
}

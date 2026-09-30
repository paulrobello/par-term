//! Transient overlay and UI request state for the window manager.
//!
//! Extracted from `WindowState` as part of the God Object decomposition (ARC-001).

use crate::app::tab_ops::ClosedTabInfo;
use std::collections::VecDeque;
use std::time::Instant;

/// State for transient UI overlays (resize, toast, pane ID) and pending UI requests.
pub(crate) struct OverlayState {
    /// Whether a request to open the settings window is pending
    pub(crate) open_settings_window_requested: bool,
    /// Name of arrangement to restore, if pending
    pub(crate) pending_arrangement_restore: Option<String>,
    /// Whether a request to reload dynamic profiles is pending
    pub(crate) reload_dynamic_profiles_requested: bool,

    /// Whether to open the settings window directly to the profiles tab
    pub(crate) open_settings_profiles_tab: bool,
    /// Whether the profiles menu needs to be rebuilt
    pub(crate) profiles_menu_needs_update: bool,

    /// Whether the resize dimensions overlay is currently visible
    pub(crate) resize_overlay_visible: bool,
    /// When to hide the resize overlay
    pub(crate) resize_overlay_hide_time: Option<Instant>,
    /// Dimensions to show in the resize overlay: (width_px, height_px, cols, rows)
    pub(crate) resize_dimensions: Option<(u32, u32, usize, usize)>,

    /// The toast stack (UX.md OV7) — what the renderer draws and what
    /// tests read (`WindowState::last_toast_text`).
    pub(crate) toasts: crate::app::overlay::toast::ToastQueue,
    /// The armed keyboard mode's status line (action prefix, resize mode),
    /// drawn in its own layer, never as a toast (UX.md OV8).
    pub(crate) mode_banner: Option<&'static str>,

    /// Active IME preedit (composing) text, drawn at the terminal cursor
    pub(crate) ime_preedit: Option<String>,

    /// When to hide the pane identification overlay
    pub(crate) pane_identify_hide_time: Option<Instant>,

    /// Recently closed tab metadata for session undo
    pub(crate) closed_tabs: VecDeque<ClosedTabInfo>,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            open_settings_window_requested: false,
            pending_arrangement_restore: None,
            reload_dynamic_profiles_requested: false,
            open_settings_profiles_tab: false,
            profiles_menu_needs_update: true,
            resize_overlay_visible: false,
            resize_overlay_hide_time: None,
            resize_dimensions: None,
            toasts: Default::default(),
            mode_banner: None,
            ime_preedit: None,
            pane_identify_hide_time: None,
            closed_tabs: VecDeque::new(),
        }
    }
}

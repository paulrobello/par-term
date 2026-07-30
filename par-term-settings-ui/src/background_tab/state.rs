//! Per-tab state for the Background tab.

use par_term_config::BackgroundImageMode;

/// Per-pane background image editor form state.
///
/// `Default` is written out rather than derived: `temp_pane_bg_opacity` starts
/// at 1.0, and deriving would silently reset it to 0.0.
#[derive(Debug)]
pub struct BackgroundTabState {
    /// Temporary per-pane background image path for editing
    pub temp_pane_bg_path: String,
    /// Temporary per-pane background mode
    pub temp_pane_bg_mode: BackgroundImageMode,
    /// Temporary per-pane background opacity
    pub temp_pane_bg_opacity: f32,
    /// Temporary per-pane background darken amount
    pub temp_pane_bg_darken: f32,
    /// Index of the pane currently being configured (None = no pane selected)
    pub temp_pane_bg_index: Option<usize>,
}

impl Default for BackgroundTabState {
    fn default() -> Self {
        Self {
            temp_pane_bg_path: String::new(),
            temp_pane_bg_mode: BackgroundImageMode::default(),
            temp_pane_bg_opacity: 1.0,
            temp_pane_bg_darken: 0.0,
            temp_pane_bg_index: None,
        }
    }
}

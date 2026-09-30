//! Render-loop pending-work state for the window manager.
//!
//! Groups the three boolean/struct fields that each signal "some work must be
//! done on the next (or a future) render-loop iteration":
//!
//! - `config_changed_by_agent` — an agent wrote a new config; propagate it.
//! - `pending_font_rebuild` — a font-related setting changed; rebuild the renderer.
//! - `config_save_state` — a config save was debounced; flush it when safe.
//!
//! Extracted from `WindowState` as part of the God Object decomposition (ARC-001).

/// Debounce state for config saves to prevent rapid concurrent writes.
#[derive(Default)]
pub(crate) struct ConfigSaveState {
    /// When the last config save was performed
    pub(crate) last_save: Option<std::time::Instant>,
    /// Whether a save was deferred and needs to be executed
    pub(crate) pending_save: bool,
}

impl ConfigSaveState {
    /// Minimum time between config saves (in milliseconds).
    pub(crate) const DEBOUNCE_INTERVAL_MS: u64 = 100;
}

/// A config change a window makes on its own while running (not through
/// Settings). The window manager persists it, so that while Settings is open
/// it lands in Settings' baseline instead of writing the live preview to disk
/// or being wiped by the next live update (UX.md SS1 follow-up).
#[derive(Debug, Clone)]
pub(crate) enum ExternalConfigChange {
    /// The OS switched light/dark and auto theme / tab style followed it.
    SystemTheme { is_dark: bool },
    /// The user finished resizing the Assistant panel.
    AssistantPanelWidth(f32),
    /// "Skip This Version" in the update dialog.
    SkippedVersion(Option<String>),
    /// The first-run shader or integrations prompt recorded an answer or an
    /// install (prompt state and installed/prompted versions).
    Integrations(Box<par_term_config::IntegrationConfig>),
}

impl PartialEq for ExternalConfigChange {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::SystemTheme { is_dark: a }, Self::SystemTheme { is_dark: b }) => a == b,
            (Self::AssistantPanelWidth(a), Self::AssistantPanelWidth(b)) => a == b,
            (Self::SkippedVersion(a), Self::SkippedVersion(b)) => a == b,
            // Never merged: each carries its window's full integration state.
            _ => false,
        }
    }
}

impl ExternalConfigChange {
    /// Apply this change to a config.
    pub(crate) fn apply(&self, config: &mut par_term_config::Config) {
        match self {
            Self::SystemTheme { is_dark } => {
                config.apply_system_theme(*is_dark);
                config.apply_system_tab_style(*is_dark);
            }
            Self::AssistantPanelWidth(width) => config.ai_inspector.ai_inspector_width = *width,
            Self::SkippedVersion(version) => config.updates.skipped_version = version.clone(),
            Self::Integrations(integrations) => config.integrations = (**integrations).clone(),
        }
    }
}

/// Pending-work flags for the render loop.
///
/// All three fields signal work that must be completed on a future frame.
/// Grouping them documents that they share the same lifecycle pattern:
/// set by some event handler, cleared once the frame-level operation runs.
#[derive(Default)]
pub(crate) struct RenderLoopState {
    /// Set when an agent/MCP config update was applied; triggers cross-window
    /// config propagation on the next `about_to_wait` tick.
    pub(crate) config_changed_by_agent: bool,
    /// Set when a font-related setting changed; triggers renderer rebuild on
    /// the next frame setup pass.
    pub(crate) pending_font_rebuild: bool,
    /// Last time tab titles were refreshed from terminal/shell integration state.
    ///
    /// Title updates do not need to run every render frame; throttling this avoids
    /// touching every tab's terminal lock on animated frames.
    pub(crate) last_tab_title_refresh: Option<std::time::Instant>,
    /// Debounce state for config saves to prevent rapid concurrent writes.
    pub(crate) config_save: ConfigSaveState,
    /// Config changes this window made on its own, awaiting persistence by
    /// the window manager.
    pub(crate) external_config_changes: Vec<ExternalConfigChange>,
    /// config.yaml was reloaded from disk this tick (SS10).
    pub(crate) config_reloaded_from_disk: bool,
}

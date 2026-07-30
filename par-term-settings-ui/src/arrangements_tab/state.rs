//! Per-tab state for the Arrangements tab.

use crate::ArrangementId;

/// Arrangement save/rename/confirm form state.
///
/// The arrangement list itself (`arrangement_manager`) and the outbound action
/// queue stay on [`crate::SettingsUI`]: the main window owns and writes them.
#[derive(Debug, Default)]
pub struct ArrangementsTabState {
    /// Name for saving a new arrangement
    pub arrangement_save_name: String,
    /// Arrangement ID pending restore confirmation
    pub arrangement_confirm_restore: Option<ArrangementId>,
    /// Arrangement ID pending delete confirmation
    pub arrangement_confirm_delete: Option<ArrangementId>,
    /// Name pending overwrite confirmation (when saving with duplicate name)
    pub arrangement_confirm_overwrite: Option<String>,
    /// Arrangement ID pending replace confirmation
    pub arrangement_confirm_replace: Option<ArrangementId>,
    /// Arrangement ID being renamed
    pub arrangement_rename_id: Option<ArrangementId>,
    /// Text buffer for rename operation
    pub arrangement_rename_text: String,
}

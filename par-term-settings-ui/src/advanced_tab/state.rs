//! Per-tab state for the Advanced tab.

/// Preferences import/export form state.
#[derive(Debug, Default)]
pub struct AdvancedTabState {
    /// Temporary URL for import-from-URL feature
    pub temp_import_url: String,
    /// Status message for import/export operations
    pub import_export_status: Option<String>,
    /// Whether the import/export status is an error (true) or success (false)
    pub import_export_is_error: bool,
}

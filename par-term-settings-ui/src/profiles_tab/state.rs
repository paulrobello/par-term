//! Per-tab state for the Profiles tab.

/// Dynamic profile source editor form state.
#[derive(Debug, Default)]
pub struct ProfilesTabState {
    /// Index of dynamic source currently being edited (None = not editing)
    pub dynamic_source_editing: Option<usize>,
    /// Temp copy of the source being edited
    pub dynamic_source_edit_buffer: Option<par_term_config::DynamicProfileSource>,
    /// Temp buffer for new header key being added
    pub dynamic_source_new_header_key: String,
    /// Temp buffer for new header value being added
    pub dynamic_source_new_header_value: String,
}

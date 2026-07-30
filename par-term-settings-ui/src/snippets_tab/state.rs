//! Per-tab state for the Snippets tab.

/// Inline snippet editor form state.
///
/// `Default` is written out rather than derived: several fields start at a
/// non-zero value, and deriving would silently reset them.
#[derive(Debug)]
pub struct SnippetsTabState {
    /// Index of snippet currently being edited (None = not editing)
    pub editing_snippet_index: Option<usize>,
    /// Temporary snippet ID for edit form
    pub temp_snippet_id: String,
    /// Temporary snippet title for edit form
    pub temp_snippet_title: String,
    /// Temporary snippet content for edit form
    pub temp_snippet_content: String,
    /// Temporary snippet keybinding for edit form
    pub temp_snippet_keybinding: String,
    /// Temporary snippet folder for edit form
    pub temp_snippet_folder: String,
    /// Temporary snippet description for edit form
    pub temp_snippet_description: String,
    /// Temporary snippet keybinding enabled for edit form
    pub temp_snippet_keybinding_enabled: bool,
    /// Temporary snippet auto_execute for edit form
    pub temp_snippet_auto_execute: bool,
    /// Temporary snippet custom variables for edit form (ordered pairs for stable UI)
    pub temp_snippet_variables: Vec<(String, String)>,
    /// Whether the add-new-snippet form is active
    pub adding_new_snippet: bool,
    /// Whether currently recording a keybinding for a snippet
    pub recording_snippet_keybinding: bool,
    /// Recorded keybinding combo for snippet (displayed during recording)
    pub snippet_recorded_combo: Option<String>,
}

impl Default for SnippetsTabState {
    fn default() -> Self {
        Self {
            editing_snippet_index: None,
            temp_snippet_id: String::new(),
            temp_snippet_title: String::new(),
            temp_snippet_content: String::new(),
            temp_snippet_keybinding: String::new(),
            temp_snippet_folder: String::new(),
            temp_snippet_description: String::new(),
            temp_snippet_keybinding_enabled: true,
            temp_snippet_auto_execute: false,
            temp_snippet_variables: Vec::new(),
            adding_new_snippet: false,
            recording_snippet_keybinding: false,
            snippet_recorded_combo: None,
        }
    }
}

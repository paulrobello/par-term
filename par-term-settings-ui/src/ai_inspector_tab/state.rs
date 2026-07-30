//! Per-tab state for the AI Inspector (Assistant) tab.

/// Assistant prompt library editor state.
#[derive(Debug)]
pub struct AiInspectorTabState {
    /// Assistant prompts loaded from the Markdown-backed prompt library
    pub assistant_prompts: Vec<par_term_config::AssistantPrompt>,
    /// Error from loading or mutating the assistant prompt library
    pub assistant_prompt_error: Option<String>,
    /// Index of the assistant prompt currently being edited
    pub editing_assistant_prompt_index: Option<usize>,
    /// Whether the add-new assistant prompt form is active
    pub adding_new_assistant_prompt: bool,
    /// Temporary assistant prompt title for the inline editor
    pub temp_assistant_prompt_title: String,
    /// Temporary assistant prompt body for the inline editor
    pub temp_assistant_prompt_body: String,
    /// Temporary assistant prompt auto-submit flag for the inline editor
    pub temp_assistant_prompt_auto_submit: bool,
    /// Whether prompt-library files changed and Assistant panels should reload their menu list
    pub assistant_prompts_changed: bool,
}

impl AiInspectorTabState {
    /// Build the tab state around a caller-supplied prompt library.
    ///
    /// The prompt list and its load error come from the caller rather than a
    /// `Default` impl because [`crate::SettingsUI::new_for_tests`] must be able
    /// to construct this without touching the filesystem.
    pub fn new(
        assistant_prompts: Vec<par_term_config::AssistantPrompt>,
        assistant_prompt_error: Option<String>,
    ) -> Self {
        Self {
            assistant_prompts,
            assistant_prompt_error,
            editing_assistant_prompt_index: None,
            adding_new_assistant_prompt: false,
            temp_assistant_prompt_title: String::new(),
            temp_assistant_prompt_body: String::new(),
            temp_assistant_prompt_auto_submit: false,
            assistant_prompts_changed: false,
        }
    }
}

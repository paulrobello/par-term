//! Double-click word and smart-selection settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use crate::types::SmartSelectionRule;
use serde::{Deserialize, Serialize};

/// Word boundary characters and pattern-based smart selection rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordSelectionConfig {
    /// Characters considered part of a word for double-click selection (in addition to alphanumeric)
    /// Default: "/-+\\~_." (matches iTerm2)
    /// Example: If you want to select entire paths, add "/" to include path separators
    #[serde(default = "crate::defaults::word_characters")]
    pub word_characters: String,

    /// Enable smart selection rules for pattern-based double-click selection
    /// When enabled, double-click will try to match patterns like URLs, emails, paths
    /// before falling back to word boundary selection
    #[serde(default = "crate::defaults::smart_selection_enabled")]
    pub smart_selection_enabled: bool,

    /// Smart selection rules for pattern-based double-click selection
    /// Rules are evaluated by precision (highest first). If a pattern matches
    /// at the cursor position, that text is selected instead of using word boundaries.
    #[serde(default = "crate::types::default_smart_selection_rules")]
    pub smart_selection_rules: Vec<SmartSelectionRule>,
}

impl Default for WordSelectionConfig {
    fn default() -> Self {
        Self {
            word_characters: crate::defaults::word_characters(),
            smart_selection_enabled: crate::defaults::smart_selection_enabled(),
            smart_selection_rules: crate::types::default_smart_selection_rules(),
        }
    }
}

//! `UnicodeConfig` — Unicode width and normalization settings.

use serde::{Deserialize, Serialize};

/// Settings controlling Unicode character width and normalization behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnicodeConfig {
    /// Unicode version for character width calculations.
    /// Different versions have different width tables, particularly for emoji.
    /// Options: unicode_9, unicode_10, ..., unicode_16, auto (default)
    #[serde(default = "crate::defaults::unicode_version")]
    pub unicode_version: par_term_emu_core_rust::UnicodeVersion,

    /// Treatment of East Asian Ambiguous width characters
    /// - narrow: 1 cell width (Western default)
    /// - wide: 2 cell width (CJK default)
    #[serde(default = "crate::defaults::ambiguous_width")]
    pub ambiguous_width: par_term_emu_core_rust::AmbiguousWidth,

    /// Unicode normalization form for text processing.
    /// Controls how Unicode text is normalized before being stored in terminal cells.
    /// - NFC: Canonical composition (default, most compatible)
    /// - NFD: Canonical decomposition (macOS HFS+ style)
    /// - NFKC: Compatibility composition (resolves ligatures like ﬁ → fi)
    /// - NFKD: Compatibility decomposition
    /// - none: No normalization
    #[serde(default = "crate::defaults::normalization_form")]
    pub normalization_form: par_term_emu_core_rust::NormalizationForm,
}

impl Default for UnicodeConfig {
    fn default() -> Self {
        Self {
            unicode_version: crate::defaults::unicode_version(),
            ambiguous_width: crate::defaults::ambiguous_width(),
            normalization_form: crate::defaults::normalization_form(),
        }
    }
}

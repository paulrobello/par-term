//! Native Unicode configuration types for par-term-config.
//!
//! These types mirror the enums from `par-term-emu-core-rust` but belong to the
//! config layer, decoupling it from the emulation core. Callers that need to pass
//! these to emu-core APIs use the `.to_core()` conversion methods.
//!
//! See AUDIT.md ARC-003 for rationale.

use serde::{Deserialize, Serialize};

/// Unicode version for width calculation tables.
///
/// Different Unicode versions have different character width assignments,
/// particularly for newly added emoji and other characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnicodeVersion {
    /// Unicode 9.0 (June 2016) - Pre-emoji standardization
    Unicode9,
    /// Unicode 10.0 (June 2017)
    Unicode10,
    /// Unicode 11.0 (June 2018)
    Unicode11,
    /// Unicode 12.0 (March 2019)
    Unicode12,
    /// Unicode 13.0 (March 2020)
    Unicode13,
    /// Unicode 14.0 (September 2021)
    Unicode14,
    /// Unicode 15.0 (September 2022)
    Unicode15,
    /// Unicode 15.1 (September 2023)
    Unicode15_1,
    /// Unicode 16.0 (September 2024)
    Unicode16,
    /// Use the latest available Unicode version (default)
    #[default]
    Auto,
}

impl UnicodeVersion {
    /// Returns a human-readable version string for display in UI.
    pub fn version_string(&self) -> &'static str {
        match self {
            UnicodeVersion::Unicode9 => "9.0",
            UnicodeVersion::Unicode10 => "10.0",
            UnicodeVersion::Unicode11 => "11.0",
            UnicodeVersion::Unicode12 => "12.0",
            UnicodeVersion::Unicode13 => "13.0",
            UnicodeVersion::Unicode14 => "14.0",
            UnicodeVersion::Unicode15 => "15.0",
            UnicodeVersion::Unicode15_1 => "15.1",
            UnicodeVersion::Unicode16 => "16.0",
            UnicodeVersion::Auto => "auto",
        }
    }

    /// Convert to the emu-core equivalent type for passing to terminal APIs.
    pub fn to_core(self) -> par_term_emu_core_rust::UnicodeVersion {
        match self {
            Self::Unicode9 => par_term_emu_core_rust::UnicodeVersion::Unicode9,
            Self::Unicode10 => par_term_emu_core_rust::UnicodeVersion::Unicode10,
            Self::Unicode11 => par_term_emu_core_rust::UnicodeVersion::Unicode11,
            Self::Unicode12 => par_term_emu_core_rust::UnicodeVersion::Unicode12,
            Self::Unicode13 => par_term_emu_core_rust::UnicodeVersion::Unicode13,
            Self::Unicode14 => par_term_emu_core_rust::UnicodeVersion::Unicode14,
            Self::Unicode15 => par_term_emu_core_rust::UnicodeVersion::Unicode15,
            Self::Unicode15_1 => par_term_emu_core_rust::UnicodeVersion::Unicode15_1,
            Self::Unicode16 => par_term_emu_core_rust::UnicodeVersion::Unicode16,
            Self::Auto => par_term_emu_core_rust::UnicodeVersion::Auto,
        }
    }
}

/// Treatment of East Asian Ambiguous width characters.
///
/// Ambiguous characters include Greek/Cyrillic letters, some symbols, and
/// other characters that may display as either 1 or 2 cells depending on context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmbiguousWidth {
    /// Narrow (1 cell) - Western/default terminal behavior
    #[default]
    Narrow,
    /// Wide (2 cells) - CJK terminal behavior
    Wide,
}

impl AmbiguousWidth {
    /// Convert to the emu-core equivalent type for passing to terminal APIs.
    pub fn to_core(self) -> par_term_emu_core_rust::AmbiguousWidth {
        match self {
            Self::Narrow => par_term_emu_core_rust::AmbiguousWidth::Narrow,
            Self::Wide => par_term_emu_core_rust::AmbiguousWidth::Wide,
        }
    }
}

/// Unicode normalization form for terminal text.
///
/// Controls how Unicode text is normalized before being stored in terminal cells.
/// Normalization ensures consistent representation for search and comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum NormalizationForm {
    /// No normalization - store text as received
    #[serde(rename = "none")]
    None,
    /// Canonical Decomposition, followed by Canonical Composition (default)
    ///
    /// Combines characters where possible (`e` + combining accent -> `e` with accent).
    /// This is the most common form, used by most systems.
    #[default]
    NFC,
    /// Canonical Decomposition
    ///
    /// Splits into base + combining marks. Used by macOS HFS+ filesystem.
    NFD,
    /// Compatibility Decomposition, followed by Canonical Composition
    ///
    /// NFC + replaces compatibility characters (ligature fi -> f + i).
    NFKC,
    /// Compatibility Decomposition
    ///
    /// NFD + replaces compatibility characters.
    NFKD,
}

impl NormalizationForm {
    /// Convert to the emu-core equivalent type for passing to terminal APIs.
    pub fn to_core(self) -> par_term_emu_core_rust::NormalizationForm {
        match self {
            Self::None => par_term_emu_core_rust::NormalizationForm::None,
            Self::NFC => par_term_emu_core_rust::NormalizationForm::NFC,
            Self::NFD => par_term_emu_core_rust::NormalizationForm::NFD,
            Self::NFKC => par_term_emu_core_rust::NormalizationForm::NFKC,
            Self::NFKD => par_term_emu_core_rust::NormalizationForm::NFKD,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_version_serde_roundtrip() {
        let versions = [
            UnicodeVersion::Unicode9,
            UnicodeVersion::Unicode10,
            UnicodeVersion::Unicode11,
            UnicodeVersion::Unicode12,
            UnicodeVersion::Unicode13,
            UnicodeVersion::Unicode14,
            UnicodeVersion::Unicode15,
            UnicodeVersion::Unicode15_1,
            UnicodeVersion::Unicode16,
            UnicodeVersion::Auto,
        ];
        for v in &versions {
            let yaml = serde_yaml_ng::to_string(v).unwrap();
            let back: UnicodeVersion = serde_yaml_ng::from_str(&yaml).unwrap();
            assert_eq!(*v, back, "roundtrip failed for {:?}", v);
        }
    }

    #[test]
    fn ambiguous_width_serde_roundtrip() {
        for w in &[AmbiguousWidth::Narrow, AmbiguousWidth::Wide] {
            let yaml = serde_yaml_ng::to_string(w).unwrap();
            let back: AmbiguousWidth = serde_yaml_ng::from_str(&yaml).unwrap();
            assert_eq!(*w, back);
        }
    }

    #[test]
    fn normalization_form_serde_roundtrip() {
        let forms = [
            NormalizationForm::None,
            NormalizationForm::NFC,
            NormalizationForm::NFD,
            NormalizationForm::NFKC,
            NormalizationForm::NFKD,
        ];
        for f in &forms {
            let yaml = serde_yaml_ng::to_string(f).unwrap();
            let back: NormalizationForm = serde_yaml_ng::from_str(&yaml).unwrap();
            assert_eq!(*f, back, "roundtrip failed for {:?}", f);
        }
    }

    #[test]
    fn unicode_version_default_is_auto() {
        assert_eq!(UnicodeVersion::default(), UnicodeVersion::Auto);
    }

    #[test]
    fn ambiguous_width_default_is_narrow() {
        assert_eq!(AmbiguousWidth::default(), AmbiguousWidth::Narrow);
    }

    #[test]
    fn normalization_form_default_is_nfc() {
        assert_eq!(NormalizationForm::default(), NormalizationForm::NFC);
    }
}

//! Paste transformation utilities.
//!
//! Provides text transformations for the "Paste Special" feature, allowing users
//! to transform clipboard content before pasting (shell escaping, case conversion,
//! encoding, whitespace normalization, etc.).
//!
//! # Sub-modules
//!
//! - [`case`] — case conversion (title, camel, pascal, snake, screaming snake, kebab)
//! - [`encoding`] — Base64, URL, Hex, and JSON escape/unescape
//! - [`sanitize`] — clipboard content sanitization (strip dangerous control chars)
//! - [`shell`] — shell quoting and backslash escaping
//! - [`whitespace`] — whitespace and newline normalization

mod case;
mod encoding;
mod sanitize;
mod shell;
mod whitespace;

#[cfg(test)]
mod tests;

use std::fmt;

// Re-export the public API
pub use sanitize::sanitize_paste_content;

use case::{camel_case, kebab_case, pascal_case, screaming_snake_case, snake_case, title_case};
use encoding::{
    base64_decode, base64_encode, hex_decode, hex_encode, json_escape, json_unescape, url_decode,
    url_encode,
};
use shell::{shell_backslash_escape, shell_double_quote, shell_single_quote};
use whitespace::{
    add_newlines, collapse_spaces, normalize_line_endings, paste_as_single_line, remove_empty_lines,
    remove_newlines, trim_lines,
};

/// Available paste transformations.
///
/// Each variant represents a text transformation that can be applied to clipboard
/// content before pasting. Organized into categories for UI display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PasteTransform {
    // Shell category
    ShellSingleQuotes,
    ShellDoubleQuotes,
    ShellBackslash,

    // Case category
    CaseUppercase,
    CaseLowercase,
    CaseTitleCase,
    CaseCamelCase,
    CasePascalCase,
    CaseSnakeCase,
    CaseScreamingSnake,
    CaseKebabCase,

    // Newline category
    NewlineSingleLine,
    NewlineAddNewlines,
    NewlineRemoveNewlines,

    // Whitespace category
    WhitespaceTrim,
    WhitespaceTrimLines,
    WhitespaceCollapseSpaces,
    WhitespaceTabsToSpaces,
    WhitespaceSpacesToTabs,
    WhitespaceRemoveEmptyLines,
    WhitespaceNormalizeLineEndings,

    // Encode category
    EncodeBase64,
    DecodeBase64,
    EncodeUrl,
    DecodeUrl,
    EncodeHex,
    DecodeHex,
    EncodeJsonEscape,
    DecodeJsonUnescape,
}

impl PasteTransform {
    /// Display name for the UI (with category prefix for searchability).
    pub fn display_name(&self) -> &'static str {
        match self {
            // Shell
            Self::ShellSingleQuotes => "Shell: Single Quotes",
            Self::ShellDoubleQuotes => "Shell: Double Quotes",
            Self::ShellBackslash => "Shell: Backslash Escape",

            // Case
            Self::CaseUppercase => "Case: UPPERCASE",
            Self::CaseLowercase => "Case: lowercase",
            Self::CaseTitleCase => "Case: Title Case",
            Self::CaseCamelCase => "Case: camelCase",
            Self::CasePascalCase => "Case: PascalCase",
            Self::CaseSnakeCase => "Case: snake_case",
            Self::CaseScreamingSnake => "Case: SCREAMING_SNAKE",
            Self::CaseKebabCase => "Case: kebab-case",

            // Newline
            Self::NewlineSingleLine => "Newline: Paste as Single Line",
            Self::NewlineAddNewlines => "Newline: Add Newlines",
            Self::NewlineRemoveNewlines => "Newline: Remove Newlines",

            // Whitespace
            Self::WhitespaceTrim => "Whitespace: Trim",
            Self::WhitespaceTrimLines => "Whitespace: Trim Lines",
            Self::WhitespaceCollapseSpaces => "Whitespace: Collapse Spaces",
            Self::WhitespaceTabsToSpaces => "Whitespace: Tabs to Spaces",
            Self::WhitespaceSpacesToTabs => "Whitespace: Spaces to Tabs",
            Self::WhitespaceRemoveEmptyLines => "Whitespace: Remove Empty Lines",
            Self::WhitespaceNormalizeLineEndings => "Whitespace: Normalize Line Endings",

            // Encode
            Self::EncodeBase64 => "Encode: Base64",
            Self::DecodeBase64 => "Decode: Base64",
            Self::EncodeUrl => "Encode: URL",
            Self::DecodeUrl => "Decode: URL",
            Self::EncodeHex => "Encode: Hex",
            Self::DecodeHex => "Decode: Hex",
            Self::EncodeJsonEscape => "Encode: JSON Escape",
            Self::DecodeJsonUnescape => "Decode: JSON Unescape",
        }
    }

    /// Short description of what the transform does.
    pub fn description(&self) -> &'static str {
        match self {
            Self::ShellSingleQuotes => "Wrap in single quotes, escape internal quotes",
            Self::ShellDoubleQuotes => "Wrap in double quotes, escape special chars",
            Self::ShellBackslash => "Escape special characters with backslash",

            Self::CaseUppercase => "Convert all characters to uppercase",
            Self::CaseLowercase => "Convert all characters to lowercase",
            Self::CaseTitleCase => "Capitalize first letter of each word",
            Self::CaseCamelCase => "Convert to camelCase (firstWordLower)",
            Self::CasePascalCase => "Convert to PascalCase (AllWordsCapitalized)",
            Self::CaseSnakeCase => "Convert to snake_case (lowercase_with_underscores)",
            Self::CaseScreamingSnake => "Convert to SCREAMING_SNAKE_CASE",
            Self::CaseKebabCase => "Convert to kebab-case (lowercase-with-hyphens)",

            Self::NewlineSingleLine => "Strip all newlines, join into a single line",
            Self::NewlineAddNewlines => "Ensure text ends with a newline after each line",
            Self::NewlineRemoveNewlines => "Remove all newline characters",

            Self::WhitespaceTrim => "Remove leading and trailing whitespace",
            Self::WhitespaceTrimLines => "Trim whitespace from each line",
            Self::WhitespaceCollapseSpaces => "Replace multiple spaces with single space",
            Self::WhitespaceTabsToSpaces => "Convert tabs to 4 spaces",
            Self::WhitespaceSpacesToTabs => "Convert 4 spaces to tabs",
            Self::WhitespaceRemoveEmptyLines => "Remove blank lines",
            Self::WhitespaceNormalizeLineEndings => "Convert line endings to LF (\\n)",

            Self::EncodeBase64 => "Encode text as Base64",
            Self::DecodeBase64 => "Decode Base64 to text",
            Self::EncodeUrl => "URL/percent-encode special characters",
            Self::DecodeUrl => "Decode URL/percent-encoded text",
            Self::EncodeHex => "Encode text as hexadecimal",
            Self::DecodeHex => "Decode hexadecimal to text",
            Self::EncodeJsonEscape => "Escape text for JSON string",
            Self::DecodeJsonUnescape => "Unescape JSON string escapes",
        }
    }

    /// All available transformations in display order.
    pub fn all() -> &'static [PasteTransform] {
        &[
            // Shell
            Self::ShellSingleQuotes,
            Self::ShellDoubleQuotes,
            Self::ShellBackslash,
            // Case
            Self::CaseUppercase,
            Self::CaseLowercase,
            Self::CaseTitleCase,
            Self::CaseCamelCase,
            Self::CasePascalCase,
            Self::CaseSnakeCase,
            Self::CaseScreamingSnake,
            Self::CaseKebabCase,
            // Newline
            Self::NewlineSingleLine,
            Self::NewlineAddNewlines,
            Self::NewlineRemoveNewlines,
            // Whitespace
            Self::WhitespaceTrim,
            Self::WhitespaceTrimLines,
            Self::WhitespaceCollapseSpaces,
            Self::WhitespaceTabsToSpaces,
            Self::WhitespaceSpacesToTabs,
            Self::WhitespaceRemoveEmptyLines,
            Self::WhitespaceNormalizeLineEndings,
            // Encode
            Self::EncodeBase64,
            Self::DecodeBase64,
            Self::EncodeUrl,
            Self::DecodeUrl,
            Self::EncodeHex,
            Self::DecodeHex,
            Self::EncodeJsonEscape,
            Self::DecodeJsonUnescape,
        ]
    }

    /// Check if the display name matches a fuzzy search query.
    pub fn matches_query(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let name = self.display_name().to_lowercase();
        let query = query.to_lowercase();
        // Simple substring matching - supports "b64", "shell", "upper", etc.
        name.contains(&query)
    }
}

impl fmt::Display for PasteTransform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Apply a transformation to the input text.
///
/// Returns `Ok(transformed_text)` on success, or `Err(error_message)` if the
/// transformation fails (e.g., invalid Base64 input for decode).
pub fn transform(input: &str, transform: PasteTransform) -> Result<String, String> {
    match transform {
        // Shell transformations
        PasteTransform::ShellSingleQuotes => Ok(shell_single_quote(input)),
        PasteTransform::ShellDoubleQuotes => Ok(shell_double_quote(input)),
        PasteTransform::ShellBackslash => Ok(shell_backslash_escape(input)),

        // Case transformations
        PasteTransform::CaseUppercase => Ok(input.to_uppercase()),
        PasteTransform::CaseLowercase => Ok(input.to_lowercase()),
        PasteTransform::CaseTitleCase => Ok(title_case(input)),
        PasteTransform::CaseCamelCase => Ok(camel_case(input)),
        PasteTransform::CasePascalCase => Ok(pascal_case(input)),
        PasteTransform::CaseSnakeCase => Ok(snake_case(input)),
        PasteTransform::CaseScreamingSnake => Ok(screaming_snake_case(input)),
        PasteTransform::CaseKebabCase => Ok(kebab_case(input)),

        // Newline transformations
        PasteTransform::NewlineSingleLine => Ok(paste_as_single_line(input)),
        PasteTransform::NewlineAddNewlines => Ok(add_newlines(input)),
        PasteTransform::NewlineRemoveNewlines => Ok(remove_newlines(input)),

        // Whitespace transformations
        PasteTransform::WhitespaceTrim => Ok(input.trim().to_string()),
        PasteTransform::WhitespaceTrimLines => Ok(trim_lines(input)),
        PasteTransform::WhitespaceCollapseSpaces => Ok(collapse_spaces(input)),
        PasteTransform::WhitespaceTabsToSpaces => Ok(input.replace('\t', "    ")),
        PasteTransform::WhitespaceSpacesToTabs => Ok(input.replace("    ", "\t")),
        PasteTransform::WhitespaceRemoveEmptyLines => Ok(remove_empty_lines(input)),
        PasteTransform::WhitespaceNormalizeLineEndings => Ok(normalize_line_endings(input)),

        // Encoding transformations
        PasteTransform::EncodeBase64 => Ok(base64_encode(input)),
        PasteTransform::DecodeBase64 => base64_decode(input),
        PasteTransform::EncodeUrl => Ok(url_encode(input)),
        PasteTransform::DecodeUrl => url_decode(input),
        PasteTransform::EncodeHex => Ok(hex_encode(input)),
        PasteTransform::DecodeHex => hex_decode(input),
        PasteTransform::EncodeJsonEscape => Ok(json_escape(input)),
        PasteTransform::DecodeJsonUnescape => json_unescape(input),
    }
}

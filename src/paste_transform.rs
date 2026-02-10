//! Paste transformation utilities.
//!
//! Provides text transformations for the "Paste Special" feature, allowing users
//! to transform clipboard content before pasting (shell escaping, case conversion,
//! encoding, whitespace normalization, etc.).

use std::fmt;

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

// ============================================================================
// Shell transformations
// ============================================================================

/// Characters that require quoting/escaping in shell contexts.
const SHELL_SPECIAL_CHARS: &[char] = &[
    ' ', '\t', '\n', '\r', // Whitespace
    '\'', '"', '`', // Quotes and backticks
    '$', '!', '&', '|', // Variable expansion and control operators
    ';', '(', ')', '{', '}', '[', ']', // Grouping and subshell
    '<', '>', // Redirection
    '*', '?', // Glob patterns
    '\\', '#', '~', '^', // Escape, comments, home, history
];

fn shell_single_quote(input: &str) -> String {
    // Single quotes: escape internal ' as '\''
    let escaped = input.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

fn shell_double_quote(input: &str) -> String {
    // Double quotes: escape $, `, \, ", !
    let mut result = String::with_capacity(input.len() + 10);
    result.push('"');
    for c in input.chars() {
        match c {
            '$' | '`' | '\\' | '"' | '!' => {
                result.push('\\');
                result.push(c);
            }
            _ => result.push(c),
        }
    }
    result.push('"');
    result
}

fn shell_backslash_escape(input: &str) -> String {
    let mut result = String::with_capacity(input.len() * 2);
    for c in input.chars() {
        if SHELL_SPECIAL_CHARS.contains(&c) {
            result.push('\\');
        }
        result.push(c);
    }
    result
}

// ============================================================================
// Case transformations
// ============================================================================

fn title_case(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut capitalize_next = true;

    for c in input.chars() {
        if c.is_whitespace() || c == '-' || c == '_' {
            result.push(c);
            capitalize_next = true;
        } else if capitalize_next {
            for upper in c.to_uppercase() {
                result.push(upper);
            }
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Split input into words (by whitespace, hyphens, underscores, or camelCase boundaries).
fn split_into_words(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current_word = String::new();
    let mut prev_was_lowercase = false;

    for c in input.chars() {
        if c.is_whitespace() || c == '-' || c == '_' {
            if !current_word.is_empty() {
                words.push(current_word);
                current_word = String::new();
            }
            prev_was_lowercase = false;
        } else if c.is_uppercase() && prev_was_lowercase {
            // camelCase boundary
            if !current_word.is_empty() {
                words.push(current_word);
                current_word = String::new();
            }
            current_word.push(c);
            prev_was_lowercase = false;
        } else {
            current_word.push(c);
            prev_was_lowercase = c.is_lowercase();
        }
    }

    if !current_word.is_empty() {
        words.push(current_word);
    }

    words
}

fn camel_case(input: &str) -> String {
    let words = split_into_words(input);
    let mut result = String::new();

    for (i, word) in words.iter().enumerate() {
        if i == 0 {
            result.push_str(&word.to_lowercase());
        } else {
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                for upper in first.to_uppercase() {
                    result.push(upper);
                }
                for c in chars {
                    result.push(c.to_ascii_lowercase());
                }
            }
        }
    }
    result
}

fn pascal_case(input: &str) -> String {
    let words = split_into_words(input);
    let mut result = String::new();

    for word in &words {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            for upper in first.to_uppercase() {
                result.push(upper);
            }
            for c in chars {
                result.push(c.to_ascii_lowercase());
            }
        }
    }
    result
}

fn snake_case(input: &str) -> String {
    let words = split_into_words(input);
    words
        .iter()
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join("_")
}

fn screaming_snake_case(input: &str) -> String {
    let words = split_into_words(input);
    words
        .iter()
        .map(|w| w.to_uppercase())
        .collect::<Vec<_>>()
        .join("_")
}

fn kebab_case(input: &str) -> String {
    let words = split_into_words(input);
    words
        .iter()
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join("-")
}

// ============================================================================
// Newline transformations
// ============================================================================

/// Strip all newlines and join into a single line, replacing newlines with spaces.
fn paste_as_single_line(input: &str) -> String {
    input.lines().collect::<Vec<_>>().join(" ")
}

/// Ensure each line ends with a newline character.
fn add_newlines(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }
    let mut result: String = input.lines().collect::<Vec<_>>().join("\n");
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Remove all newline characters from the text.
fn remove_newlines(input: &str) -> String {
    input.replace(['\n', '\r'], "")
}

// ============================================================================
// Whitespace transformations
// ============================================================================

fn trim_lines(input: &str) -> String {
    input
        .lines()
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join("\n")
}

fn collapse_spaces(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut prev_was_space = false;

    for c in input.chars() {
        if c == ' ' {
            if !prev_was_space {
                result.push(c);
                prev_was_space = true;
            }
        } else {
            result.push(c);
            prev_was_space = false;
        }
    }
    result
}

fn remove_empty_lines(input: &str) -> String {
    input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_line_endings(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}

// ============================================================================
// Encoding transformations
// ============================================================================

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut result = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).map(|&b| b as u32).unwrap_or(0);
        let b2 = chunk.get(2).map(|&b| b as u32).unwrap_or(0);

        let n = (b0 << 16) | (b1 << 8) | b2;

        result.push(BASE64_CHARS[(n >> 18) as usize & 0x3F] as char);
        result.push(BASE64_CHARS[(n >> 12) as usize & 0x3F] as char);

        if chunk.len() > 1 {
            result.push(BASE64_CHARS[(n >> 6) as usize & 0x3F] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(BASE64_CHARS[n as usize & 0x3F] as char);
        } else {
            result.push('=');
        }
    }

    result
}

fn base64_decode(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(String::new());
    }

    // Build reverse lookup table
    let mut decode_table = [255u8; 256];
    for (i, &c) in BASE64_CHARS.iter().enumerate() {
        decode_table[c as usize] = i as u8;
    }

    let mut bytes = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits_collected = 0;

    for c in input.chars() {
        if c == '=' {
            break;
        }
        if c.is_whitespace() {
            continue;
        }

        let value = decode_table[c as usize];
        if value == 255 {
            return Err(format!("Invalid Base64 character: '{}'", c));
        }

        buffer = (buffer << 6) | (value as u32);
        bits_collected += 6;

        if bits_collected >= 8 {
            bits_collected -= 8;
            bytes.push((buffer >> bits_collected) as u8);
            buffer &= (1 << bits_collected) - 1;
        }
    }

    String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8 in decoded data: {}", e))
}

fn url_encode(input: &str) -> String {
    let mut result = String::with_capacity(input.len() * 3);

    for c in input.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                result.push(c);
            }
            _ => {
                for byte in c.to_string().as_bytes() {
                    result.push('%');
                    result.push_str(&format!("{:02X}", byte));
                }
            }
        }
    }
    result
}

fn url_decode(input: &str) -> Result<String, String> {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() != 2 {
                return Err("Incomplete percent-encoding".to_string());
            }
            match u8::from_str_radix(&hex, 16) {
                Ok(byte) => bytes.push(byte),
                Err(_) => return Err(format!("Invalid hex in URL encoding: %{}", hex)),
            }
        } else if c == '+' {
            bytes.push(b' ');
        } else {
            for byte in c.to_string().as_bytes() {
                bytes.push(*byte);
            }
        }
    }

    String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8 in decoded data: {}", e))
}

fn hex_encode(input: &str) -> String {
    input
        .as_bytes()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

fn hex_decode(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(String::new());
    }

    // Remove common hex prefixes
    let input = input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"))
        .unwrap_or(input);

    // Filter out whitespace and collect hex chars
    let hex_chars: String = input.chars().filter(|c| !c.is_whitespace()).collect();

    if !hex_chars.len().is_multiple_of(2) {
        return Err("Hex string must have even length".to_string());
    }

    let bytes: Result<Vec<u8>, _> = (0..hex_chars.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex_chars[i..i + 2], 16)
                .map_err(|_| format!("Invalid hex: {}", &hex_chars[i..i + 2]))
        })
        .collect();

    let bytes = bytes?;
    String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8 in decoded data: {}", e))
}

fn json_escape(input: &str) -> String {
    let mut result = String::with_capacity(input.len() + 10);

    for c in input.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\x08' => result.push_str("\\b"), // backspace
            '\x0C' => result.push_str("\\f"), // form feed
            c if c.is_control() => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            _ => result.push(c),
        }
    }
    result
}

fn json_unescape(input: &str) -> Result<String, String> {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => result.push('"'),
                Some('\\') => result.push('\\'),
                Some('/') => result.push('/'),
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('b') => result.push('\x08'),
                Some('f') => result.push('\x0C'),
                Some('u') => {
                    let hex: String = chars.by_ref().take(4).collect();
                    if hex.len() != 4 {
                        return Err("Incomplete \\u escape sequence".to_string());
                    }
                    match u32::from_str_radix(&hex, 16) {
                        Ok(code) => match char::from_u32(code) {
                            Some(ch) => result.push(ch),
                            None => return Err(format!("Invalid Unicode code point: \\u{}", hex)),
                        },
                        Err(_) => return Err(format!("Invalid hex in \\u escape: {}", hex)),
                    }
                }
                Some(other) => {
                    // Unknown escape, keep as-is
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    Ok(result)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Shell transformations
    #[test]
    fn test_shell_single_quotes() {
        assert_eq!(
            transform("hello world", PasteTransform::ShellSingleQuotes).unwrap(),
            "'hello world'"
        );
        assert_eq!(
            transform("it's a test", PasteTransform::ShellSingleQuotes).unwrap(),
            "'it'\\''s a test'"
        );
    }

    #[test]
    fn test_shell_double_quotes() {
        assert_eq!(
            transform("hello world", PasteTransform::ShellDoubleQuotes).unwrap(),
            "\"hello world\""
        );
        assert_eq!(
            transform("$HOME/file", PasteTransform::ShellDoubleQuotes).unwrap(),
            "\"\\$HOME/file\""
        );
    }

    #[test]
    fn test_shell_backslash() {
        assert_eq!(
            transform("hello world", PasteTransform::ShellBackslash).unwrap(),
            "hello\\ world"
        );
        assert_eq!(
            transform("$var", PasteTransform::ShellBackslash).unwrap(),
            "\\$var"
        );
    }

    // Case transformations
    #[test]
    fn test_case_uppercase() {
        assert_eq!(
            transform("Hello World", PasteTransform::CaseUppercase).unwrap(),
            "HELLO WORLD"
        );
    }

    #[test]
    fn test_case_lowercase() {
        assert_eq!(
            transform("Hello World", PasteTransform::CaseLowercase).unwrap(),
            "hello world"
        );
    }

    #[test]
    fn test_case_title_case() {
        assert_eq!(
            transform("hello world", PasteTransform::CaseTitleCase).unwrap(),
            "Hello World"
        );
        assert_eq!(
            transform("hello-world", PasteTransform::CaseTitleCase).unwrap(),
            "Hello-World"
        );
    }

    #[test]
    fn test_case_camel_case() {
        assert_eq!(
            transform("hello world", PasteTransform::CaseCamelCase).unwrap(),
            "helloWorld"
        );
        assert_eq!(
            transform("Hello World", PasteTransform::CaseCamelCase).unwrap(),
            "helloWorld"
        );
        assert_eq!(
            transform("hello_world", PasteTransform::CaseCamelCase).unwrap(),
            "helloWorld"
        );
    }

    #[test]
    fn test_case_pascal_case() {
        assert_eq!(
            transform("hello world", PasteTransform::CasePascalCase).unwrap(),
            "HelloWorld"
        );
    }

    #[test]
    fn test_case_snake_case() {
        assert_eq!(
            transform("Hello World", PasteTransform::CaseSnakeCase).unwrap(),
            "hello_world"
        );
        assert_eq!(
            transform("helloWorld", PasteTransform::CaseSnakeCase).unwrap(),
            "hello_world"
        );
    }

    #[test]
    fn test_case_screaming_snake() {
        assert_eq!(
            transform("Hello World", PasteTransform::CaseScreamingSnake).unwrap(),
            "HELLO_WORLD"
        );
    }

    #[test]
    fn test_case_kebab_case() {
        assert_eq!(
            transform("Hello World", PasteTransform::CaseKebabCase).unwrap(),
            "hello-world"
        );
    }

    // Newline transformations
    #[test]
    fn test_newline_single_line() {
        assert_eq!(
            transform("line1\nline2\nline3", PasteTransform::NewlineSingleLine).unwrap(),
            "line1 line2 line3"
        );
        assert_eq!(
            transform("single line", PasteTransform::NewlineSingleLine).unwrap(),
            "single line"
        );
    }

    #[test]
    fn test_newline_add_newlines() {
        assert_eq!(
            transform("line1\nline2", PasteTransform::NewlineAddNewlines).unwrap(),
            "line1\nline2\n"
        );
        // Already has trailing newline
        assert_eq!(
            transform("line1\nline2\n", PasteTransform::NewlineAddNewlines).unwrap(),
            "line1\nline2\n"
        );
    }

    #[test]
    fn test_newline_remove_newlines() {
        assert_eq!(
            transform("line1\nline2\nline3", PasteTransform::NewlineRemoveNewlines).unwrap(),
            "line1line2line3"
        );
        assert_eq!(
            transform("line1\r\nline2", PasteTransform::NewlineRemoveNewlines).unwrap(),
            "line1line2"
        );
    }

    // Whitespace transformations
    #[test]
    fn test_whitespace_trim() {
        assert_eq!(
            transform("  hello  ", PasteTransform::WhitespaceTrim).unwrap(),
            "hello"
        );
    }

    #[test]
    fn test_whitespace_trim_lines() {
        assert_eq!(
            transform("  line1  \n  line2  ", PasteTransform::WhitespaceTrimLines).unwrap(),
            "line1\nline2"
        );
    }

    #[test]
    fn test_whitespace_collapse_spaces() {
        assert_eq!(
            transform("hello    world", PasteTransform::WhitespaceCollapseSpaces).unwrap(),
            "hello world"
        );
    }

    #[test]
    fn test_whitespace_tabs_to_spaces() {
        assert_eq!(
            transform("hello\tworld", PasteTransform::WhitespaceTabsToSpaces).unwrap(),
            "hello    world"
        );
    }

    #[test]
    fn test_whitespace_spaces_to_tabs() {
        assert_eq!(
            transform("hello    world", PasteTransform::WhitespaceSpacesToTabs).unwrap(),
            "hello\tworld"
        );
    }

    #[test]
    fn test_whitespace_remove_empty_lines() {
        assert_eq!(
            transform(
                "line1\n\nline2\n  \nline3",
                PasteTransform::WhitespaceRemoveEmptyLines
            )
            .unwrap(),
            "line1\nline2\nline3"
        );
    }

    #[test]
    fn test_whitespace_normalize_line_endings() {
        assert_eq!(
            transform(
                "line1\r\nline2\rline3",
                PasteTransform::WhitespaceNormalizeLineEndings
            )
            .unwrap(),
            "line1\nline2\nline3"
        );
    }

    // Encoding transformations
    #[test]
    fn test_encode_base64() {
        assert_eq!(
            transform("hello", PasteTransform::EncodeBase64).unwrap(),
            "aGVsbG8="
        );
        assert_eq!(
            transform("Hello World!", PasteTransform::EncodeBase64).unwrap(),
            "SGVsbG8gV29ybGQh"
        );
    }

    #[test]
    fn test_decode_base64() {
        assert_eq!(
            transform("aGVsbG8=", PasteTransform::DecodeBase64).unwrap(),
            "hello"
        );
        assert_eq!(
            transform("SGVsbG8gV29ybGQh", PasteTransform::DecodeBase64).unwrap(),
            "Hello World!"
        );
    }

    #[test]
    fn test_base64_roundtrip() {
        let original = "The quick brown fox jumps over the lazy dog!";
        let encoded = transform(original, PasteTransform::EncodeBase64).unwrap();
        let decoded = transform(&encoded, PasteTransform::DecodeBase64).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_encode_url() {
        assert_eq!(
            transform("hello world", PasteTransform::EncodeUrl).unwrap(),
            "hello%20world"
        );
        assert_eq!(
            transform("a=b&c=d", PasteTransform::EncodeUrl).unwrap(),
            "a%3Db%26c%3Dd"
        );
    }

    #[test]
    fn test_decode_url() {
        assert_eq!(
            transform("hello%20world", PasteTransform::DecodeUrl).unwrap(),
            "hello world"
        );
        assert_eq!(
            transform("hello+world", PasteTransform::DecodeUrl).unwrap(),
            "hello world"
        );
    }

    #[test]
    fn test_url_roundtrip() {
        let original = "hello world! & goodbye=yes";
        let encoded = transform(original, PasteTransform::EncodeUrl).unwrap();
        let decoded = transform(&encoded, PasteTransform::DecodeUrl).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_encode_hex() {
        assert_eq!(
            transform("hello", PasteTransform::EncodeHex).unwrap(),
            "68656c6c6f"
        );
    }

    #[test]
    fn test_decode_hex() {
        assert_eq!(
            transform("68656c6c6f", PasteTransform::DecodeHex).unwrap(),
            "hello"
        );
        assert_eq!(
            transform("0x68656c6c6f", PasteTransform::DecodeHex).unwrap(),
            "hello"
        );
    }

    #[test]
    fn test_hex_roundtrip() {
        let original = "Hello World!";
        let encoded = transform(original, PasteTransform::EncodeHex).unwrap();
        let decoded = transform(&encoded, PasteTransform::DecodeHex).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_encode_json_escape() {
        assert_eq!(
            transform("hello\nworld", PasteTransform::EncodeJsonEscape).unwrap(),
            "hello\\nworld"
        );
        assert_eq!(
            transform("say \"hi\"", PasteTransform::EncodeJsonEscape).unwrap(),
            "say \\\"hi\\\""
        );
    }

    #[test]
    fn test_decode_json_unescape() {
        assert_eq!(
            transform("hello\\nworld", PasteTransform::DecodeJsonUnescape).unwrap(),
            "hello\nworld"
        );
        assert_eq!(
            transform("say \\\"hi\\\"", PasteTransform::DecodeJsonUnescape).unwrap(),
            "say \"hi\""
        );
    }

    #[test]
    fn test_json_roundtrip() {
        let original = "Line1\nLine2\tTabbed \"quoted\"";
        let encoded = transform(original, PasteTransform::EncodeJsonEscape).unwrap();
        let decoded = transform(&encoded, PasteTransform::DecodeJsonUnescape).unwrap();
        assert_eq!(decoded, original);
    }

    // Edge cases
    #[test]
    fn test_empty_string() {
        for transform_type in PasteTransform::all() {
            let result = transform("", *transform_type);
            assert!(
                result.is_ok(),
                "Transform {:?} failed on empty string",
                transform_type
            );
        }
    }

    #[test]
    fn test_unicode() {
        // Uppercase preserves emojis
        assert_eq!(
            transform("Hello! ", PasteTransform::CaseUppercase).unwrap(),
            "HELLO! "
        );
        // Base64 encoding of emoji (rocket is F0 9F 9A 81 in UTF-8)
        let encoded = transform("", PasteTransform::EncodeBase64).unwrap();
        let decoded = transform(&encoded, PasteTransform::DecodeBase64).unwrap();
        assert_eq!(decoded, "");
    }

    #[test]
    fn test_fuzzy_match() {
        // Substring matching on display name
        assert!(PasteTransform::EncodeBase64.matches_query("base"));
        assert!(PasteTransform::EncodeBase64.matches_query("Base64"));
        assert!(PasteTransform::ShellSingleQuotes.matches_query("shell"));
        assert!(PasteTransform::ShellSingleQuotes.matches_query("single"));
        assert!(PasteTransform::CaseUppercase.matches_query("upper"));
        assert!(PasteTransform::CaseUppercase.matches_query("CASE"));
        assert!(PasteTransform::CaseUppercase.matches_query("")); // empty matches all
        assert!(!PasteTransform::CaseUppercase.matches_query("xyz"));
    }

    // Error cases
    #[test]
    fn test_invalid_base64() {
        let result = transform("not valid base64!!!", PasteTransform::DecodeBase64);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_hex() {
        let result = transform("xyz", PasteTransform::DecodeHex);
        assert!(result.is_err());

        let result = transform("abc", PasteTransform::DecodeHex); // odd length
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_url_encoding() {
        let result = transform("%ZZ", PasteTransform::DecodeUrl);
        assert!(result.is_err());
    }
}

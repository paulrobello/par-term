//! Smart selection module for pattern-based text selection.
//!
//! This module provides intelligent double-click selection based on regex patterns.
//! When the user double-clicks, the system first tries smart selection rules (sorted
//! by precision, highest first). If a pattern matches at the cursor position, that
//! text is selected. Otherwise, it falls back to word boundary selection.

use crate::config::SmartSelectionRule;
use regex::Regex;

/// Compiled smart selection rules with cached regex patterns
pub struct SmartSelectionMatcher {
    /// Compiled rules sorted by precision (highest first)
    rules: Vec<CompiledRule>,
}

struct CompiledRule {
    #[allow(dead_code)]
    name: String,
    regex: Regex,
    precision: f64,
}

impl SmartSelectionMatcher {
    /// Create a new matcher from a list of smart selection rules
    pub fn new(rules: &[SmartSelectionRule]) -> Self {
        let mut compiled: Vec<CompiledRule> = rules
            .iter()
            .filter(|r| r.enabled)
            .filter_map(|r| match Regex::new(&r.regex) {
                Ok(regex) => Some(CompiledRule {
                    name: r.name.clone(),
                    regex,
                    precision: r.precision.value(),
                }),
                Err(e) => {
                    log::warn!(
                        "Failed to compile smart selection regex '{}': {}",
                        r.name,
                        e
                    );
                    None
                }
            })
            .collect();

        // Sort by precision descending (highest first)
        compiled.sort_by(|a, b| {
            b.precision
                .partial_cmp(&a.precision)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Self { rules: compiled }
    }

    /// Try to find a pattern match at the given character position in the line.
    ///
    /// Returns the start and end column indices (inclusive) if a match is found,
    /// or None if no pattern matches at this position.
    ///
    /// # Arguments
    /// * `line` - The full text of the line
    /// * `col` - The column position (character index) where the cursor is
    pub fn find_match_at(&self, line: &str, col: usize) -> Option<(usize, usize)> {
        // Convert col to byte offset for regex matching
        let byte_offset = char_to_byte_offset(line, col)?;

        for rule in &self.rules {
            // Find all matches in the line
            for mat in rule.regex.find_iter(line) {
                let match_start_byte = mat.start();
                let match_end_byte = mat.end();

                // Check if the cursor position is within this match
                if byte_offset >= match_start_byte && byte_offset < match_end_byte {
                    // Convert byte offsets back to character offsets
                    let start_col = byte_to_char_offset(line, match_start_byte)?;
                    let end_col = byte_to_char_offset(line, match_end_byte)?.saturating_sub(1);

                    return Some((start_col, end_col));
                }
            }
        }

        None
    }
}

/// Convert a character offset to a byte offset in a UTF-8 string
fn char_to_byte_offset(s: &str, char_offset: usize) -> Option<usize> {
    s.char_indices()
        .nth(char_offset)
        .map(|(byte_idx, _)| byte_idx)
        .or_else(|| {
            // If char_offset is at or past the end, return the string length
            if char_offset >= s.chars().count() {
                Some(s.len())
            } else {
                None
            }
        })
}

/// Convert a byte offset to a character offset in a UTF-8 string
fn byte_to_char_offset(s: &str, byte_offset: usize) -> Option<usize> {
    if byte_offset > s.len() {
        return None;
    }
    Some(s[..byte_offset].chars().count())
}

/// Check if a character should be considered part of a word.
///
/// A character is part of a word if:
/// - It is alphanumeric (a-z, A-Z, 0-9)
/// - It is in the user-defined word_characters set
///
/// Note: Unlike some terminals, underscore is NOT hardcoded as a word character.
/// It is included in the default word_characters setting (`/-+\~_.`) but can be
/// removed by the user for full control over word selection behavior.
pub fn is_word_char(ch: char, word_characters: &str) -> bool {
    ch.is_alphanumeric() || word_characters.contains(ch)
}

/// Find word boundaries at the given position using configurable word characters.
///
/// Returns (start_col, end_col) as inclusive indices.
pub fn find_word_boundaries(line: &str, col: usize, word_characters: &str) -> (usize, usize) {
    let chars: Vec<char> = line.chars().collect();

    if chars.is_empty() || col >= chars.len() {
        return (col, col);
    }

    let mut start_col = col;
    let mut end_col = col;

    // Expand left
    while start_col > 0 && is_word_char(chars[start_col - 1], word_characters) {
        start_col -= 1;
    }

    // Make sure the clicked position is a word character, otherwise return single char
    if !is_word_char(chars[col], word_characters) {
        return (col, col);
    }

    // Expand right
    while end_col < chars.len() - 1 && is_word_char(chars[end_col + 1], word_characters) {
        end_col += 1;
    }

    (start_col, end_col)
}

/// Cache for compiled smart selection matchers to avoid recompilation
pub struct SmartSelectionCache {
    /// Cached matcher (recreated when rules change)
    matcher: Option<SmartSelectionMatcher>,
    /// Hash of the rules used to create the cached matcher
    rules_hash: u64,
}

impl Default for SmartSelectionCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SmartSelectionCache {
    pub fn new() -> Self {
        Self {
            matcher: None,
            rules_hash: 0,
        }
    }

    /// Get or create a matcher for the given rules
    pub fn get_matcher(&mut self, rules: &[SmartSelectionRule]) -> &SmartSelectionMatcher {
        let hash = hash_rules(rules);

        if self.rules_hash != hash || self.matcher.is_none() {
            self.matcher = Some(SmartSelectionMatcher::new(rules));
            self.rules_hash = hash;
        }

        self.matcher.as_ref().unwrap()
    }
}

/// Simple hash for rules to detect changes
fn hash_rules(rules: &[SmartSelectionRule]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    for rule in rules {
        rule.name.hash(&mut hasher);
        rule.regex.hash(&mut hasher);
        rule.enabled.hash(&mut hasher);
        // Use precision ordinal for hashing
        std::mem::discriminant(&rule.precision).hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{SmartSelectionPrecision, SmartSelectionRule};

    fn test_rules() -> Vec<SmartSelectionRule> {
        vec![
            SmartSelectionRule::new(
                "HTTP URL",
                r"https?://[^\s]+",
                SmartSelectionPrecision::VeryHigh,
            ),
            SmartSelectionRule::new(
                "Email",
                r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b",
                SmartSelectionPrecision::High,
            ),
            SmartSelectionRule::new(
                "File path",
                r"~?/?(?:[a-zA-Z0-9._-]+/)+[a-zA-Z0-9._-]+/?",
                SmartSelectionPrecision::Normal,
            ),
        ]
    }

    #[test]
    fn test_find_url_match() {
        let matcher = SmartSelectionMatcher::new(&test_rules());
        let line = "Check out https://example.com/path for more info";

        // Click on 'h' in https
        let result = matcher.find_match_at(line, 10);
        assert_eq!(result, Some((10, 33)));

        // Click on 'e' in example
        let result = matcher.find_match_at(line, 18);
        assert_eq!(result, Some((10, 33)));

        // Click on 'C' in Check (not in URL)
        let result = matcher.find_match_at(line, 0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_email_match() {
        let matcher = SmartSelectionMatcher::new(&test_rules());
        let line = "Contact user@example.com for help";

        // Click on 'u' in user
        let result = matcher.find_match_at(line, 8);
        assert_eq!(result, Some((8, 23)));

        // Click on '@'
        let result = matcher.find_match_at(line, 12);
        assert_eq!(result, Some((8, 23)));
    }

    #[test]
    fn test_find_path_match() {
        let matcher = SmartSelectionMatcher::new(&test_rules());
        let line = "Edit ~/Documents/file.txt and save";

        // Click on 'D' in Documents
        let result = matcher.find_match_at(line, 7);
        assert_eq!(result, Some((5, 24)));
    }

    #[test]
    fn test_word_boundaries_default() {
        let line = "hello_world test-case foo.bar";
        let word_chars = "/-+\\~_.";

        // Click on 'w' in world
        let (start, end) = find_word_boundaries(line, 6, word_chars);
        assert_eq!(
            &line.chars().collect::<Vec<_>>()[start..=end]
                .iter()
                .collect::<String>(),
            "hello_world"
        );

        // Click on 't' in test
        let (start, end) = find_word_boundaries(line, 12, word_chars);
        assert_eq!(
            &line.chars().collect::<Vec<_>>()[start..=end]
                .iter()
                .collect::<String>(),
            "test-case"
        );
    }

    #[test]
    fn test_word_boundaries_empty_config() {
        let line = "hello_world test-case";
        let word_chars = "";

        // With empty word_chars, only alphanumeric characters are word chars
        // underscore is NOT hardcoded - it must be in word_characters to be included
        // Click on 'w' in world - should stop at underscore
        let (start, end) = find_word_boundaries(line, 6, word_chars);
        assert_eq!(
            &line.chars().collect::<Vec<_>>()[start..=end]
                .iter()
                .collect::<String>(),
            "world"
        );

        // Click on 'h' in hello - should stop at underscore
        let (start, end) = find_word_boundaries(line, 0, word_chars);
        assert_eq!(
            &line.chars().collect::<Vec<_>>()[start..=end]
                .iter()
                .collect::<String>(),
            "hello"
        );

        // Click on 't' in test - should stop at hyphen
        let (start, end) = find_word_boundaries(line, 12, word_chars);
        assert_eq!(
            &line.chars().collect::<Vec<_>>()[start..=end]
                .iter()
                .collect::<String>(),
            "test"
        );
    }

    #[test]
    fn test_is_word_char() {
        let word_chars = "/-+\\~_.";

        assert!(is_word_char('a', word_chars));
        assert!(is_word_char('Z', word_chars));
        assert!(is_word_char('5', word_chars));
        assert!(is_word_char('_', word_chars));
        assert!(is_word_char('-', word_chars));
        assert!(is_word_char('/', word_chars));
        assert!(is_word_char('.', word_chars));

        assert!(!is_word_char(' ', word_chars));
        assert!(!is_word_char('@', word_chars));
        assert!(!is_word_char('!', word_chars));
    }

    #[test]
    fn test_unicode_handling() {
        let matcher = SmartSelectionMatcher::new(&test_rules());
        let line = "日本語 https://example.com 中文";

        // The URL starts at character position 4 (after "日本語 ")
        // Click on URL after unicode - verify the URL starts at position 4
        let result = matcher.find_match_at(line, 4);
        // The URL "https://example.com" is 19 characters (4+19-1 = 22 for inclusive end)
        assert_eq!(result, Some((4, 22)));
    }

    #[test]
    fn test_disabled_rule() {
        let mut rules = test_rules();
        rules[0].enabled = false; // Disable URL rule

        let matcher = SmartSelectionMatcher::new(&rules);
        let line = "Check out https://example.com for more info";

        // URL rule is disabled, so no match
        let result = matcher.find_match_at(line, 10);
        assert_eq!(result, None);
    }

    #[test]
    fn test_precision_ordering() {
        // Create rules where a lower precision rule would match a broader pattern
        let rules = vec![
            SmartSelectionRule::new("Whitespace-bounded", r"\S+", SmartSelectionPrecision::Low),
            SmartSelectionRule::new(
                "Email",
                r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b",
                SmartSelectionPrecision::High,
            ),
        ];

        let matcher = SmartSelectionMatcher::new(&rules);
        let line = "Contact user@example.com for help";

        // Should match email (higher precision) not the whole word
        let result = matcher.find_match_at(line, 12);
        assert_eq!(result, Some((8, 23)));
    }
}

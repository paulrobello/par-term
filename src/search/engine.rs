//! Search engine for terminal scrollback.

use super::types::{SearchConfig, SearchMatch};
use regex::{Regex, RegexBuilder};

/// Search engine that performs text searches on terminal content.
pub struct SearchEngine {
    /// Cached compiled regex for the current query.
    cached_regex: Option<(String, bool, Regex)>, // (pattern, case_sensitive, compiled)
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchEngine {
    /// Create a new search engine.
    pub fn new() -> Self {
        Self { cached_regex: None }
    }

    /// Search through lines of text and return all matches.
    ///
    /// # Arguments
    /// * `lines` - Iterator of (line_index, line_text) pairs
    /// * `query` - The search query
    /// * `config` - Search configuration options
    ///
    /// # Returns
    /// A vector of SearchMatch containing all matches found.
    pub fn search<I>(&mut self, lines: I, query: &str, config: &SearchConfig) -> Vec<SearchMatch>
    where
        I: Iterator<Item = (usize, String)>,
    {
        if query.is_empty() {
            return Vec::new();
        }

        let mut matches = Vec::new();

        if config.use_regex {
            self.search_regex(lines, query, config, &mut matches);
        } else {
            self.search_plain(lines, query, config, &mut matches);
        }

        matches
    }

    /// Perform plain text search.
    fn search_plain<I>(
        &self,
        lines: I,
        query: &str,
        config: &SearchConfig,
        matches: &mut Vec<SearchMatch>,
    ) where
        I: Iterator<Item = (usize, String)>,
    {
        let query_lower = if config.case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };

        // Query length in characters (not bytes)
        let query_char_len = query.chars().count();

        for (line_idx, line) in lines {
            let search_line = if config.case_sensitive {
                line.clone()
            } else {
                line.to_lowercase()
            };

            let mut start_byte = 0;
            while let Some(pos) = search_line[start_byte..].find(&query_lower) {
                let byte_offset = start_byte + pos;

                // Convert byte offset to character offset for cell positioning
                let char_column = Self::byte_offset_to_char_offset(&search_line, byte_offset);

                // Check whole word matching if enabled (using byte offset for string slicing)
                if config.whole_word
                    && !Self::is_whole_word_static(&line, byte_offset, query_lower.len())
                {
                    start_byte = byte_offset + 1;
                    continue;
                }

                matches.push(SearchMatch::new(line_idx, char_column, query_char_len));
                start_byte = byte_offset + query_lower.len().max(1);

                // Avoid infinite loops on empty matches
                if query.is_empty() {
                    break;
                }
            }
        }
    }

    /// Perform regex search.
    fn search_regex<I>(
        &mut self,
        lines: I,
        query: &str,
        config: &SearchConfig,
        matches: &mut Vec<SearchMatch>,
    ) where
        I: Iterator<Item = (usize, String)>,
    {
        // Try to compile or reuse cached regex
        let regex = match self.get_or_compile_regex(query, config.case_sensitive) {
            Ok(re) => re.clone(), // Clone to avoid borrow issues
            Err(e) => {
                log::debug!("Invalid regex pattern '{}': {}", query, e);
                return;
            }
        };

        for (line_idx, line) in lines {
            for mat in regex.find_iter(&line) {
                let byte_start = mat.start();
                let byte_end = mat.end();

                // Convert byte offsets to character offsets for cell positioning
                let char_column = Self::byte_offset_to_char_offset(&line, byte_start);
                let char_length = Self::byte_offset_to_char_offset(&line, byte_end) - char_column;

                // Check whole word matching if enabled (using byte offsets for string slicing)
                if config.whole_word
                    && !Self::is_whole_word_static(&line, byte_start, byte_end - byte_start)
                {
                    continue;
                }

                matches.push(SearchMatch::new(line_idx, char_column, char_length));
            }
        }
    }

    /// Convert a byte offset to a character offset in a string.
    /// This is needed because String::find() returns byte offsets, but we need
    /// character offsets for cell positioning (each cell = 1 character).
    fn byte_offset_to_char_offset(s: &str, byte_offset: usize) -> usize {
        s[..byte_offset].chars().count()
    }

    /// Get cached regex or compile a new one.
    fn get_or_compile_regex(
        &mut self,
        pattern: &str,
        case_sensitive: bool,
    ) -> Result<&Regex, regex::Error> {
        // Check if we have a cached regex that matches
        let needs_recompile = match &self.cached_regex {
            Some((cached_pattern, cached_case, _)) => {
                cached_pattern != pattern || *cached_case != case_sensitive
            }
            None => true,
        };

        if needs_recompile {
            let regex = RegexBuilder::new(pattern)
                .case_insensitive(!case_sensitive)
                .build()?;
            self.cached_regex = Some((pattern.to_string(), case_sensitive, regex));
        }

        Ok(&self
            .cached_regex
            .as_ref()
            .expect("cached_regex was just set to Some above if it was None")
            .2)
    }

    /// Check if a match at the given position is a whole word.
    fn is_whole_word_static(line: &str, start: usize, length: usize) -> bool {
        let end = start + length;

        // Check character before the match
        if start > 0
            && let Some(c) = line[..start].chars().last()
            && (c.is_alphanumeric() || c == '_')
        {
            return false;
        }

        // Check character after the match
        if end < line.len()
            && let Some(c) = line[end..].chars().next()
            && (c.is_alphanumeric() || c == '_')
        {
            return false;
        }

        true
    }

    /// Clear the cached regex.
    pub fn clear_cache(&mut self) {
        self.cached_regex = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_lines<'a>(texts: &'a [&'a str]) -> impl Iterator<Item = (usize, String)> + 'a {
        texts.iter().enumerate().map(|(i, s)| (i, s.to_string()))
    }

    #[test]
    fn test_plain_search_case_insensitive() {
        let mut engine = SearchEngine::new();
        let lines: Vec<&str> = vec!["Hello World", "hello there", "HELLO WORLD"];
        let config = SearchConfig::default();

        let matches = engine.search(make_lines(&lines), "hello", &config);

        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0], SearchMatch::new(0, 0, 5));
        assert_eq!(matches[1], SearchMatch::new(1, 0, 5));
        assert_eq!(matches[2], SearchMatch::new(2, 0, 5));
    }

    #[test]
    fn test_plain_search_case_sensitive() {
        let mut engine = SearchEngine::new();
        let lines: Vec<&str> = vec!["Hello World", "hello there", "HELLO WORLD"];
        let config = SearchConfig {
            case_sensitive: true,
            ..Default::default()
        };

        let matches = engine.search(make_lines(&lines), "hello", &config);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], SearchMatch::new(1, 0, 5));
    }

    #[test]
    fn test_plain_search_multiple_matches_per_line() {
        let mut engine = SearchEngine::new();
        let lines: Vec<&str> = vec!["foo bar foo baz foo"];
        let config = SearchConfig::default();

        let matches = engine.search(make_lines(&lines), "foo", &config);

        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0], SearchMatch::new(0, 0, 3));
        assert_eq!(matches[1], SearchMatch::new(0, 8, 3));
        assert_eq!(matches[2], SearchMatch::new(0, 16, 3));
    }

    #[test]
    fn test_whole_word_matching() {
        let mut engine = SearchEngine::new();
        let lines: Vec<&str> = vec!["foobar foo barfoo"];
        let config = SearchConfig {
            whole_word: true,
            ..Default::default()
        };

        let matches = engine.search(make_lines(&lines), "foo", &config);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], SearchMatch::new(0, 7, 3));
    }

    #[test]
    fn test_regex_search() {
        let mut engine = SearchEngine::new();
        let lines: Vec<&str> = vec![
            "error: something failed",
            "warning: check this",
            "error: again",
        ];
        let config = SearchConfig {
            use_regex: true,
            ..Default::default()
        };

        let matches = engine.search(make_lines(&lines), "error:", &config);

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0], SearchMatch::new(0, 0, 6));
        assert_eq!(matches[1], SearchMatch::new(2, 0, 6));
    }

    #[test]
    fn test_regex_pattern() {
        let mut engine = SearchEngine::new();
        let lines: Vec<&str> = vec!["test123", "test456", "notest"];
        let config = SearchConfig {
            use_regex: true,
            ..Default::default()
        };

        let matches = engine.search(make_lines(&lines), r"test\d+", &config);

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0], SearchMatch::new(0, 0, 7));
        assert_eq!(matches[1], SearchMatch::new(1, 0, 7));
    }

    #[test]
    fn test_empty_query() {
        let mut engine = SearchEngine::new();
        let lines: Vec<&str> = vec!["some text"];
        let config = SearchConfig::default();

        let matches = engine.search(make_lines(&lines), "", &config);

        assert!(matches.is_empty());
    }

    #[test]
    fn test_unicode_character_offsets() {
        let mut engine = SearchEngine::new();
        // Emoji folder icon (4 bytes in UTF-8) followed by space and text
        let lines: Vec<&str> = vec!["📁 Downloads", "normal text"];
        let config = SearchConfig::default();

        let matches = engine.search(make_lines(&lines), "down", &config);

        // "down" should be found at character position 2 (after "📁 ")
        // NOT byte position 5 (4 bytes for emoji + 1 for space)
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line, 0);
        assert_eq!(matches[0].column, 2); // Character offset, not byte offset
        assert_eq!(matches[0].length, 4);
    }

    #[test]
    fn test_unicode_multiple_emoji() {
        let mut engine = SearchEngine::new();
        // Multiple emoji before the search term
        let lines: Vec<&str> = vec!["🎉🎊🎁 party time"];
        let config = SearchConfig::default();

        let matches = engine.search(make_lines(&lines), "party", &config);

        // "party" starts at character 4 (3 emoji + 1 space)
        // NOT byte position 13 (3*4 bytes + 1 space)
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].column, 4);
        assert_eq!(matches[0].length, 5);
    }

    #[test]
    fn test_invalid_regex() {
        let mut engine = SearchEngine::new();
        let lines: Vec<&str> = vec!["some text"];
        let config = SearchConfig {
            use_regex: true,
            ..Default::default()
        };

        // Invalid regex should return empty results
        let matches = engine.search(make_lines(&lines), "[invalid", &config);

        assert!(matches.is_empty());
    }
}

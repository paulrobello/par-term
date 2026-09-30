//! Query matching for Settings search (UX.md SQ3).
//!
//! A query is split into tokens and every token must match somewhere in the
//! candidate text (token AND). Matching is case-insensitive, treats any
//! non-alphanumeric character as a word separator (so `prompt_on_quit` and
//! "prompt on quit" are the same query), drops filler words ("on", "as"),
//! and applies light stemming so "shortcuts" finds "shortcut" and
//! "blinking" finds "blink".

/// Filler words dropped from a query unless the query is nothing but them.
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "as", "at", "by", "for", "in", "is", "of", "on", "or", "the", "to", "when",
    "with",
];

/// A parsed query: the stemmed tokens every candidate must contain.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Query {
    tokens: Vec<String>,
}

impl Query {
    /// Parse a raw query string.
    pub fn parse(raw: &str) -> Self {
        let all: Vec<String> = words(raw).collect();
        let meaningful: Vec<&String> = all
            .iter()
            .filter(|w| !STOPWORDS.contains(&w.as_str()))
            .collect();
        let kept: Vec<&String> = if meaningful.is_empty() {
            all.iter().collect()
        } else {
            meaningful
        };
        Self {
            tokens: kept.into_iter().map(|w| stem(w)).collect(),
        }
    }

    /// True when the query has no tokens (empty or whitespace only).
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// True when every token matches `text`.
    pub fn matches(&self, text: &str) -> bool {
        self.matches_hays(&[&Haystack::new(text)])
    }

    /// True when every token matches at least one of `texts` (the tokens may
    /// be spread across them: a control label plus its section title).
    pub fn matches_any_of<'a>(&self, texts: impl IntoIterator<Item = &'a str>) -> bool {
        let hays: Vec<Haystack> = texts.into_iter().map(Haystack::new).collect();
        self.matches_hays(&hays.iter().collect::<Vec<_>>())
    }

    /// True when every token is found in at least one of `hays`.
    pub fn matches_hays(&self, hays: &[&Haystack]) -> bool {
        self.tokens
            .iter()
            .all(|t| hays.iter().any(|h| h.contains(t)))
    }

    /// True when at least one token is found in `hay`.
    pub fn any_token_in(&self, hay: &Haystack) -> bool {
        self.tokens.iter().any(|t| hay.contains(t))
    }
}

/// Pre-tokenized candidate text: the stems of every word, so a registry is
/// tokenized once rather than on every keystroke.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Haystack {
    stems: Vec<String>,
}

impl Haystack {
    /// Tokenize one text.
    pub fn new(text: &str) -> Self {
        Self {
            stems: words(text).map(|w| stem(&w)).collect(),
        }
    }

    /// Tokenize several texts into one haystack.
    pub fn of<'a>(texts: impl IntoIterator<Item = &'a str>) -> Self {
        Self {
            stems: texts
                .into_iter()
                .flat_map(|t| words(t).map(|w| stem(&w)).collect::<Vec<_>>())
                .collect(),
        }
    }

    /// A query token matches a word whose stem starts with it, so a partial
    /// word ("scrollb") still finds "scrollback" while typing.
    fn contains(&self, token: &str) -> bool {
        self.stems.iter().any(|s| s.starts_with(token))
    }
}

/// Lowercased words: runs of alphanumerics. Everything else separates.
fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
}

/// Light English stemming: strip one common inflection suffix. Words of
/// three letters or fewer are left alone, so short terms such as "osc" and
/// "fps" are never mangled.
fn stem(word: &str) -> String {
    const SUFFIXES: &[(&str, &str)] =
        &[("ies", "y"), ("ing", ""), ("ed", ""), ("es", ""), ("s", "")];
    if word.chars().count() <= 3 {
        return word.to_string();
    }
    let mut stemmed = word.to_string();
    for (suffix, replacement) in SUFFIXES {
        if let Some(root) = word.strip_suffix(suffix)
            && root.chars().count() >= 3
            && !(*suffix == "s" && root.ends_with('s'))
        {
            stemmed = format!("{root}{replacement}");
            break;
        }
    }
    // "enable" / "enabled" / "enables" all reduce to "enabl".
    if stemmed.chars().count() > 3 && stemmed.ends_with('e') {
        stemmed.pop();
    }
    stemmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_is_empty() {
        assert!(Query::parse("").is_empty());
        assert!(Query::parse("   ").is_empty());
        assert!(Query::parse(" - _ ").is_empty());
        assert!(!Query::parse("x").is_empty());
    }

    #[test]
    fn tokens_are_anded_across_the_text() {
        let q = Query::parse("cursor blink");
        assert!(q.matches("Cursor blink"));
        assert!(q.matches("Blink the cursor"));
        assert!(!q.matches("Cursor style"));
    }

    #[test]
    fn case_and_separators_are_ignored() {
        let q = Query::parse("prompt_on_quit");
        assert!(q.matches("Prompt on quit"));
        assert!(Query::parse("Copy-On-Select").matches("copy on select"));
    }

    #[test]
    fn stopwords_are_dropped_unless_they_are_the_whole_query() {
        assert!(Query::parse("option as meta").matches("Option key sends Meta"));
        assert!(Query::parse("copy on select").matches("Auto-copy selection"));
        let only_stopwords = Query::parse("on");
        assert!(!only_stopwords.is_empty());
        assert!(only_stopwords.matches("Always on top"));
    }

    #[test]
    fn light_stemming_equates_inflections() {
        assert!(Query::parse("shortcuts").matches("Keyboard shortcut"));
        assert!(Query::parse("shortcut").matches("Keyboard shortcuts"));
        assert!(Query::parse("blinking").matches("Cursor blink"));
        assert!(Query::parse("keybinding").matches("Keybindings"));
        assert!(Query::parse("entries").matches("Max entry count"));
        assert!(Query::parse("enabled").matches("Enable bell"));
        assert!(Query::parse("closes").matches("Close window"));
        assert!(Query::parse("quit").matches("Confirm before quitting"));
    }

    #[test]
    fn prefix_of_a_word_matches_while_typing() {
        assert!(Query::parse("scrollb").matches("Scrollback lines"));
        assert!(!Query::parse("crollback").matches("Scrollback lines"));
    }

    #[test]
    fn tokens_may_spread_across_several_texts() {
        let q = Query::parse("close confirmation");
        assert!(q.matches_any_of(["Close Confirmation", "Confirm before quitting"]));
        assert!(q.matches_any_of(["Close", "Confirmation"]));
        assert!(!q.matches_any_of(["Close", "Quit"]));
        assert!(q.any_token_in(&Haystack::new("Close")));
        assert!(!q.any_token_in(&Haystack::new("Quit")));
    }

    #[test]
    fn short_words_are_not_stemmed() {
        assert_eq!(stem("osc"), "osc");
        assert_eq!(stem("tabs"), "tab");
        assert_eq!(stem("class"), "class");
    }
}

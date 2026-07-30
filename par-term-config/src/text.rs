//! Char-boundary-safe text helpers shared across the par-term crates.
//!
//! Terminal code routinely holds a *column* index — a count of grid cells — and
//! needs to address the matching position in a `String`. Byte-slicing with such
//! an index panics on any line containing non-ASCII text, so every column →
//! string conversion goes through these helpers.

/// Byte offset of the `col`-th character of `s`.
///
/// Saturates to `s.len()` when `col` is past the end of `s`, so the result is
/// always a valid slice boundary and the function never panics.
pub fn column_to_byte_offset(s: &str, col: usize) -> usize {
    s.char_indices()
        .nth(col)
        .map_or(s.len(), |(byte_index, _)| byte_index)
}

/// Character index of the character that starts at `byte_offset` in `s`.
///
/// The inverse of [`column_to_byte_offset`]. A `byte_offset` that is not a char
/// boundary yields the index of the character containing it rather than
/// panicking.
pub fn byte_offset_to_column(s: &str, byte_offset: usize) -> usize {
    s.char_indices()
        .take_while(|(byte_index, _)| *byte_index < byte_offset)
        .count()
}

/// Truncate `s` to at most `max_chars` characters, respecting UTF-8 char
/// boundaries.
pub fn truncate_chars(s: &str, max_chars: usize) -> &str {
    &s[..column_to_byte_offset(s, max_chars)]
}

/// Lowercase `s` one character at a time, recording for each produced character
/// the index of the source character it came from.
///
/// `char::to_lowercase` can produce more than one character (`İ` becomes `i`
/// followed by U+0307), so a position in the lowercased text is not a position
/// in `s`; the returned map converts back. Char-wise lowering differs from
/// [`str::to_lowercase`] only for Greek final sigma, so callers that lower both
/// haystack and needle through this function stay self-consistent.
pub fn lowercase_with_source_map(s: &str) -> (String, Vec<usize>) {
    let mut lowered = String::with_capacity(s.len());
    let mut sources: Vec<usize> = Vec::with_capacity(s.len());

    for (source_index, c) in s.chars().enumerate() {
        for lowered_char in c.to_lowercase() {
            lowered.push(lowered_char);
            sources.push(source_index);
        }
    }

    (lowered, sources)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCENTED: &str = "café";
    const CJK: &str = "日本語";
    const EMOJI: &str = "😀😁😂";
    const MIXED: &str = "Hi 日本 café 😀";

    #[test]
    fn column_to_byte_offset_ascii() {
        assert_eq!(column_to_byte_offset("hello", 0), 0);
        assert_eq!(column_to_byte_offset("hello", 3), 3);
        assert_eq!(column_to_byte_offset("hello", 5), 5);
    }

    #[test]
    fn column_to_byte_offset_saturates_past_end() {
        assert_eq!(column_to_byte_offset(ACCENTED, 99), ACCENTED.len());
        assert_eq!(column_to_byte_offset(CJK, usize::MAX), CJK.len());
        assert_eq!(column_to_byte_offset("", 1), 0);
    }

    #[test]
    fn column_to_byte_offset_non_ascii() {
        // "café" = c(1) a(1) f(1) é(2)
        assert_eq!(column_to_byte_offset(ACCENTED, 3), 3);
        assert_eq!(column_to_byte_offset(ACCENTED, 4), 5);
        // Each CJK char is 3 bytes, each emoji 4.
        assert_eq!(column_to_byte_offset(CJK, 2), 6);
        assert_eq!(column_to_byte_offset(EMOJI, 2), 8);
        for col in 0..=MIXED.chars().count() {
            assert!(MIXED.is_char_boundary(column_to_byte_offset(MIXED, col)));
        }
    }

    #[test]
    fn byte_offset_to_column_round_trips() {
        for text in [ACCENTED, CJK, EMOJI, MIXED, "ascii only"] {
            for col in 0..=text.chars().count() {
                let byte_offset = column_to_byte_offset(text, col);
                assert_eq!(byte_offset_to_column(text, byte_offset), col, "{text}");
            }
        }
    }

    #[test]
    fn truncate_chars_counts_characters_not_bytes() {
        assert_eq!(truncate_chars("hello world", 5), "hello");
        assert_eq!(truncate_chars(ACCENTED, 4), ACCENTED);
        assert_eq!(truncate_chars(ACCENTED, 10), ACCENTED);
        assert_eq!(truncate_chars(ACCENTED, 3), "caf");
        assert_eq!(truncate_chars(CJK, 2), "日本");
        assert_eq!(truncate_chars(EMOJI, 1), "😀");
        assert_eq!(truncate_chars(MIXED, 5), "Hi 日本");
        assert_eq!(truncate_chars("", 5), "");
        assert_eq!(truncate_chars(EMOJI, 0), "");
    }

    #[test]
    fn truncate_chars_replaces_byte_slicing_that_would_panic() {
        // The display-truncation call sites all sliced `&s[..n]` directly.
        // `&ACCENTED[..4]` panics: byte 4 is inside `é`.
        assert!(!ACCENTED.is_char_boundary(4));
        assert_eq!(truncate_chars(ACCENTED, 4), ACCENTED);
        // The masked-header path caps at 8 characters; byte 8 splits a CJK
        // character.
        assert!(!CJK.is_char_boundary(8));
        assert_eq!(truncate_chars(CJK, 8), CJK);
        // An emoji is 4 bytes, so every odd byte cap lands mid-character.
        assert!(!EMOJI.is_char_boundary(2));
        assert_eq!(truncate_chars(EMOJI, 2), "😀😁");
    }

    #[test]
    fn lowercase_map_is_identity_for_ascii() {
        let (lowered, sources) = lowercase_with_source_map("AbC");
        assert_eq!(lowered, "abc");
        assert_eq!(sources, vec![0, 1, 2]);
    }

    #[test]
    fn lowercase_map_tracks_expanding_characters() {
        // U+0130 lowercases to two characters, so the lowered text is longer
        // than the source in characters as well as bytes.
        let (lowered, sources) = lowercase_with_source_map("aİb");
        assert_eq!(lowered, "ai\u{307}b");
        assert_eq!(lowered.chars().count(), 4);
        assert_eq!(sources, vec![0, 1, 1, 2]);
        // The final 'b' maps back to source character 2, not 3.
        assert_eq!(sources[3], 2);
    }

    #[test]
    fn lowercase_map_covers_non_ascii_fixtures() {
        for text in [ACCENTED, CJK, EMOJI, MIXED] {
            let (lowered, sources) = lowercase_with_source_map(text);
            assert_eq!(lowered.chars().count(), sources.len());
            assert!(sources.windows(2).all(|w| w[0] <= w[1]));
            assert_eq!(sources.last().copied(), Some(text.chars().count() - 1));
        }
    }
}

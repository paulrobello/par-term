//! Word-level diff highlighting for the diff renderer.
//!
//! Computes which tokens differ between two strings using a longest-common-
//! subsequence (LCS) approach, then produces [`StyledSegment`]s with changed
//! words rendered in a highlight background.

use crate::prettifier::types::StyledSegment;

// ---------------------------------------------------------------------------
// Word tokeniser
// ---------------------------------------------------------------------------

/// Split a string into words for word-level diff comparison.
///
/// Words are runs of alphanumeric characters / underscores; every other
/// character is its own single-character token.
pub(super) fn split_into_words(s: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut start = None;

    for (i, ch) in s.char_indices() {
        if ch.is_alphanumeric() || ch == '_' {
            if start.is_none() {
                start = Some(i);
            }
        } else {
            if let Some(s_idx) = start {
                words.push(&s[s_idx..i]);
                start = None;
            }
            // Each non-word character is its own token
            words.push(&s[i..i + ch.len_utf8()]);
        }
    }
    if let Some(s_idx) = start {
        words.push(&s[s_idx..]);
    }

    words
}

// ---------------------------------------------------------------------------
// LCS helpers
// ---------------------------------------------------------------------------

/// Compute the longest common subsequence length table for two slices.
pub(super) fn lcs_table<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<Vec<usize>> {
    let m = a.len();
    let n = b.len();
    let mut table = vec![vec![0usize; n + 1]; m + 1];

    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                table[i][j] = table[i - 1][j - 1] + 1;
            } else {
                table[i][j] = table[i - 1][j].max(table[i][j - 1]);
            }
        }
    }

    table
}

/// Maximum token count before skipping LCS (prevents O(n*m) blowup).
pub(super) const MAX_LCS_TOKENS: usize = 200;

/// Mark which tokens are changed (not in LCS) for word-level highlighting.
pub(super) fn mark_changes<'a>(tokens: &[&'a str], other: &[&'a str]) -> Vec<bool> {
    // Guard: if either side is too large, treat all tokens as changed.
    if tokens.len() > MAX_LCS_TOKENS || other.len() > MAX_LCS_TOKENS {
        return vec![true; tokens.len()];
    }
    let table = lcs_table(tokens, other);
    let mut changed = vec![true; tokens.len()];

    let mut i = tokens.len();
    let mut j = other.len();

    while i > 0 && j > 0 {
        if tokens[i - 1] == other[j - 1] {
            changed[i - 1] = false;
            i -= 1;
            j -= 1;
        } else if table[i - 1][j] >= table[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }

    changed
}

// ---------------------------------------------------------------------------
// Segment builder
// ---------------------------------------------------------------------------

/// Produce styled segments for a line with word-level diff highlighting.
///
/// Words that appear only in `line_text` (not in `other_text`) are rendered
/// with `highlight_bg` and bold; unchanged words use `base_fg` with no
/// background.
pub(super) fn word_diff_segments(
    line_text: &str,
    other_text: &str,
    base_fg: [u8; 3],
    highlight_bg: [u8; 3],
) -> Vec<StyledSegment> {
    let words_a = split_into_words(line_text);
    let words_b = split_into_words(other_text);
    let changes = mark_changes(&words_a, &words_b);

    let mut segments = Vec::new();
    let mut current_text = String::new();
    let mut current_changed = false;

    for (word, &is_changed) in words_a.iter().zip(changes.iter()) {
        if is_changed != current_changed && !current_text.is_empty() {
            segments.push(StyledSegment {
                text: std::mem::take(&mut current_text),
                fg: Some(base_fg),
                bg: if current_changed {
                    Some(highlight_bg)
                } else {
                    None
                },
                bold: current_changed,
                ..Default::default()
            });
        }
        current_changed = is_changed;
        current_text.push_str(word);
    }

    if !current_text.is_empty() {
        segments.push(StyledSegment {
            text: current_text,
            fg: Some(base_fg),
            bg: if current_changed {
                Some(highlight_bg)
            } else {
                None
            },
            bold: current_changed,
            ..Default::default()
        });
    }

    segments
}

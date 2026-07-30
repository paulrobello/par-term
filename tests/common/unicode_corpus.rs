//! Shared non-ASCII and wide-character test corpus.
//!
//! # Why this exists
//!
//! Six crashes shipped because a terminal *column* index was used as a UTF-8
//! *byte* offset, or as a `char`-vector index with no upper bound. All six were
//! invisible to 1,965 green tests for one reason: column, byte offset and char
//! index coincide **only** for pure-ASCII single-width rows, and every test
//! input in the suite was ASCII. This corpus makes the three quantities diverge
//! on purpose.
//!
//! # Three counts, deliberately
//!
//! Every fixture records `byte_len` and `char_count`, because confusing them is
//! the entire bug class, plus `grapheme_count`, because the *fourth* quantity —
//! what the grid actually renders in one cell — diverges from all of them for
//! ZWJ sequences and combining marks. `unicode_corpus_self_test` verifies all
//! three against the live values so the fixtures cannot rot.
//!
//! # What is deliberately absent: display columns
//!
//! There is no `display_cols` field. Width lives in the core VT crate and is
//! selectable at runtime through `unicode_version`, so any number hardcoded
//! here would be a second, unverified authority that silently disagrees with
//! the terminal. `grapheme_count` is checked against the repo's own
//! `TextShaper`, which is a real authority. The genuinely-wide fixtures are
//! listed in [`WIDE_FIXTURES`] for the cases that need them.
//!
//! # Usage
//!
//! Include with an explicit path so the corpus does not drag in the config
//! helpers in `common/mod.rs`:
//!
//! ```ignore
//! #[path = "common/unicode_corpus.rs"]
//! mod unicode_corpus;
//! ```
//!
//! Sibling crates (`par-term-config`, `par-term-input`) publish to crates.io
//! independently and carry their own minimal copies rather than reaching
//! outside their package directory, which `cargo package` would reject.

#![allow(dead_code)]

/// One corpus entry and the three counts that must not be confused.
#[derive(Clone, Copy, Debug)]
pub struct UnicodeFixture {
    /// Stable name, used in assertion messages.
    pub label: &'static str,
    pub text: &'static str,
    /// `text.len()` — UTF-8 bytes.
    pub byte_len: usize,
    /// `text.chars().count()` — Unicode scalar values. This is what copy-mode
    /// motions and `Grid::row_text`-derived column indices actually count.
    pub char_count: usize,
    /// User-perceived characters, per the repo's own `TextShaper`. Differs from
    /// `char_count` for ZWJ sequences, flags, skin-tone modifiers and combining
    /// marks — the cases where the char-index model and the rendered grid part
    /// company.
    pub grapheme_count: usize,
}

const fn fixture(
    label: &'static str,
    text: &'static str,
    byte_len: usize,
    char_count: usize,
    grapheme_count: usize,
) -> UnicodeFixture {
    UnicodeFixture {
        label,
        text,
        byte_len,
        char_count,
        grapheme_count,
    }
}

/// The corpus. Ordered roughly by how badly each entry breaks the
/// "one column is one byte is one char" assumption.
pub const CORPUS: &[UnicodeFixture] = &[
    // Control group: the shape every existing test already used.
    fixture("ascii_baseline", "hello world", 11, 11, 11),
    // byte_len != char_count, width == char_count. Cheapest possible trigger.
    fixture("accented_latin", "café", 5, 4, 4),
    fixture("accented_words", "naïve résumé", 15, 12, 12),
    // 2-byte scalars at width 1 — breaks byte arithmetic, not width arithmetic.
    fixture("cyrillic", "Привет мир", 19, 10, 10),
    // Greek: the one place char-wise lowercasing differs from `str::to_lowercase`.
    fixture("greek_final_sigma", "ΟΔΟΣ", 8, 4, 4),
    // U+0130 lowercases to *two* chars, so a lowercased index is not a source index.
    fixture("turkish_dotted_i", "İstanbul", 9, 8, 8),
    // Wide cells: char_count is *less* than the columns occupied, so a column
    // index read as a char index runs past the end of the vector.
    fixture("cjk", "日本語", 9, 3, 3),
    fixture("cjk_mixed_ascii", "日本語 test", 14, 8, 8),
    fixture("hangul", "한글 텍스트", 16, 6, 6),
    // 4-byte scalars, width 2.
    fixture("emoji", "😀", 4, 1, 1),
    fixture("emoji_run", "😀😁😂", 12, 3, 3),
    // Multi-scalar graphemes: char_count and grapheme_count part company here.
    fixture(
        "emoji_zwj_family",
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
        18,
        5,
        1,
    ),
    fixture("emoji_skin_tone", "\u{1F44D}\u{1F3FD}", 8, 2, 1),
    fixture("flag_emoji", "\u{1F1EF}\u{1F1F5}", 8, 2, 1),
    // Combining marks: 2 chars, 1 grapheme, 1 column.
    fixture("combining_mark", "e\u{0301}", 3, 2, 1),
    fixture("combining_in_word", "cafe\u{0301}", 6, 5, 4),
    // RTL: guards against ordering assumptions in slicing code.
    fixture("rtl_arabic", "مرحبا", 10, 5, 5),
    fixture("rtl_hebrew", "שלום", 8, 4, 4),
    // Zero-width: characters that occupy no column at all.
    fixture("zero_width_space", "a\u{200B}b", 5, 3, 3),
    fixture("zero_width_joiner", "a\u{200D}b", 5, 3, 2),
    // Realistic paste content: 3-byte quotes in otherwise-ASCII text.
    fixture("curly_quotes", "\u{201C}x\u{201D}", 7, 3, 3),
    // Boundaries at irregular byte offsets.
    fixture("mixed_scripts", "a日b😀c", 10, 5, 5),
];

/// Fixtures whose every character is East Asian Wide, so the row occupies
/// exactly `2 * char_count` grid columns. Used by tests that need a row where
/// the column index provably exceeds the character count.
pub const WIDE_FIXTURES: &[&str] = &["日本語", "한글", "中文字"];

/// Fixtures where one grapheme spans several scalars, so a char index and a
/// rendered cell are not the same thing.
pub fn multi_scalar_graphemes() -> impl Iterator<Item = &'static UnicodeFixture> {
    CORPUS
        .iter()
        .filter(|fixture| fixture.grapheme_count < fixture.char_count)
}

/// Fixtures containing at least one non-ASCII character.
pub fn non_ascii() -> impl Iterator<Item = &'static UnicodeFixture> {
    CORPUS.iter().filter(|fixture| !fixture.text.is_ascii())
}

/// A deterministic 64-bit LCG.
///
/// Stands in for `proptest`: it keeps every failure reproducible from a seed
/// printed in the assertion, adds no dependency to a published workspace, and
/// cannot produce the "flaky on CI only" failures the spec warns about.
pub struct Lcg(u64);

impl Lcg {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    /// Uniform-ish index below `len`. Uses the high bits, which are the good
    /// ones in an LCG.
    pub fn below(&mut self, len: usize) -> usize {
        if len == 0 {
            return 0;
        }
        (self.next_u64() >> 33) as usize % len
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    /// Concatenate one to `max_atoms` corpus texts into a new string.
    pub fn corpus_string(&mut self, max_atoms: usize) -> String {
        let count = 1 + self.below(max_atoms);
        let mut text = String::new();
        for _ in 0..count {
            text.push_str(self.pick(CORPUS).text);
        }
        text
    }
}

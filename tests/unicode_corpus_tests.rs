//! Drives the non-ASCII corpus through every path where a terminal column, a
//! UTF-8 byte offset and a `char` index are conflated.
//!
//! Six crashes shipped from that conflation and 1,965 green tests missed all of
//! them, because column, byte offset and char index coincide for pure-ASCII
//! single-width rows and every test input was ASCII. These tests exist so a
//! seventh site fails here instead of in a user's terminal.
//!
//! # Two kinds of assertion, kept distinct
//!
//! **Correctness assertions** state what the code must do. **Pinning
//! assertions** record what it does today where that is arguably wrong; each
//! one is marked `DIVERGENCE` and says what the ideal would be. A pinned test
//! failing means someone changed the behaviour — read the comment and decide
//! whether it was on purpose. Asserting an ideal the code does not implement
//! would leave a permanently red suite, which is worse than a documented one.
//!
//! The largest divergence pinned here: **the char-index model is not the grid
//! column model.** `Grid::row_text` emits one grapheme per cell, so a ZWJ family
//! emoji is one column but five `char`s. Copy-mode motions count `char`s. Search
//! was deliberately made to agree with the motions rather than invent a third
//! convention, so both are consistently off for multi-scalar graphemes. These
//! tests pin that consistency rather than paper over it.

#[path = "common/unicode_corpus.rs"]
mod unicode_corpus;

use par_term::copy_mode::CopyModeState;
use par_term::paste_transform::{PasteTransform, transform};
use par_term::smart_selection::{find_word_boundaries, is_word_char};
use par_term::text_shaper::TextShaper;
use par_term_config::ShaderUniformValue;
use par_term_config::text::{
    byte_offset_to_column, column_to_byte_offset, lowercase_with_source_map, truncate_chars,
};
use unicode_corpus::{CORPUS, Lcg, WIDE_FIXTURES, multi_scalar_graphemes, non_ascii};

// ===========================================================================
// The corpus itself
// ===========================================================================

#[test]
fn unicode_corpus_self_test() {
    let shaper = TextShaper::new();
    assert!(CORPUS.len() >= 20, "the corpus has been gutted");

    for fixture in CORPUS {
        assert_eq!(
            fixture.text.len(),
            fixture.byte_len,
            "{}: byte_len drifted",
            fixture.label
        );
        assert_eq!(
            fixture.text.chars().count(),
            fixture.char_count,
            "{}: char_count drifted",
            fixture.label
        );
        assert_eq!(
            shaper.detect_grapheme_clusters(fixture.text).len(),
            fixture.grapheme_count,
            "{}: grapheme_count drifted",
            fixture.label
        );
    }
}

#[test]
fn the_corpus_actually_makes_the_three_counts_diverge() {
    // A corpus where byte == char == grapheme everywhere would compile, pass,
    // and catch nothing. Prove each divergence is represented.
    assert!(
        non_ascii().any(|f| f.byte_len > f.char_count),
        "no fixture separates bytes from chars"
    );
    assert!(
        multi_scalar_graphemes().count() >= 4,
        "too few fixtures separate graphemes from chars"
    );
    assert!(
        CORPUS
            .iter()
            .any(|f| f.text.chars().count() == 1 && f.byte_len == 4),
        "no 4-byte single scalar (emoji) in the corpus"
    );
    assert!(
        WIDE_FIXTURES.iter().all(|text| !text.is_ascii()),
        "the wide-fixture list must hold wide characters"
    );
}

// ===========================================================================
// par_term_config::text — the shared helpers the fixes introduced
// ===========================================================================

#[test]
fn column_to_byte_offset_always_lands_on_a_char_boundary() {
    for fixture in CORPUS {
        // Deliberately overshoot: a column index can exceed the char count on
        // any row containing wide characters, which is exactly the case that
        // used to panic.
        for column in 0..=fixture.char_count + 8 {
            let offset = column_to_byte_offset(fixture.text, column);
            assert!(
                offset <= fixture.byte_len,
                "{}: column {column} gave offset {offset} past the end",
                fixture.label
            );
            assert!(
                fixture.text.is_char_boundary(offset),
                "{}: column {column} gave non-boundary offset {offset}",
                fixture.label
            );
            // The result must be usable for slicing without panicking.
            let _ = &fixture.text[..offset];
        }
    }
}

#[test]
fn byte_offset_to_column_inverts_column_to_byte_offset() {
    for fixture in CORPUS {
        for column in 0..=fixture.char_count {
            let offset = column_to_byte_offset(fixture.text, column);
            assert_eq!(
                byte_offset_to_column(fixture.text, offset),
                column,
                "{}: round trip failed at column {column}",
                fixture.label
            );
        }
    }
}

#[test]
fn byte_offset_to_column_tolerates_offsets_inside_a_character() {
    // A byte offset that splits a character must resolve to the character
    // containing it rather than panicking — this is the guard that makes the
    // helper safe to call with arithmetic derived from a column.
    for fixture in non_ascii() {
        for offset in 0..=fixture.byte_len {
            let column = byte_offset_to_column(fixture.text, offset);
            assert!(
                column <= fixture.char_count,
                "{}: offset {offset} gave column {column} > char_count",
                fixture.label
            );
        }
    }
}

#[test]
fn truncate_chars_never_splits_a_character() {
    for fixture in CORPUS {
        for max_chars in 0..=fixture.char_count + 4 {
            let truncated = truncate_chars(fixture.text, max_chars);
            assert!(
                truncated.chars().count() <= max_chars,
                "{}: truncate to {max_chars} produced {} chars",
                fixture.label,
                truncated.chars().count()
            );
            assert!(
                fixture.text.starts_with(truncated),
                "{}: truncation is not a prefix",
                fixture.label
            );
            assert_eq!(
                truncated.chars().count(),
                max_chars.min(fixture.char_count),
                "{}: truncation dropped or kept the wrong number of chars",
                fixture.label
            );
        }
    }
}

#[test]
fn truncate_chars_replaces_the_byte_slicing_that_used_to_panic() {
    // The display-truncation sites all did `&s[..n]` with a column-derived `n`.
    // For each fixture find a byte cap that is *not* a char boundary and show
    // the helper handles the same number safely.
    for fixture in non_ascii() {
        let Some(bad_offset) = (1..fixture.byte_len).find(|n| !fixture.text.is_char_boundary(*n))
        else {
            continue; // every offset is a boundary (all-ASCII or 1-byte scalars)
        };
        // `&fixture.text[..bad_offset]` would panic here. The helper treats the
        // number as a character count and returns something sliceable.
        let truncated = truncate_chars(fixture.text, bad_offset);
        assert!(fixture.text.starts_with(truncated));
    }
}

#[test]
fn lowercase_source_map_stays_aligned_across_the_corpus() {
    for fixture in CORPUS {
        let (lowered, sources) = lowercase_with_source_map(fixture.text);
        assert_eq!(
            lowered.chars().count(),
            sources.len(),
            "{}: source map length does not match the lowered text",
            fixture.label
        );
        assert!(
            sources.windows(2).all(|pair| pair[0] <= pair[1]),
            "{}: source map is not monotonic",
            fixture.label
        );
        for source in &sources {
            assert!(
                *source < fixture.char_count,
                "{}: source index {source} is out of range",
                fixture.label
            );
        }
    }
}

#[test]
fn lowercase_expands_turkish_dotted_i_and_the_map_absorbs_it() {
    // U+0130 lowercases to two characters, so a position in the lowered text is
    // not a position in the source. This is the subtle half of the copy-mode
    // search defect: a naive fix that lowercases both sides still misplaces the
    // match column without this map.
    let (lowered, sources) = lowercase_with_source_map("İstanbul");
    assert_eq!("İstanbul".chars().count(), 8);
    assert_eq!(lowered.chars().count(), 9, "lowering added a character");
    assert_eq!(sources.len(), 9);
    assert_eq!(sources[0], 0);
    assert_eq!(
        sources[1], 0,
        "the combining dot maps back to the same source"
    );
    assert_eq!(sources[2], 1, "the next real character maps to source 1");
    assert_eq!(*sources.last().expect("non-empty"), 7);
}

#[test]
fn char_wise_lowering_diverges_from_str_to_lowercase_only_on_final_sigma() {
    // DIVERGENCE (pinned, and documented in par-term-config/src/text.rs).
    // `lowercase_with_source_map` lowers character by character, so it cannot
    // apply the Greek final-sigma rule that `str::to_lowercase` implements.
    // Callers that lower both haystack and needle through this function stay
    // self-consistent, which is why the divergence is acceptable — but it is
    // real, and this test proves the documented claim rather than trusting it.
    let (char_wise, _) = lowercase_with_source_map("ΟΔΟΣ");
    assert_eq!(
        char_wise, "οδοσ",
        "char-wise lowering yields a medial sigma"
    );
    assert_eq!(
        "ΟΔΟΣ".to_lowercase(),
        "οδος",
        "str::to_lowercase uses final sigma"
    );
    assert_ne!(char_wise, "ΟΔΟΣ".to_lowercase());

    // Everywhere else in the corpus the two agree, which is the other half of
    // the claim and the part that would break silently.
    for fixture in CORPUS {
        if fixture.label == "greek_final_sigma" {
            continue;
        }
        let (char_wise, _) = lowercase_with_source_map(fixture.text);
        assert_eq!(
            char_wise,
            fixture.text.to_lowercase(),
            "{}: char-wise lowering diverged outside the documented case",
            fixture.label
        );
    }
}

// ===========================================================================
// Copy-mode motions
// ===========================================================================

/// Copy mode entered on an 80-column grid with the cursor parked at the last
/// column, which is what `$` (`move_to_line_end`) does.
fn at_line_end(cols: usize) -> CopyModeState {
    let mut state = CopyModeState::new();
    state.enter(0, 0, cols, 24, 0);
    state.move_to_line_end();
    state
}

fn at_column(column: usize, cols: usize) -> CopyModeState {
    let mut state = CopyModeState::new();
    state.enter(column, 0, cols, 24, 0);
    state
}

/// A line-walking motion: `(state, line_text, word_characters)`.
type Motion = fn(&mut CopyModeState, &str, &str);

/// Every motion that indexes into a line by column. These are the functions
/// where a column index meets a `char` vector.
const LINE_MOTIONS: &[(&str, Motion)] = &[
    ("move_word_forward", |s, line, wc| {
        s.move_word_forward(line, wc)
    }),
    ("move_word_backward", |s, line, wc| {
        s.move_word_backward(line, wc)
    }),
    ("move_word_end", |s, line, wc| s.move_word_end(line, wc)),
    ("move_big_word_forward", |s, line, _| {
        s.move_big_word_forward(line)
    }),
    ("move_big_word_backward", |s, line, _| {
        s.move_big_word_backward(line)
    }),
    ("move_big_word_end", |s, line, _| s.move_big_word_end(line)),
    ("move_to_first_non_blank", |s, line, _| {
        s.move_to_first_non_blank(line)
    }),
];

#[test]
fn every_motion_survives_every_cursor_column_on_every_fixture() {
    // The original crash: `$` parks the cursor at column 79 on a row holding
    // three characters, and the backward motions index the char vector with it.
    // Sweep every motion against every column, including far past the text.
    const COLS: usize = 80;
    let word_chars = "/-+\\~_.";

    for fixture in CORPUS {
        for column in 0..COLS {
            for (name, motion) in LINE_MOTIONS {
                let mut state = at_column(column, COLS);
                motion(&mut state, fixture.text, word_chars);
                assert!(
                    state.cursor_col < COLS,
                    "{} left the cursor at {} on {} from column {column}",
                    name,
                    state.cursor_col,
                    fixture.label
                );
            }
        }
    }
}

#[test]
fn backward_motions_from_line_end_land_inside_the_text() {
    // The specific reproduction: `$` then `b` on a row of wide characters.
    for fixture in CORPUS {
        if fixture.char_count == 0 {
            continue;
        }
        for (name, backward) in [
            ("move_word_backward", true),
            ("move_big_word_backward", false),
        ] {
            let mut state = at_line_end(80);
            if backward {
                state.move_word_backward(fixture.text, "");
            } else {
                state.move_big_word_backward(fixture.text);
            }
            assert!(
                state.cursor_col < fixture.char_count,
                "{} on {} left the cursor at {}, outside the {} characters of text",
                name,
                fixture.label,
                state.cursor_col,
                fixture.char_count
            );
        }
    }
}

#[test]
fn wide_character_rows_have_more_columns_than_characters() {
    // This is the structural reason the crash existed, stated as a test: on a
    // wide row the grid column index legitimately exceeds the character count,
    // so any motion that seeds a char index from `cursor_col` must clamp.
    for text in WIDE_FIXTURES {
        let char_count = text.chars().count();
        let occupied_columns = char_count * 2;
        assert!(
            occupied_columns > char_count,
            "{text:?} should occupy more columns than it has characters"
        );

        // Park the cursor at the last column the row occupies and move back.
        let mut state = at_column(occupied_columns - 1, 80);
        state.move_word_backward(text, "");
        assert!(state.cursor_col < char_count);
    }
}

#[test]
fn word_motion_counts_scalars_not_graphemes() {
    // DIVERGENCE (pinned, not endorsed). `Grid::row_text` pushes one grapheme
    // per cell, so a ZWJ family emoji renders in one cell — but the motions
    // walk `chars()`, so they see five. Moving forward over a row of ZWJ emoji
    // therefore lands on a column the grid never renders a character at.
    //
    // Ideal behaviour would be for motions to walk graphemes, matching the
    // grid. That is a real behaviour change across copy mode and search, which
    // were deliberately made to agree with each other. This test records the
    // current, self-consistent convention so a future unification is a visible
    // decision rather than an accident.
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    let line = format!("{family} tail");
    assert_eq!(family.chars().count(), 5, "five scalars");
    assert_eq!(
        TextShaper::new().detect_grapheme_clusters(family).len(),
        1,
        "one grapheme, so the grid renders one cell"
    );

    let mut state = at_column(0, 80);
    state.move_big_word_forward(&line);
    // Grapheme-based motion would put the cursor at column 2 ("tail" after one
    // emoji cell and one space). Scalar-based motion puts it at 6.
    assert_eq!(
        state.cursor_col, 6,
        "pinned: the motion skipped five scalars, not one grapheme"
    );
    assert_eq!(
        line.chars().nth(6),
        Some('t'),
        "column 6 is 't' in char space"
    );
}

#[test]
fn combining_marks_are_separate_columns_to_the_motions() {
    // DIVERGENCE (pinned, not endorsed). "cafe" + U+0301 is four graphemes and
    // four rendered cells, but five scalars, and the motions count scalars.
    let text = "cafe\u{0301}";
    assert_eq!(text.chars().count(), 5);
    assert_eq!(TextShaper::new().detect_grapheme_clusters(text).len(), 4);

    let mut state = at_column(4, 80);
    state.move_word_backward(text, "");
    assert_eq!(
        state.cursor_col, 0,
        "the whole run is one word regardless of the mark"
    );
}

#[test]
fn a_combining_mark_continues_the_word_it_attaches_to() {
    // Regression test. `is_word_char` was `is_alphanumeric() || contains(ch)`.
    // Combining marks are Unicode category Mn, which is not alphanumeric, and
    // neither are ZWJ or emoji modifiers — so each of them terminated a word,
    // even though the grid draws the mark inside the preceding cell.
    //
    // The user-visible symptoms: double-clicking "café" typed as "cafe" +
    // U+0301 selected "cafe" and dropped the accent, and double-clicking a
    // skin-toned 👍🏽 selected half the grapheme. Precomposed é worked, and that
    // contrast is what identified it as a defect rather than a Unicode quirk.
    for (label, ch) in [
        ("combining acute", '\u{0301}'),
        ("zero width joiner", '\u{200D}'),
        ("skin tone modifier", '\u{1F3FD}'),
    ] {
        assert!(is_word_char(ch, ""), "{label}: continues the previous word");
    }

    // Zero-width space is a separator, not a mark, so it stays a non-word
    // character. Grouping it with the marks would merge words either side of it.
    assert!(!is_word_char('\u{200B}', ""));

    let text = "cafe\u{0301}";
    assert_eq!(
        find_word_boundaries(text, 3, ""),
        (0, 4),
        "the accent belongs to the word it sits on"
    );
    assert_eq!(
        find_word_boundaries(text, 4, ""),
        (0, 4),
        "clicking the mark selects the whole word, not the mark alone"
    );

    // Precomposed é continues to select as before.
    assert!(is_word_char('é', ""));
    assert_eq!(find_word_boundaries("café", 3, ""), (0, 3));
}

#[test]
fn motions_are_stable_under_randomized_corpus_lines() {
    // Fuzz-style sweep: build lines from corpus atoms and whitespace, then run
    // random motion sequences. The property is "never panics and never leaves
    // the cursor outside the grid" — that generalizes to sites nobody audited.
    const COLS: usize = 40;
    let mut rng = Lcg::new(0x00C0_FFEE_D00D_0001);

    for case in 0..3_000 {
        let mut line = rng.corpus_string(3);
        if rng.next_u64() & 1 == 0 {
            line.push(' ');
            line.push_str(rng.pick(CORPUS).text);
        }

        let mut state = at_column(rng.below(COLS), COLS);
        for _ in 0..6 {
            match rng.below(8) {
                0 => state.move_word_forward(&line, "_-"),
                1 => state.move_word_backward(&line, "_-"),
                2 => state.move_word_end(&line, "_-"),
                3 => state.move_big_word_forward(&line),
                4 => state.move_big_word_backward(&line),
                5 => state.move_big_word_end(&line),
                6 => state.move_to_first_non_blank(&line),
                _ => state.move_to_line_end(),
            }
            assert!(
                state.cursor_col < COLS,
                "case {case}: cursor escaped the grid at {} on {line:?}",
                state.cursor_col
            );
        }
    }
}

// ===========================================================================
// Smart selection — the other column-indexed line walker
// ===========================================================================

#[test]
fn find_word_boundaries_returns_columns_inside_the_line() {
    for fixture in CORPUS {
        for column in 0..fixture.char_count + 8 {
            let (start, end) = find_word_boundaries(fixture.text, column, "_-./");
            assert!(
                start <= end,
                "{}: inverted range at {column}",
                fixture.label
            );
            if column < fixture.char_count {
                assert!(
                    end < fixture.char_count,
                    "{}: end {end} past the {} characters at column {column}",
                    fixture.label,
                    fixture.char_count
                );
                // Both ends must be usable as char indices.
                assert!(fixture.text.chars().nth(start).is_some());
                assert!(fixture.text.chars().nth(end).is_some());
            }
        }
    }
}

#[test]
fn find_word_boundaries_selects_a_whole_non_ascii_word() {
    // "un café au lait": characters 3..7 are "café".
    let line = "un café au lait";
    let (start, end) = find_word_boundaries(line, 4, "");
    assert_eq!((start, end), (3, 6));
    assert_eq!(
        line.chars()
            .skip(start)
            .take(end - start + 1)
            .collect::<String>(),
        "café"
    );

    // The accented character itself is a valid click target.
    let (start, end) = find_word_boundaries(line, 6, "");
    assert_eq!((start, end), (3, 6));
}

// ===========================================================================
// Paste transforms
// ===========================================================================

#[test]
fn no_paste_transform_panics_on_any_corpus_input() {
    // Every transform against every fixture. Several decoders index tables by
    // byte and pair characters by position; on non-ASCII input they must return
    // Err, never panic and never produce mojibake.
    // The property is *non-panic*, and that alone: `transform` returns a
    // `String`, so UTF-8 validity is guaranteed by the type and asserting it
    // would prove nothing. The only extra thing worth checking is that a
    // failure carries a message a user can act on.
    for fixture in CORPUS {
        for transform_kind in PasteTransform::all() {
            if let Err(message) = transform(fixture.text, *transform_kind) {
                assert!(
                    !message.is_empty(),
                    "{}: {transform_kind:?} failed with an empty message",
                    fixture.label
                );
            }
        }
    }
}

#[test]
fn base64_and_hex_round_trip_every_fixture() {
    for fixture in CORPUS {
        let encoded = transform(fixture.text, PasteTransform::EncodeBase64)
            .unwrap_or_else(|e| panic!("{}: base64 encode failed: {e}", fixture.label));
        assert!(
            encoded.is_ascii(),
            "{}: base64 output must be ASCII",
            fixture.label
        );
        let decoded = transform(&encoded, PasteTransform::DecodeBase64)
            .unwrap_or_else(|e| panic!("{}: base64 decode failed: {e}", fixture.label));
        assert_eq!(
            decoded, fixture.text,
            "{}: base64 round trip",
            fixture.label
        );

        let encoded = transform(fixture.text, PasteTransform::EncodeHex)
            .unwrap_or_else(|e| panic!("{}: hex encode failed: {e}", fixture.label));
        assert_eq!(
            encoded.len(),
            fixture.byte_len * 2,
            "{}: hex encodes bytes, so the output is 2x the byte length",
            fixture.label
        );
        let decoded = transform(&encoded, PasteTransform::DecodeHex)
            .unwrap_or_else(|e| panic!("{}: hex decode failed: {e}", fixture.label));
        assert_eq!(decoded, fixture.text, "{}: hex round trip", fixture.label);
    }
}

#[test]
fn url_and_json_round_trip_every_fixture() {
    for fixture in CORPUS {
        let encoded = transform(fixture.text, PasteTransform::EncodeUrl)
            .unwrap_or_else(|e| panic!("{}: url encode failed: {e}", fixture.label));
        let decoded = transform(&encoded, PasteTransform::DecodeUrl)
            .unwrap_or_else(|e| panic!("{}: url decode failed: {e}", fixture.label));
        assert_eq!(decoded, fixture.text, "{}: url round trip", fixture.label);

        let encoded = transform(fixture.text, PasteTransform::EncodeJsonEscape)
            .unwrap_or_else(|e| panic!("{}: json escape failed: {e}", fixture.label));
        let decoded = transform(&encoded, PasteTransform::DecodeJsonUnescape)
            .unwrap_or_else(|e| panic!("{}: json unescape failed: {e}", fixture.label));
        assert_eq!(decoded, fixture.text, "{}: json round trip", fixture.label);
    }
}

#[test]
fn decoders_reject_non_ascii_input_instead_of_indexing_by_byte() {
    // The Base64 decoder builds a 256-entry table and used to index it with the
    // character value; any codepoint at or above U+0100 ran off the end. The
    // hex decoder paired digits by byte, so an even-length ASCII string mixed
    // with a 2-byte character split that character down the middle.
    for fixture in non_ascii() {
        let error = transform(fixture.text, PasteTransform::DecodeBase64)
            .expect_err(&format!("{}: raw text is not valid Base64", fixture.label));
        assert!(
            error.contains("Invalid Base64 character"),
            "{}: expected a character rejection, got {error:?}",
            fixture.label
        );

        let error = transform(fixture.text, PasteTransform::DecodeHex)
            .expect_err(&format!("{}: raw text is not valid hex", fixture.label));
        assert!(
            error.starts_with("Invalid hex"),
            "{}: expected a hex rejection, got {error:?}",
            fixture.label
        );
    }
}

#[test]
fn hex_decoder_pairs_characters_not_bytes() {
    // "abcé" is 5 bytes but 4 characters. Pairing by byte would see an even
    // count and try to combine half of `é` with a real digit; pairing by
    // character correctly rejects `é` as a non-digit.
    let input = "abcé";
    assert_eq!(input.len(), 5);
    assert_eq!(input.chars().count(), 4);
    let error = transform(input, PasteTransform::DecodeHex).expect_err("é is not a hex digit");
    assert!(error.contains('é'), "the error must name the bad character");

    // A valid even-length hex string still decodes.
    assert_eq!(
        transform("48690a", PasteTransform::DecodeHex).expect("valid hex"),
        "Hi\n"
    );
}

#[test]
fn case_transforms_preserve_non_ascii_content() {
    // Case transforms walk characters and rebuild strings; a byte-wise
    // implementation would corrupt multi-byte scalars into replacement
    // characters. Nothing may produce U+FFFD.
    for fixture in CORPUS {
        for transform_kind in [
            PasteTransform::CaseUppercase,
            PasteTransform::CaseLowercase,
            PasteTransform::CaseTitleCase,
            PasteTransform::CaseCamelCase,
            PasteTransform::CasePascalCase,
            PasteTransform::CaseSnakeCase,
            PasteTransform::CaseScreamingSnake,
            PasteTransform::CaseKebabCase,
        ] {
            let output = transform(fixture.text, transform_kind)
                .unwrap_or_else(|e| panic!("{}: {transform_kind:?} failed: {e}", fixture.label));
            assert!(
                !output.contains('\u{FFFD}'),
                "{}: {transform_kind:?} produced a replacement character",
                fixture.label
            );
        }
    }
}

#[test]
fn shell_quoting_keeps_non_ascii_intact() {
    for fixture in CORPUS {
        for transform_kind in [
            PasteTransform::ShellSingleQuotes,
            PasteTransform::ShellDoubleQuotes,
            PasteTransform::ShellBackslash,
        ] {
            let output = transform(fixture.text, transform_kind)
                .unwrap_or_else(|e| panic!("{}: {transform_kind:?} failed: {e}", fixture.label));
            // Quoting inserts backslashes and delimiters around ASCII
            // metacharacters, so the whole string is not preserved verbatim.
            // What must be preserved is every non-ASCII character: a byte-wise
            // escaper would split one and emit replacement characters.
            for ch in fixture.text.chars().filter(|c| !c.is_ascii()) {
                assert!(
                    output.contains(ch),
                    "{}: {transform_kind:?} lost {ch:?} from {output:?}",
                    fixture.label
                );
            }
            assert!(
                !output.contains('\u{FFFD}'),
                "{}: {transform_kind:?} produced a replacement character",
                fixture.label
            );
        }
    }
}

#[test]
fn transforms_are_stable_under_randomized_corpus_input() {
    // The highest-value property: no transform panics on any string built from
    // the corpus alphabet, whatever the byte/char alignment happens to be.
    //
    // UTF-8 validity is guaranteed by the `String` return type, so the checks
    // below assert things that can actually fail: the two encoders must emit
    // pure ASCII (a byte-wise bug would leak input bytes through), and every
    // encode/decode pair must be lossless whatever the byte/char alignment.
    let mut rng = Lcg::new(0x0BAD_F00D_1234_5678);
    for case in 0..2_000 {
        let text = rng.corpus_string(4);

        let transform_kind = *rng.pick(PasteTransform::all());
        if let Err(message) = transform(&text, transform_kind) {
            assert!(
                !message.is_empty(),
                "case {case}: {transform_kind:?} failed without a message"
            );
        }

        for (encode, decode) in [
            (PasteTransform::EncodeBase64, PasteTransform::DecodeBase64),
            (PasteTransform::EncodeHex, PasteTransform::DecodeHex),
        ] {
            let encoded =
                transform(&text, encode).unwrap_or_else(|e| panic!("case {case}: {encode:?}: {e}"));
            assert!(
                encoded.is_ascii(),
                "case {case}: {encode:?} emitted non-ASCII for {text:?}"
            );
            let decoded = transform(&encoded, decode)
                .unwrap_or_else(|e| panic!("case {case}: {decode:?}: {e}"));
            assert_eq!(
                decoded, text,
                "case {case}: {encode:?}/{decode:?} lost data"
            );
        }
    }
}

// ===========================================================================
// Shader colour hex parsing (the sixth motivating defect)
// ===========================================================================

#[test]
fn shader_color_hex_rejects_non_ascii_instead_of_slicing_mid_character() {
    // `ShaderColorValue::from_hex` slices the string by byte range to read each
    // channel. `#1é234` is six *bytes* after the `#` but five characters, so the
    // range 0..2 used to cut `é` in half and panic. `from_hex` is private; the
    // reachable surface is `ShaderUniformValue`, which is what a `config.yaml`
    // or a shared `.glsl` metadata block actually deserializes through.
    for bad in ["#1é234", "#é23456", "#12345é", "#日本語", "#🎉🎉"] {
        let yaml = format!("{bad:?}");
        let parsed = serde_yaml_ng::from_str::<ShaderUniformValue>(&yaml);
        assert!(
            parsed.is_err(),
            "{bad:?} must be rejected, not parsed or panicked on"
        );
    }

    // Well-formed colours still parse, so the guard is a rejection and not a
    // blanket failure.
    for good in ["#ff8800", "#ff8800cc", "#000000"] {
        let yaml = format!("{good:?}");
        let parsed = serde_yaml_ng::from_str::<ShaderUniformValue>(&yaml)
            .unwrap_or_else(|e| panic!("{good:?} is a valid colour but was rejected: {e}"));
        // Confirms the `#`-prefixed string really does route through the colour
        // parser, so the rejections above are `from_hex` doing its job rather
        // than the string being refused for some unrelated reason.
        assert!(
            matches!(parsed, ShaderUniformValue::Color(_)),
            "{good:?} parsed as {parsed:?}, not a colour"
        );
    }
}

#[test]
fn shader_color_hex_never_panics_on_corpus_derived_strings() {
    // Sweep every corpus fixture into every byte position of a hex literal.
    // Any surviving byte-range slice would panic on one of these.
    for fixture in CORPUS {
        for prefix_len in 0..=6 {
            let candidate = format!(
                "#{}{}{}",
                "1".repeat(prefix_len),
                fixture.text,
                "0".repeat(6usize.saturating_sub(prefix_len))
            );
            let yaml = format!("{candidate:?}");
            // The assertion is that this returns rather than unwinding.
            let _ = serde_yaml_ng::from_str::<ShaderUniformValue>(&yaml);
        }
    }
}

// ===========================================================================
// Search-shaped index arithmetic
// ===========================================================================

#[test]
fn a_column_returned_from_a_lowercased_search_addresses_the_source_character() {
    // Copy-mode search lowercases the line, finds a byte offset in the lowered
    // text, converts it to a column there, then maps back through the source
    // map. This reproduces that chain against the corpus and checks the final
    // column addresses the character the caller expects — the exact step that
    // returned a byte offset as a column before the fix.
    for fixture in non_ascii() {
        let (lowered, sources) = lowercase_with_source_map(fixture.text);
        let Some(needle) = lowered.chars().next() else {
            continue;
        };
        let needle = needle.to_string();

        let found_byte = lowered.find(&needle).expect("the first character occurs");
        let lowered_column = byte_offset_to_column(&lowered, found_byte);
        let source_column = sources
            .get(lowered_column)
            .copied()
            .expect("the source map covers every lowered character");

        assert!(
            source_column < fixture.char_count,
            "{}: mapped column {source_column} is outside the source text",
            fixture.label
        );
        assert!(
            fixture.text.chars().nth(source_column).is_some(),
            "{}: mapped column does not address a character",
            fixture.label
        );
    }
}

#[test]
fn a_byte_offset_used_as_a_column_would_have_been_wrong_here() {
    // Demonstrates the defect the corpus is designed to catch, so the test
    // above cannot be dismissed as testing nothing: on a CJK line the byte
    // offset of the second word is three times its column.
    let line = "日本語 test";
    let byte_offset = line.find("test").expect("the word is present");
    let column = byte_offset_to_column(line, byte_offset);
    assert_eq!(byte_offset, 10, "byte offset of 'test'");
    assert_eq!(column, 4, "column of 'test'");
    assert_ne!(byte_offset, column, "this is the whole bug in one line");
    assert_eq!(line.chars().nth(column), Some('t'));
    // Using the byte offset as a column would run off the end of the row.
    assert_eq!(line.chars().nth(byte_offset), None);
}

#[test]
fn index_helpers_are_stable_under_randomized_strings() {
    let mut rng = Lcg::new(0xFEED_FACE_CAFE_0001);
    for case in 0..5_000 {
        let text = rng.corpus_string(4);
        let column = rng.below(text.chars().count() + 10);

        let offset = column_to_byte_offset(&text, column);
        assert!(
            text.is_char_boundary(offset),
            "case {case}: offset {offset} is not a boundary in {text:?}"
        );
        assert!(offset <= text.len(), "case {case}: offset past the end");

        let back = byte_offset_to_column(&text, offset);
        assert!(
            back <= text.chars().count(),
            "case {case}: column {back} exceeds the char count"
        );

        let truncated = truncate_chars(&text, column);
        assert!(truncated.chars().count() <= column);
        assert!(text.starts_with(truncated));

        let (lowered, sources) = lowercase_with_source_map(&text);
        assert_eq!(lowered.chars().count(), sources.len(), "case {case}");
    }
}

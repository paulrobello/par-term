//! Tests for multi-character grapheme cluster rendering
//!
//! These tests verify proper handling of:
//! - Flag emoji (regional indicator pairs)
//! - ZWJ sequences (family emoji, profession emoji)
//! - Skin tone modifiers
//! - Combining characters (diacritics)
//! - Variation selectors

use par_term::text_shaper::TextShaper;

/// Test grapheme cluster detection for various emoji types
mod grapheme_detection {
    use super::*;

    #[test]
    fn test_flag_emoji_is_single_grapheme() {
        let shaper = TextShaper::new();

        // US flag: U+1F1FA U+1F1F8 (two regional indicators)
        let clusters = shaper.detect_grapheme_clusters("\u{1F1FA}\u{1F1F8}");
        assert_eq!(
            clusters.len(),
            1,
            "US flag should be detected as single grapheme cluster"
        );
        assert_eq!(clusters[0].1, "\u{1F1FA}\u{1F1F8}");

        // UK flag: U+1F1EC U+1F1E7
        let clusters = shaper.detect_grapheme_clusters("\u{1F1EC}\u{1F1E7}");
        assert_eq!(
            clusters.len(),
            1,
            "UK flag should be detected as single grapheme cluster"
        );

        // Japan flag: U+1F1EF U+1F1F5
        let clusters = shaper.detect_grapheme_clusters("\u{1F1EF}\u{1F1F5}");
        assert_eq!(
            clusters.len(),
            1,
            "Japan flag should be detected as single grapheme cluster"
        );
    }

    #[test]
    fn test_zwj_sequences_are_single_grapheme() {
        let shaper = TextShaper::new();

        // Family: man, woman, girl, boy
        // U+1F468 U+200D U+1F469 U+200D U+1F467 U+200D U+1F466
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
        let clusters = shaper.detect_grapheme_clusters(family);
        assert_eq!(
            clusters.len(),
            1,
            "Family emoji should be detected as single grapheme cluster"
        );

        // Woman technologist: U+1F469 U+200D U+1F4BB
        let technologist = "\u{1F469}\u{200D}\u{1F4BB}";
        let clusters = shaper.detect_grapheme_clusters(technologist);
        assert_eq!(
            clusters.len(),
            1,
            "Woman technologist should be detected as single grapheme cluster"
        );

        // Rainbow flag: U+1F3F3 U+FE0F U+200D U+1F308
        let rainbow = "\u{1F3F3}\u{FE0F}\u{200D}\u{1F308}";
        let clusters = shaper.detect_grapheme_clusters(rainbow);
        assert_eq!(
            clusters.len(),
            1,
            "Rainbow flag should be detected as single grapheme cluster"
        );
    }

    #[test]
    fn test_skin_tone_modifiers_are_single_grapheme() {
        let shaper = TextShaper::new();

        // Waving hand + medium skin tone: U+1F44B U+1F3FD
        let waving = "\u{1F44B}\u{1F3FD}";
        let clusters = shaper.detect_grapheme_clusters(waving);
        assert_eq!(
            clusters.len(),
            1,
            "Waving hand with skin tone should be single grapheme"
        );

        // Thumbs up + light skin tone: U+1F44D U+1F3FB
        let thumbs = "\u{1F44D}\u{1F3FB}";
        let clusters = shaper.detect_grapheme_clusters(thumbs);
        assert_eq!(
            clusters.len(),
            1,
            "Thumbs up with skin tone should be single grapheme"
        );

        // All skin tones for same emoji
        let base = '\u{1F44B}'; // Waving hand
        let skin_tones = [
            '\u{1F3FB}', // Light
            '\u{1F3FC}', // Medium-light
            '\u{1F3FD}', // Medium
            '\u{1F3FE}', // Medium-dark
            '\u{1F3FF}', // Dark
        ];
        for tone in skin_tones {
            let emoji = format!("{}{}", base, tone);
            let clusters = shaper.detect_grapheme_clusters(&emoji);
            assert_eq!(
                clusters.len(),
                1,
                "Emoji with skin tone modifier should be single grapheme"
            );
        }
    }

    #[test]
    fn test_combining_characters_are_single_grapheme() {
        let shaper = TextShaper::new();

        // e + combining acute accent = é
        let e_acute = "e\u{0301}";
        let clusters = shaper.detect_grapheme_clusters(e_acute);
        assert_eq!(
            clusters.len(),
            1,
            "e with combining acute should be single grapheme"
        );

        // a + combining ring above = å
        let a_ring = "a\u{030A}";
        let clusters = shaper.detect_grapheme_clusters(a_ring);
        assert_eq!(
            clusters.len(),
            1,
            "a with combining ring should be single grapheme"
        );

        // Multiple combining marks: a + macron + acute
        let multi = "a\u{0304}\u{0301}";
        let clusters = shaper.detect_grapheme_clusters(multi);
        assert_eq!(
            clusters.len(),
            1,
            "Multiple combining marks should be single grapheme"
        );
    }

    #[test]
    fn test_variation_selectors() {
        let shaper = TextShaper::new();

        // Heart with emoji presentation: U+2764 U+FE0F
        let heart_emoji = "\u{2764}\u{FE0F}";
        let clusters = shaper.detect_grapheme_clusters(heart_emoji);
        assert_eq!(
            clusters.len(),
            1,
            "Heart with emoji variation selector should be single grapheme"
        );

        // Heart with text presentation: U+2764 U+FE0E
        let heart_text = "\u{2764}\u{FE0E}";
        let clusters = shaper.detect_grapheme_clusters(heart_text);
        assert_eq!(
            clusters.len(),
            1,
            "Heart with text variation selector should be single grapheme"
        );
    }

    #[test]
    fn test_mixed_content_grapheme_count() {
        let shaper = TextShaper::new();

        // "Hello 🇺🇸 world 👋🏽!"
        // Should be: H e l l o <space> flag <space> w o r l d <space> wave !
        // = 5 + 1 + 1 + 1 + 5 + 1 + 1 + 1 = 16 graphemes
        let text = "Hello \u{1F1FA}\u{1F1F8} world \u{1F44B}\u{1F3FD}!";
        let clusters = shaper.detect_grapheme_clusters(text);
        assert_eq!(
            clusters.len(),
            16,
            "Mixed content should have correct count"
        );

        // Verify the emoji clusters specifically
        assert_eq!(clusters[6].1, "\u{1F1FA}\u{1F1F8}"); // US flag
        assert_eq!(clusters[14].1, "\u{1F44B}\u{1F3FD}"); // Waving hand with skin tone
    }
}

/// Test regional indicator detection helper
mod regional_indicators {
    use super::*;

    #[test]
    fn test_is_regional_indicator_pair() {
        let shaper = TextShaper::new();

        // Valid regional indicator pairs (flags)
        assert!(shaper.is_regional_indicator_pair("\u{1F1FA}\u{1F1F8}")); // US
        assert!(shaper.is_regional_indicator_pair("\u{1F1EC}\u{1F1E7}")); // GB
        assert!(shaper.is_regional_indicator_pair("\u{1F1E9}\u{1F1EA}")); // DE
        assert!(shaper.is_regional_indicator_pair("\u{1F1E6}\u{1F1FA}")); // AU

        // Invalid cases
        assert!(!shaper.is_regional_indicator_pair("US")); // ASCII letters
        assert!(!shaper.is_regional_indicator_pair("\u{1F1FA}")); // Single indicator
        assert!(!shaper.is_regional_indicator_pair("ABC")); // Three chars
        assert!(!shaper.is_regional_indicator_pair("\u{1F600}")); // Non-RI emoji
    }
}

/// Test ZWJ detection helper
mod zwj_detection {
    use super::*;

    #[test]
    fn test_contains_zwj() {
        let shaper = TextShaper::new();

        // Sequences with ZWJ
        assert!(shaper.contains_zwj("\u{1F468}\u{200D}\u{1F469}")); // Man + ZWJ + Woman
        assert!(shaper.contains_zwj("\u{1F469}\u{200D}\u{1F4BB}")); // Woman + ZWJ + Laptop
        assert!(shaper.contains_zwj("\u{1F3F3}\u{FE0F}\u{200D}\u{1F308}")); // Rainbow flag

        // Sequences without ZWJ
        assert!(!shaper.contains_zwj("\u{1F600}")); // Grinning face
        assert!(!shaper.contains_zwj("\u{1F1FA}\u{1F1F8}")); // Flag (uses RI, not ZWJ)
        assert!(!shaper.contains_zwj("\u{1F44B}\u{1F3FD}")); // Skin tone (uses modifier, not ZWJ)
        assert!(!shaper.contains_zwj("hello")); // Plain text
    }
}

/// Test grapheme cluster boundary detection
mod cluster_boundaries {
    use super::*;

    #[test]
    fn test_cluster_indices_are_byte_positions() {
        let shaper = TextShaper::new();

        // ASCII: each char is 1 byte
        let clusters = shaper.detect_grapheme_clusters("hello");
        assert_eq!(clusters[0].0, 0);
        assert_eq!(clusters[1].0, 1);
        assert_eq!(clusters[2].0, 2);

        // Multi-byte: check positions are correct
        let text = "a\u{1F600}b"; // a, grinning face (4 bytes), b
        let clusters = shaper.detect_grapheme_clusters(text);
        assert_eq!(clusters.len(), 3);
        assert_eq!(clusters[0].0, 0); // 'a' at byte 0
        assert_eq!(clusters[0].1, "a");
        assert_eq!(clusters[1].0, 1); // emoji at byte 1
        assert_eq!(clusters[1].1, "\u{1F600}");
        assert_eq!(clusters[2].0, 5); // 'b' at byte 5 (1 + 4)
        assert_eq!(clusters[2].1, "b");
    }

    #[test]
    fn test_empty_string() {
        let shaper = TextShaper::new();
        let clusters = shaper.detect_grapheme_clusters("");
        assert!(clusters.is_empty());
    }

    #[test]
    fn test_single_grapheme_strings() {
        let shaper = TextShaper::new();

        // Single ASCII char
        let clusters = shaper.detect_grapheme_clusters("x");
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].1, "x");

        // Single emoji
        let clusters = shaper.detect_grapheme_clusters("\u{1F600}");
        assert_eq!(clusters.len(), 1);

        // Single flag (2 codepoints, 1 grapheme)
        let clusters = shaper.detect_grapheme_clusters("\u{1F1FA}\u{1F1F8}");
        assert_eq!(clusters.len(), 1);
    }
}

/// Test edge cases and complex scenarios
mod edge_cases {
    use super::*;

    #[test]
    fn test_consecutive_flags() {
        let shaper = TextShaper::new();

        // Two flags in a row: 🇺🇸🇬🇧
        let text = "\u{1F1FA}\u{1F1F8}\u{1F1EC}\u{1F1E7}";
        let clusters = shaper.detect_grapheme_clusters(text);
        assert_eq!(
            clusters.len(),
            2,
            "Two consecutive flags should be 2 graphemes"
        );
        assert_eq!(clusters[0].1, "\u{1F1FA}\u{1F1F8}");
        assert_eq!(clusters[1].1, "\u{1F1EC}\u{1F1E7}");
    }

    #[test]
    fn test_emoji_in_text() {
        let shaper = TextShaper::new();

        // Text with embedded emoji: "I ❤️ Rust"
        // I (1) + space (1) + heart+VS16 (1) + space (1) + R (1) + u (1) + s (1) + t (1) = 8
        let text = "I \u{2764}\u{FE0F} Rust";
        let clusters = shaper.detect_grapheme_clusters(text);
        assert_eq!(clusters.len(), 8);
    }

    #[test]
    fn test_keycap_sequences() {
        let shaper = TextShaper::new();

        // Keycap digit one: 1 + FE0F + 20E3
        let keycap_one = "1\u{FE0F}\u{20E3}";
        let clusters = shaper.detect_grapheme_clusters(keycap_one);
        assert_eq!(
            clusters.len(),
            1,
            "Keycap sequence should be single grapheme"
        );
    }

    #[test]
    fn test_tag_sequences() {
        let shaper = TextShaper::new();

        // England flag uses tag sequences
        // U+1F3F4 + tag characters + U+E007F
        // This is complex but should still be single grapheme
        let england = "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}";
        let clusters = shaper.detect_grapheme_clusters(england);
        assert_eq!(
            clusters.len(),
            1,
            "Tag sequence (England flag) should be single grapheme"
        );
    }

    #[test]
    fn test_zwj_with_skin_tone() {
        let shaper = TextShaper::new();

        // Couple with heart, both with skin tones
        // This combines ZWJ and skin tone modifiers
        let couple = "\u{1F469}\u{1F3FB}\u{200D}\u{2764}\u{FE0F}\u{200D}\u{1F468}\u{1F3FD}";
        let clusters = shaper.detect_grapheme_clusters(couple);
        assert_eq!(
            clusters.len(),
            1,
            "Complex ZWJ with skin tones should be single grapheme"
        );
    }
}

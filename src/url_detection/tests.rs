//! Tests for URL and file path detection utilities.

use super::*;

#[test]
fn test_detect_http_url() {
    let text = "Visit https://example.com for more info";
    let urls = detect_urls_in_line(text, 0);
    assert_eq!(urls.len(), 1);
    assert_eq!(urls[0].url, "https://example.com");
    assert_eq!(urls[0].start_col, 6);
    assert_eq!(urls[0].end_col, 25); // Exclusive end position
}

#[test]
fn test_detect_www_url() {
    let text = "Check out www.example.com";
    let urls = detect_urls_in_line(text, 0);
    assert_eq!(urls.len(), 1);
    assert_eq!(urls[0].url, "www.example.com");
}

#[test]
fn test_detect_multiple_urls() {
    let text = "See https://example.com and http://test.org";
    let urls = detect_urls_in_line(text, 0);
    assert_eq!(urls.len(), 2);
    assert_eq!(urls[0].url, "https://example.com");
    assert_eq!(urls[1].url, "http://test.org");
}

#[test]
fn test_find_url_at_position() {
    let text = "Visit https://example.com for more";
    let urls = detect_urls_in_line(text, 5);

    // Position within URL
    assert!(find_url_at_position(&urls, 10, 5).is_some());

    // Position outside URL
    assert!(find_url_at_position(&urls, 0, 5).is_none());
    assert!(find_url_at_position(&urls, 30, 5).is_none());

    // Wrong row
    assert!(find_url_at_position(&urls, 10, 6).is_none());
}

#[test]
fn test_no_urls() {
    let text = "This line has no URLs at all";
    let urls = detect_urls_in_line(text, 0);
    assert_eq!(urls.len(), 0);
}

#[test]
fn test_url_schemes() {
    let text = "ftp://files.com ssh://git.com file:///path git://repo.com";
    let urls = detect_urls_in_line(text, 0);
    assert_eq!(urls.len(), 4);
}

#[test]
fn test_detect_relative_file_path() {
    let text = "./src/lambda_check_sf_status/.gitignore";
    let paths = detect_file_paths_in_line(text, 0);
    assert_eq!(paths.len(), 1, "Should detect exactly one path");
    assert_eq!(paths[0].url, "./src/lambda_check_sf_status/.gitignore");
    assert_eq!(paths[0].start_col, 0);
    assert_eq!(paths[0].end_col, text.len());
}

#[test]
fn test_detect_nested_path_no_double_match() {
    // This test ensures we don't match /src/handler.py inside ./foo/src/handler.py
    let text = "./src/lambda_sap_po_to_zen/src/handler.py";
    let paths = detect_file_paths_in_line(text, 0);
    assert_eq!(
        paths.len(),
        1,
        "Should detect exactly one path, not multiple overlapping ones"
    );
    assert_eq!(paths[0].url, text);
    assert_eq!(paths[0].start_col, 0);
}

#[test]
fn test_detect_home_path() {
    let text = "~/Documents/file.txt";
    let paths = detect_file_paths_in_line(text, 0);
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].url, "~/Documents/file.txt");
}

#[test]
fn test_detect_path_with_line_number() {
    let text = "./src/main.rs:42";
    let paths = detect_file_paths_in_line(text, 0);
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].url, "./src/main.rs");
    if let DetectedItemType::FilePath { line, column } = &paths[0].item_type {
        assert_eq!(*line, Some(42));
        assert_eq!(*column, None);
    } else {
        panic!("Expected FilePath type");
    }
}

#[test]
fn test_detect_path_with_line_and_col() {
    let text = "./src/main.rs:42:10";
    let paths = detect_file_paths_in_line(text, 0);
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].url, "./src/main.rs");
    if let DetectedItemType::FilePath { line, column } = &paths[0].item_type {
        assert_eq!(*line, Some(42));
        assert_eq!(*column, Some(10));
    } else {
        panic!("Expected FilePath type");
    }
}

#[test]
fn test_absolute_path_with_multiple_components() {
    let text = "/Users/probello/.claude";
    let paths = detect_file_paths_in_line(text, 0);
    assert_eq!(
        paths.len(),
        1,
        "Should match absolute path at start of string"
    );
    assert_eq!(paths[0].url, "/Users/probello/.claude");
    assert_eq!(paths[0].start_col, 0);
}

#[test]
fn test_absolute_path_after_whitespace() {
    let text = "ls /Users/probello/.claude";
    let paths = detect_file_paths_in_line(text, 0);
    assert_eq!(
        paths.len(),
        1,
        "Should match absolute path after whitespace"
    );
    assert_eq!(paths[0].url, "/Users/probello/.claude");
    assert_eq!(paths[0].start_col, 3);
}

#[test]
fn test_no_match_single_component_absolute_path() {
    // Single-component paths like /etc are too likely to be false positives
    let text = "/etc";
    let paths = detect_file_paths_in_line(text, 0);
    assert_eq!(
        paths.len(),
        0,
        "Should not match single-component absolute paths"
    );
}

#[test]
fn test_no_false_absolute_match_inside_relative() {
    // Absolute path branch should NOT match /bar/baz inside ./foo/bar/baz
    let text = "./foo/bar/baz";
    let paths = detect_file_paths_in_line(text, 0);
    assert_eq!(
        paths.len(),
        1,
        "Should only match the relative path, not internal absolute"
    );
    assert_eq!(paths[0].url, "./foo/bar/baz");
}

/// Verify that regex byte offsets can be correctly mapped to column indices
/// when multi-byte UTF-8 characters precede the matched text.
/// This is the mapping that url_hover.rs applies after detection.
#[test]
fn test_byte_offset_to_column_mapping_with_multibyte() {
    // Simulate a terminal line: "★ ~/docs" where ★ is a 3-byte UTF-8 char
    // Cell layout: [★][ ][~][/][d][o][c][s]
    // Columns:      0   1  2  3  4  5  6  7
    let graphemes = ["★", " ", "~", "/", "d", "o", "c", "s"];
    let cols = graphemes.len();

    // Build line and byte-to-col mapping (same logic as url_hover.rs)
    let mut line = String::new();
    let mut byte_to_col: Vec<usize> = Vec::new();
    for (col_idx, g) in graphemes.iter().enumerate() {
        for _ in 0..g.len() {
            byte_to_col.push(col_idx);
        }
        line.push_str(g);
    }
    byte_to_col.push(cols); // sentinel

    let map = |b: usize| -> usize { byte_to_col.get(b).copied().unwrap_or(cols) };

    // Detect file path in the concatenated string
    let paths = detect_file_paths_in_line(&line, 0);
    assert_eq!(paths.len(), 1, "Should detect ~/docs");

    // The regex returns byte offsets: "★" is 3 bytes, " " is 1 byte
    // so ~/docs starts at byte 4 (not column 2)
    assert_eq!(paths[0].start_col, 4, "Byte offset should be 4");

    // After mapping, column index should be 2
    let start_col = map(paths[0].start_col);
    let end_col = map(paths[0].end_col);
    assert_eq!(start_col, 2, "Column should be 2 (after ★ and space)");
    assert_eq!(end_col, cols, "End column should be 8 (end of line)");
}

// --- ensure_url_scheme tests ---

#[test]
fn test_ensure_url_scheme_adds_https_when_no_scheme() {
    assert_eq!(
        ensure_url_scheme("www.example.com"),
        "https://www.example.com"
    );
    assert_eq!(
        ensure_url_scheme("example.com/path"),
        "https://example.com/path"
    );
}

#[test]
fn test_ensure_url_scheme_preserves_existing_scheme() {
    assert_eq!(
        ensure_url_scheme("https://example.com"),
        "https://example.com"
    );
    assert_eq!(
        ensure_url_scheme("http://example.com"),
        "http://example.com"
    );
    assert_eq!(
        ensure_url_scheme("ftp://files.example.com"),
        "ftp://files.example.com"
    );
    assert_eq!(
        ensure_url_scheme("file:///tmp/test.html"),
        "file:///tmp/test.html"
    );
}

// --- expand_link_handler tests ---

#[test]
fn test_expand_link_handler_replaces_url_placeholder() {
    let parts =
        expand_link_handler("firefox {url}", "https://example.com").expect("should succeed");
    assert_eq!(parts, vec!["firefox", "https://example.com"]);
}

#[test]
fn test_expand_link_handler_multi_word_command() {
    let parts = expand_link_handler("open -a Firefox {url}", "https://example.com")
        .expect("should succeed");
    assert_eq!(parts, vec!["open", "-a", "Firefox", "https://example.com"]);
}

#[test]
fn test_expand_link_handler_no_placeholder() {
    // If command has no {url}, it still works - the URL just doesn't appear
    let parts = expand_link_handler("my-browser", "https://example.com").expect("should succeed");
    assert_eq!(parts, vec!["my-browser"]);
}

#[test]
fn test_expand_link_handler_errors_on_empty_expansion() {
    // A command that is only whitespace after expansion should error
    let result = expand_link_handler("   ", "https://example.com");
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        "Link handler command is empty after expansion"
    );
}

#[test]
fn test_expand_link_handler_empty_command() {
    let result = expand_link_handler("", "https://example.com");
    assert!(result.is_err());
}

// --- H1 security: URL argument injection prevention ---

#[test]
fn test_expand_link_handler_url_with_spaces_stays_single_arg() {
    // A crafted URL with spaces must NOT inject additional arguments
    let parts = expand_link_handler(
        "firefox {url}",
        "https://evil.com --new-window javascript:alert(1)",
    )
    .expect("should succeed");
    // The URL (including its spaces) must remain a single argument
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], "firefox");
    assert_eq!(
        parts[1],
        "https://evil.com --new-window javascript:alert(1)"
    );
}

#[test]
fn test_expand_link_handler_url_with_shell_metacharacters() {
    // Shell metacharacters in URLs must not cause issues
    let parts = expand_link_handler("open {url}", "https://example.com/search?q=foo&bar=baz|cat")
        .expect("should succeed");
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[1], "https://example.com/search?q=foo&bar=baz|cat");
}

#[test]
fn test_expand_link_handler_quoted_template_preserved() {
    // A template that uses shell quoting should be parsed correctly
    let parts = expand_link_handler("open -a 'Google Chrome' {url}", "https://example.com")
        .expect("should succeed");
    assert_eq!(
        parts,
        vec!["open", "-a", "Google Chrome", "https://example.com"]
    );
}

// --- H2 security: shell_escape tests ---

#[test]
fn test_shell_escape_basic_path() {
    assert_eq!(shell_escape("/tmp/file.txt"), "'/tmp/file.txt'");
}

#[test]
fn test_shell_escape_path_with_single_quotes() {
    assert_eq!(
        shell_escape("/tmp/it's a file.txt"),
        "'/tmp/it'\\''s a file.txt'"
    );
}

#[test]
fn test_shell_escape_path_with_backticks() {
    // Backticks inside single quotes are safe (not interpreted)
    assert_eq!(
        shell_escape("/tmp/`rm -rf /`/file.txt"),
        "'/tmp/`rm -rf /`/file.txt'"
    );
}

#[test]
fn test_shell_escape_path_with_dollar_expansion() {
    // $(cmd) inside single quotes is safe (not interpreted)
    assert_eq!(
        shell_escape("/tmp/$(whoami)/file.txt"),
        "'/tmp/$(whoami)/file.txt'"
    );
}

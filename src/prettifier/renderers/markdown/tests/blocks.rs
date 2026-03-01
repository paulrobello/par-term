//! Block classification tests for the Markdown renderer.

use super::super::blocks::{
    BlockElement, classify_blocks, is_separator_row, is_table_row, parse_alignment,
    parse_table_cells,
};
use crate::prettifier::renderers::table::ColumnAlignment;

// -- Table helper tests --

#[test]
fn test_is_table_row() {
    assert!(is_table_row("| A | B |"));
    assert!(is_table_row("| A |"));
    assert!(!is_table_row("no pipes here"));
    assert!(!is_table_row(""));
}

#[test]
fn test_is_separator_row() {
    assert!(is_separator_row("|---|---|"));
    assert!(is_separator_row("| --- | --- |"));
    assert!(is_separator_row("|:---|:---:|---:|"));
    assert!(!is_separator_row("| A | B |"));
    assert!(!is_separator_row("plain text"));
}

#[test]
fn test_parse_table_cells() {
    let cells = parse_table_cells("| A | B | C |");
    assert_eq!(cells, vec!["A", "B", "C"]);
}

#[test]
fn test_parse_alignment() {
    assert_eq!(parse_alignment(":---"), ColumnAlignment::Left);
    assert_eq!(parse_alignment(":---:"), ColumnAlignment::Center);
    assert_eq!(parse_alignment("---:"), ColumnAlignment::Right);
    assert_eq!(parse_alignment("---"), ColumnAlignment::Left);
}

// -- Block classification tests --

#[test]
fn test_classify_code_block() {
    let lines: Vec<String> = vec![
        "```rust".to_string(),
        "let x = 1;".to_string(),
        "```".to_string(),
    ];
    let blocks = classify_blocks(&lines);
    assert_eq!(blocks.len(), 1);
    assert!(
        matches!(&blocks[0], BlockElement::CodeBlock { language: Some(lang), .. } if lang == "rust")
    );
}

#[test]
fn test_classify_table() {
    let lines: Vec<String> = vec![
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    let blocks = classify_blocks(&lines);
    assert_eq!(blocks.len(), 1);
    assert!(matches!(&blocks[0], BlockElement::Table { .. }));
}

#[test]
fn test_classify_mixed() {
    let lines: Vec<String> = vec![
        "Hello".to_string(),
        "```".to_string(),
        "code".to_string(),
        "```".to_string(),
        "World".to_string(),
    ];
    let blocks = classify_blocks(&lines);
    assert_eq!(blocks.len(), 3); // Line, CodeBlock, Line
}

#[test]
fn test_unclosed_code_block() {
    let lines: Vec<String> = vec![
        "```rust".to_string(),
        "let x = 1;".to_string(),
        // No closing fence.
    ];
    let blocks = classify_blocks(&lines);
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        BlockElement::CodeBlock {
            fence_close_idx, ..
        } => {
            assert!(fence_close_idx.is_none());
        }
        _ => panic!("Expected CodeBlock"),
    }
}

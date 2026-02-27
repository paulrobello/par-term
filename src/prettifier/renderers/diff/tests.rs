//! Tests for the diff renderer.

use super::config::{DiffRendererConfig, DiffStyle};
use super::diff_parser::parse_hunk_header;
use super::diff_word::split_into_words;
use super::renderer::{DiffRenderer, register_diff_renderer};
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::traits::{ContentRenderer, RendererConfig, ThemeColors};
use crate::prettifier::types::{ContentBlock, RendererCapability, StyledLine};
use std::time::SystemTime;

fn test_config() -> RendererConfig {
    RendererConfig {
        terminal_width: 80,
        ..Default::default()
    }
}

fn wide_config() -> RendererConfig {
    RendererConfig {
        terminal_width: 200,
        ..Default::default()
    }
}

fn renderer() -> DiffRenderer {
    DiffRenderer::new(DiffRendererConfig::default())
}

fn inline_renderer() -> DiffRenderer {
    DiffRenderer::new(DiffRendererConfig {
        style: DiffStyle::Inline,
        ..Default::default()
    })
}

fn make_block(lines: &[&str]) -> ContentBlock {
    ContentBlock {
        lines: lines.iter().map(|s| s.to_string()).collect(),
        preceding_command: None,
        start_row: 0,
        end_row: lines.len(),
        timestamp: SystemTime::now(),
    }
}

fn all_text(lines: &[StyledLine]) -> String {
    lines
        .iter()
        .map(|l| {
            l.segments
                .iter()
                .map(|s| s.text.as_str())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// -- Trait methods --

#[test]
fn test_format_id() {
    let r = renderer();
    assert_eq!(r.format_id(), "diff");
    assert_eq!(r.display_name(), "Diff");
    assert_eq!(r.format_badge(), "DIFF");
    assert_eq!(r.capabilities(), vec![RendererCapability::TextStyling]);
}

// -- Parsing --

#[test]
fn test_parse_git_diff() {
    let lines: Vec<String> = vec![
        "diff --git a/src/main.rs b/src/main.rs",
        "index abc1234..def5678 100644",
        "--- a/src/main.rs",
        "+++ b/src/main.rs",
        "@@ -1,3 +1,4 @@",
        " line1",
        "+added",
        " line2",
        " line3",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    use super::diff_parser::parse_unified_diff;
    let files = parse_unified_diff(&lines);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].old_path, "a/src/main.rs");
    assert_eq!(files[0].new_path, "b/src/main.rs");
    assert_eq!(files[0].hunks.len(), 1);
    assert_eq!(files[0].hunks[0].old_start, 1);
    assert_eq!(files[0].hunks[0].old_count, 3);
    assert_eq!(files[0].hunks[0].new_start, 1);
    assert_eq!(files[0].hunks[0].new_count, 4);
    assert_eq!(files[0].hunks[0].lines.len(), 4);
}

#[test]
fn test_parse_multiple_files() {
    let lines: Vec<String> = vec![
        "diff --git a/file1.rs b/file1.rs",
        "--- a/file1.rs",
        "+++ b/file1.rs",
        "@@ -1,2 +1,2 @@",
        "-old1",
        "+new1",
        "diff --git a/file2.rs b/file2.rs",
        "--- a/file2.rs",
        "+++ b/file2.rs",
        "@@ -1,2 +1,2 @@",
        "-old2",
        "+new2",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    use super::diff_parser::parse_unified_diff;
    let files = parse_unified_diff(&lines);
    assert_eq!(files.len(), 2);
}

#[test]
fn test_parse_multiple_hunks() {
    let lines: Vec<String> = vec![
        "diff --git a/file.rs b/file.rs",
        "--- a/file.rs",
        "+++ b/file.rs",
        "@@ -1,3 +1,3 @@",
        " context",
        "-old",
        "+new",
        "@@ -10,3 +10,3 @@",
        " another",
        "-old2",
        "+new2",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    use super::diff_parser::parse_unified_diff;
    let files = parse_unified_diff(&lines);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].hunks.len(), 2);
    assert_eq!(files[0].hunks[0].old_start, 1);
    assert_eq!(files[0].hunks[1].old_start, 10);
}

#[test]
fn test_parse_non_git_diff() {
    let lines: Vec<String> = vec![
        "--- file.txt.orig",
        "+++ file.txt",
        "@@ -1,3 +1,3 @@",
        " line1",
        "-old",
        "+new",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    use super::diff_parser::parse_unified_diff;
    let files = parse_unified_diff(&lines);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].old_path, "file.txt.orig");
    assert_eq!(files[0].new_path, "file.txt");
}

// -- Line coloring --

#[test]
fn test_added_lines_green() {
    use crate::prettifier::traits::ContentRenderer;
    let r = inline_renderer();
    let block = make_block(&[
        "diff --git a/f b/f",
        "--- a/f",
        "+++ b/f",
        "@@ -1,2 +1,3 @@",
        " ctx",
        "+added line",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let added_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("+added"))
        .unwrap();
    assert_eq!(added_seg.fg, Some(theme.palette[2])); // Green
}

#[test]
fn test_removed_lines_red() {
    use crate::prettifier::traits::ContentRenderer;
    let r = DiffRenderer::new(DiffRendererConfig {
        style: DiffStyle::Inline,
        word_diff: false,
        ..Default::default()
    });
    let block = make_block(&[
        "diff --git a/f b/f",
        "--- a/f",
        "+++ b/f",
        "@@ -1,2 +1,1 @@",
        "-removed line",
        " ctx",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let removed_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("-removed"))
        .unwrap();
    assert_eq!(removed_seg.fg, Some(theme.palette[1])); // Red
}

#[test]
fn test_file_headers_bold() {
    use crate::prettifier::traits::ContentRenderer;
    let r = inline_renderer();
    let block = make_block(&[
        "diff --git a/f b/f",
        "--- a/f",
        "+++ b/f",
        "@@ -1,1 +1,1 @@",
        "-old",
        "+new",
    ]);
    let result = r.render(&block, &test_config()).unwrap();

    let old_header = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.starts_with("--- "))
        .unwrap();
    assert!(old_header.bold);

    let new_header = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.starts_with("+++ "))
        .unwrap();
    assert!(new_header.bold);
}

#[test]
fn test_hunk_headers_cyan() {
    use crate::prettifier::traits::ContentRenderer;
    let r = inline_renderer();
    let block = make_block(&[
        "diff --git a/f b/f",
        "--- a/f",
        "+++ b/f",
        "@@ -1,3 +1,3 @@ fn main()",
        " context",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let hunk = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("@@"))
        .unwrap();
    assert_eq!(hunk.fg, Some(theme.palette[6])); // Cyan
}

#[test]
fn test_context_lines_default_color() {
    use crate::prettifier::traits::ContentRenderer;
    let r = inline_renderer();
    let block = make_block(&[
        "diff --git a/f b/f",
        "--- a/f",
        "+++ b/f",
        "@@ -1,3 +1,3 @@",
        " context line",
    ]);
    let result = r.render(&block, &test_config()).unwrap();

    let ctx = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("context line"))
        .unwrap();
    assert!(ctx.fg.is_none()); // Default foreground
}

// -- Word-level diff --

#[test]
fn test_word_diff_highlighting() {
    use crate::prettifier::traits::ContentRenderer;
    let r = inline_renderer();
    let block = make_block(&[
        "diff --git a/f b/f",
        "--- a/f",
        "+++ b/f",
        "@@ -1,1 +1,1 @@",
        "-the old word here",
        "+the new word here",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("old"));
    assert!(text.contains("new"));
}

#[test]
fn test_word_diff_disabled() {
    use crate::prettifier::traits::ContentRenderer;
    let r = DiffRenderer::new(DiffRendererConfig {
        style: DiffStyle::Inline,
        word_diff: false,
        ..Default::default()
    });
    let block = make_block(&[
        "diff --git a/f b/f",
        "--- a/f",
        "+++ b/f",
        "@@ -1,1 +1,1 @@",
        "-old line",
        "+new line",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    // Should still render, just without word-level highlighting
    let text = all_text(&result.lines);
    assert!(text.contains("-old line"));
    assert!(text.contains("+new line"));
}

// -- Line numbers --

#[test]
fn test_line_numbers_shown() {
    use crate::prettifier::traits::ContentRenderer;
    let r = inline_renderer();
    let block = make_block(&[
        "diff --git a/f b/f",
        "--- a/f",
        "+++ b/f",
        "@@ -10,3 +10,3 @@",
        " context",
        "-old",
        "+new",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("10"));
}

#[test]
fn test_line_numbers_hidden() {
    use crate::prettifier::traits::ContentRenderer;
    let r = DiffRenderer::new(DiffRendererConfig {
        style: DiffStyle::Inline,
        show_line_numbers: false,
        ..Default::default()
    });
    let block = make_block(&[
        "diff --git a/f b/f",
        "--- a/f",
        "+++ b/f",
        "@@ -10,3 +10,3 @@",
        " context",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    // Check that there is no gutter segment (the first segment should not be a line number)
    let first_ctx_line = result
        .lines
        .iter()
        .find(|l| l.segments.iter().any(|s| s.text.contains("context")));
    assert!(first_ctx_line.is_some());
    // With show_line_numbers off, first segment should be the content itself
    let segments = &first_ctx_line.unwrap().segments;
    assert!(segments[0].text.contains("context"));
}

// -- Side-by-side mode --

#[test]
fn test_auto_style_inline_narrow() {
    let r = renderer();
    assert!(!r.use_side_by_side(80));
}

#[test]
fn test_auto_style_side_by_side_wide() {
    let r = renderer();
    assert!(r.use_side_by_side(200));
}

#[test]
fn test_forced_inline() {
    let r = DiffRenderer::new(DiffRendererConfig {
        style: DiffStyle::Inline,
        ..Default::default()
    });
    assert!(!r.use_side_by_side(200));
}

#[test]
fn test_forced_side_by_side() {
    let r = DiffRenderer::new(DiffRendererConfig {
        style: DiffStyle::SideBySide,
        ..Default::default()
    });
    assert!(r.use_side_by_side(80));
}

#[test]
fn test_side_by_side_render() {
    use crate::prettifier::traits::ContentRenderer;
    let r = DiffRenderer::new(DiffRendererConfig {
        style: DiffStyle::SideBySide,
        ..Default::default()
    });
    let block = make_block(&[
        "diff --git a/f b/f",
        "--- a/f",
        "+++ b/f",
        "@@ -1,3 +1,3 @@",
        " context",
        "-old line",
        "+new line",
    ]);
    let result = r.render(&block, &wide_config()).unwrap();
    let text = all_text(&result.lines);
    // Side-by-side should have the divider
    assert!(text.contains(" | "));
}

// -- Error cases --

#[test]
fn test_empty_diff_error() {
    use crate::prettifier::traits::ContentRenderer;
    let r = renderer();
    let block = make_block(&["not a diff at all"]);
    let result = r.render(&block, &test_config());
    assert!(result.is_err());
}

// -- Line mappings --

#[test]
fn test_line_mappings_populated() {
    use crate::prettifier::traits::ContentRenderer;
    let r = inline_renderer();
    let block = make_block(&[
        "diff --git a/f b/f",
        "--- a/f",
        "+++ b/f",
        "@@ -1,2 +1,2 @@",
        "-old",
        "+new",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    assert_eq!(result.line_mapping.len(), result.lines.len());
}

// -- Registration --

#[test]
fn test_register_diff_renderer() {
    let mut registry = RendererRegistry::new(0.6);
    register_diff_renderer(&mut registry, &DiffRendererConfig::default());
    assert_eq!(registry.renderer_count(), 1);
    assert!(registry.get_renderer("diff").is_some());
    assert_eq!(
        registry.get_renderer("diff").unwrap().display_name(),
        "Diff"
    );
}

// -- Config defaults --

#[test]
fn test_config_defaults() {
    let config = DiffRendererConfig::default();
    assert_eq!(config.style, DiffStyle::Auto);
    assert_eq!(config.side_by_side_min_width, 160);
    assert!(config.word_diff);
    assert!(config.show_line_numbers);
}

// -- Hunk header parsing --

#[test]
fn test_hunk_header_parsing() {
    let (old_s, old_c, new_s, new_c, text) = parse_hunk_header("@@ -10,5 +20,7 @@ fn main()");
    assert_eq!(old_s, 10);
    assert_eq!(old_c, 5);
    assert_eq!(new_s, 20);
    assert_eq!(new_c, 7);
    assert_eq!(text, "fn main()");
}

#[test]
fn test_hunk_header_no_count() {
    let (old_s, old_c, new_s, new_c, _) = parse_hunk_header("@@ -1 +1 @@");
    assert_eq!(old_s, 1);
    assert_eq!(old_c, 1);
    assert_eq!(new_s, 1);
    assert_eq!(new_c, 1);
}

// -- Word splitting --

#[test]
fn test_split_into_words() {
    let words = split_into_words("hello world");
    assert_eq!(words, vec!["hello", " ", "world"]);
}

#[test]
fn test_split_into_words_punctuation() {
    let words = split_into_words("fn(a, b)");
    assert_eq!(words, vec!["fn", "(", "a", ",", " ", "b", ")"]);
}

// -- Format badge --

#[test]
fn test_format_badge() {
    use crate::prettifier::traits::ContentRenderer;
    let r = renderer();
    let block = make_block(&[
        "diff --git a/f b/f",
        "--- a/f",
        "+++ b/f",
        "@@ -1,1 +1,1 @@",
        "-old",
        "+new",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    assert_eq!(result.format_badge, "DIFF");
}

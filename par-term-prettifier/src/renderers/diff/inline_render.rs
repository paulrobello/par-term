//! Inline (unified) diff rendering for DiffRenderer.
//!
//! Contains `render_inline` and `render_hunk_inline` — the word-diff-aware
//! rendering pass used when side-by-side mode is not active.

use super::super::push_line;
use super::config::{DiffLineState, DiffRendererConfig};
use super::diff_parser::{DiffFile, DiffHunk, DiffLine};
use super::diff_word::word_diff_segments;
use super::helpers::gutter_segment;
use crate::traits::ThemeColors;
use crate::types::{SourceLineMapping, StyledLine, StyledSegment};

/// Render a full diff in inline mode (all files and hunks).
pub(super) fn render_inline(
    config: &DiffRendererConfig,
    files: &[DiffFile],
    lines: &mut Vec<StyledLine>,
    line_mapping: &mut Vec<SourceLineMapping>,
    theme: &ThemeColors,
) {
    for file in files {
        // Render file diff header
        if !file.header_lines.is_empty() {
            for header_line in &file.header_lines {
                push_line(
                    lines,
                    line_mapping,
                    vec![StyledSegment {
                        text: header_line.clone(),
                        fg: Some(theme.palette[15]), // Bright white
                        bold: true,
                        ..Default::default()
                    }],
                    None,
                );
            }
        }

        // Render --- / +++ file headers
        if !file.old_path.is_empty() {
            push_line(
                lines,
                line_mapping,
                vec![StyledSegment {
                    text: format!("--- {}", file.old_path),
                    fg: Some(theme.palette[1]), // Red
                    bold: true,
                    ..Default::default()
                }],
                None,
            );
            push_line(
                lines,
                line_mapping,
                vec![StyledSegment {
                    text: format!("+++ {}", file.new_path),
                    fg: Some(theme.palette[2]), // Green
                    bold: true,
                    ..Default::default()
                }],
                None,
            );
        }

        for hunk in &file.hunks {
            render_hunk_inline(config, hunk, lines, line_mapping, theme);
        }
    }
}

/// Render a single hunk in inline mode.
pub(super) fn render_hunk_inline(
    config: &DiffRendererConfig,
    hunk: &DiffHunk,
    lines: &mut Vec<StyledLine>,
    line_mapping: &mut Vec<SourceLineMapping>,
    theme: &ThemeColors,
) {
    // Hunk header
    let hunk_header = format!(
        "@@ -{},{} +{},{} @@{}",
        hunk.old_start,
        hunk.old_count,
        hunk.new_start,
        hunk.new_count,
        if hunk.header_text.is_empty() {
            String::new()
        } else {
            format!(" {}", hunk.header_text)
        }
    );
    push_line(
        lines,
        line_mapping,
        vec![StyledSegment {
            text: hunk_header,
            fg: Some(theme.palette[6]), // Cyan
            ..Default::default()
        }],
        None,
    );

    let mut state = DiffLineState {
        old_line: hunk.old_start,
        new_line: hunk.new_start,
    };

    // Collect lines for word-diff pairing
    let hunk_lines = &hunk.lines;
    let mut i = 0;

    while i < hunk_lines.len() {
        match &hunk_lines[i] {
            DiffLine::Context(text) => {
                let mut segments = Vec::new();
                if config.show_line_numbers {
                    segments.push(gutter_segment(
                        Some(state.old_line),
                        Some(state.new_line),
                        theme,
                    ));
                }
                segments.push(StyledSegment {
                    text: format!(" {text}"),
                    ..Default::default()
                });
                push_line(lines, line_mapping, segments, None);
                state.old_line += 1;
                state.new_line += 1;
                i += 1;
            }
            DiffLine::Removed(removed_text) => {
                // Check if this is a paired remove/add for word-level diff
                if config.word_diff {
                    // Collect consecutive removed lines
                    let remove_start = i;
                    let mut remove_end = i;
                    while remove_end < hunk_lines.len() {
                        if matches!(&hunk_lines[remove_end], DiffLine::Removed(_)) {
                            remove_end += 1;
                        } else {
                            break;
                        }
                    }
                    // Collect consecutive added lines
                    let add_start = remove_end;
                    let mut add_end = remove_end;
                    while add_end < hunk_lines.len() {
                        if matches!(&hunk_lines[add_end], DiffLine::Added(_)) {
                            add_end += 1;
                        } else {
                            break;
                        }
                    }

                    let removed_count = remove_end - remove_start;
                    let added_count = add_end - add_start;

                    if added_count > 0 && removed_count > 0 {
                        // Pair up remove/add lines for word-level diff
                        let pair_count = removed_count.min(added_count);

                        for j in 0..removed_count {
                            if let DiffLine::Removed(r_text) = &hunk_lines[remove_start + j] {
                                let mut segments = Vec::new();
                                if config.show_line_numbers {
                                    segments.push(gutter_segment(
                                        Some(state.old_line),
                                        None,
                                        theme,
                                    ));
                                }
                                segments.push(StyledSegment {
                                    text: "-".to_string(),
                                    fg: Some(theme.palette[1]),
                                    ..Default::default()
                                });
                                if j < pair_count {
                                    if let DiffLine::Added(a_text) = &hunk_lines[add_start + j] {
                                        segments.extend(word_diff_segments(
                                            r_text,
                                            a_text,
                                            theme.palette[1],
                                            [100, 0, 0],
                                        ));
                                    }
                                } else {
                                    segments.push(StyledSegment {
                                        text: r_text.clone(),
                                        fg: Some(theme.palette[1]),
                                        ..Default::default()
                                    });
                                }
                                push_line(lines, line_mapping, segments, None);
                                state.old_line += 1;
                            }
                        }
                        for j in 0..added_count {
                            if let DiffLine::Added(a_text) = &hunk_lines[add_start + j] {
                                let mut segments = Vec::new();
                                if config.show_line_numbers {
                                    segments.push(gutter_segment(
                                        None,
                                        Some(state.new_line),
                                        theme,
                                    ));
                                }
                                segments.push(StyledSegment {
                                    text: "+".to_string(),
                                    fg: Some(theme.palette[2]),
                                    ..Default::default()
                                });
                                if j < pair_count {
                                    if let DiffLine::Removed(r_text) = &hunk_lines[remove_start + j]
                                    {
                                        segments.extend(word_diff_segments(
                                            a_text,
                                            r_text,
                                            theme.palette[2],
                                            [0, 80, 0],
                                        ));
                                    }
                                } else {
                                    segments.push(StyledSegment {
                                        text: a_text.clone(),
                                        fg: Some(theme.palette[2]),
                                        ..Default::default()
                                    });
                                }
                                push_line(lines, line_mapping, segments, None);
                                state.new_line += 1;
                            }
                        }
                        i = add_end;
                    } else {
                        // No paired add — plain removed line
                        let mut segments = Vec::new();
                        if config.show_line_numbers {
                            segments.push(gutter_segment(Some(state.old_line), None, theme));
                        }
                        segments.push(StyledSegment {
                            text: format!("-{removed_text}"),
                            fg: Some(theme.palette[1]), // Red
                            ..Default::default()
                        });
                        push_line(lines, line_mapping, segments, None);
                        state.old_line += 1;
                        i += 1;
                    }
                } else {
                    // No word diff — plain removed line
                    let mut segments = Vec::new();
                    if config.show_line_numbers {
                        segments.push(gutter_segment(Some(state.old_line), None, theme));
                    }
                    segments.push(StyledSegment {
                        text: format!("-{removed_text}"),
                        fg: Some(theme.palette[1]), // Red
                        ..Default::default()
                    });
                    push_line(lines, line_mapping, segments, None);
                    state.old_line += 1;
                    i += 1;
                }
            }
            DiffLine::Added(added_text) => {
                // Standalone added line (not paired with a removed line)
                let mut segments = Vec::new();
                if config.show_line_numbers {
                    segments.push(gutter_segment(None, Some(state.new_line), theme));
                }
                segments.push(StyledSegment {
                    text: format!("+{added_text}"),
                    fg: Some(theme.palette[2]), // Green
                    ..Default::default()
                });
                push_line(lines, line_mapping, segments, None);
                state.new_line += 1;
                i += 1;
            }
        }
    }
}

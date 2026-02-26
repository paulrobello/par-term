//! Inline span extraction for the Markdown renderer.
//!
//! Defines [`InlineSpan`], [`SpanKind`], and the compiled regex accessors
//! used for inline markup (bold, italic, code, links).  The main export is
//! [`extract_inline_spans`], which performs a multi-pass greedy parse of a
//! text string and returns non-overlapping spans sorted by start position.

use regex::Regex;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Compiled inline regexes
// ---------------------------------------------------------------------------

pub(super) fn re_inline_code() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"`([^`]+)`")
            .expect("re_inline_code: pattern is valid and should always compile")
    })
}

pub(super) fn re_link() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\[([^\]]+)\]\(([^)]+)\)")
            .expect("re_link: pattern is valid and should always compile")
    })
}

pub(super) fn re_bold_italic() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\*\*\*(.+?)\*\*\*|___(.+?)___")
            .expect("re_bold_italic: pattern is valid and should always compile")
    })
}

pub(super) fn re_bold() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\*\*(.+?)\*\*|__(.+?)__")
            .expect("re_bold: pattern is valid and should always compile")
    })
}

pub(super) fn re_italic() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Use \b around underscore italic to avoid matching snake_case identifiers.
    RE.get_or_init(|| {
        Regex::new(r"\*([^*]+)\*|\b_([^_]+)_\b")
            .expect("re_italic: pattern is valid and should always compile")
    })
}

// ---------------------------------------------------------------------------
// Inline span types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(super) struct InlineSpan {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) kind: SpanKind,
}

#[derive(Debug, Clone)]
pub(super) enum SpanKind {
    Code(String),
    Link { text: String, url: String },
    BoldItalic(String),
    Bold(String),
    Italic(String),
}

// ---------------------------------------------------------------------------
// Occupancy helpers
// ---------------------------------------------------------------------------

pub(super) fn any_occupied(occupied: &[bool], start: usize, end: usize) -> bool {
    occupied[start..end].iter().any(|&b| b)
}

pub(super) fn mark_occupied(occupied: &mut [bool], start: usize, end: usize) {
    for b in &mut occupied[start..end] {
        *b = true;
    }
}

pub(super) fn find_in_unoccupied(
    text: &str,
    re: &Regex,
    occupied: &[bool],
) -> Vec<(usize, usize)> {
    let mut results = Vec::new();
    let mut pos = 0;
    while pos < text.len() {
        if occupied[pos] {
            pos += 1;
            continue;
        }
        if let Some(m) = re.find_at(text, pos) {
            if !any_occupied(occupied, m.start(), m.end()) {
                results.push((m.start(), m.end()));
                pos = m.end();
            } else {
                pos = m.start() + 1;
            }
        } else {
            break;
        }
    }
    results
}

// ---------------------------------------------------------------------------
// Main extraction function
// ---------------------------------------------------------------------------

/// Extract all inline markup spans from `text`, returning them sorted by
/// start position.  Each byte range in the returned spans is non-overlapping;
/// code spans have the highest priority and prevent other markup from matching
/// within them.
pub(super) fn extract_inline_spans(text: &str) -> Vec<InlineSpan> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut occupied = vec![false; text.len()];
    let mut spans = Vec::new();

    // Pass 1: Code spans (highest priority, opaque)
    for (start, end) in find_in_unoccupied(text, re_inline_code(), &occupied) {
        let caps = re_inline_code()
            .captures(&text[start..])
            .expect("re_inline_code matched a range that should always capture group 1");
        let content = caps
            .get(1)
            .expect("re_inline_code capture group 1 must be present after a match")
            .as_str()
            .to_string();
        mark_occupied(&mut occupied, start, end);
        spans.push(InlineSpan {
            start,
            end,
            kind: SpanKind::Code(content),
        });
    }

    // Pass 2: Links
    for (start, end) in find_in_unoccupied(text, re_link(), &occupied) {
        let caps = re_link()
            .captures(&text[start..])
            .expect("re_link matched a range that should always capture groups 1 and 2");
        let link_text = caps
            .get(1)
            .expect("re_link capture group 1 (link text) must be present after a match")
            .as_str()
            .to_string();
        let url = caps
            .get(2)
            .expect("re_link capture group 2 (url) must be present after a match")
            .as_str()
            .to_string();
        mark_occupied(&mut occupied, start, end);
        spans.push(InlineSpan {
            start,
            end,
            kind: SpanKind::Link {
                text: link_text,
                url,
            },
        });
    }

    // Pass 3: Bold+italic
    for (start, end) in find_in_unoccupied(text, re_bold_italic(), &occupied) {
        let caps = re_bold_italic()
            .captures(&text[start..])
            .expect("re_bold_italic matched a range that should always capture group 1 or 2");
        let content = caps
            .get(1)
            .or_else(|| caps.get(2))
            .expect("re_bold_italic must capture group 1 or 2 after a match")
            .as_str()
            .to_string();
        mark_occupied(&mut occupied, start, end);
        spans.push(InlineSpan {
            start,
            end,
            kind: SpanKind::BoldItalic(content),
        });
    }

    // Pass 4: Bold
    for (start, end) in find_in_unoccupied(text, re_bold(), &occupied) {
        let caps = re_bold()
            .captures(&text[start..])
            .expect("re_bold matched a range that should always capture group 1 or 2");
        let content = caps
            .get(1)
            .or_else(|| caps.get(2))
            .expect("re_bold must capture group 1 or 2 after a match")
            .as_str()
            .to_string();
        mark_occupied(&mut occupied, start, end);
        spans.push(InlineSpan {
            start,
            end,
            kind: SpanKind::Bold(content),
        });
    }

    // Pass 5: Italic
    for (start, end) in find_in_unoccupied(text, re_italic(), &occupied) {
        let caps = re_italic()
            .captures(&text[start..])
            .expect("re_italic matched a range that should always capture group 1 or 2");
        let content = caps
            .get(1)
            .or_else(|| caps.get(2))
            .expect("re_italic must capture group 1 or 2 after a match")
            .as_str()
            .to_string();
        mark_occupied(&mut occupied, start, end);
        spans.push(InlineSpan {
            start,
            end,
            kind: SpanKind::Italic(content),
        });
    }

    spans.sort_by_key(|s| s.start);
    spans
}

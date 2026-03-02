//! Compiled block-level regular expressions used during line rendering.
//!
//! Each accessor uses a `OnceLock` to compile the pattern at most once.

use regex::Regex;
use std::sync::OnceLock;

pub(super) fn re_header() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(#{1,6})\s+(.*)$")
            .expect("re_header: pattern is valid and should always compile")
    })
}

pub(super) fn re_blockquote() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^>\s?(.*)$")
            .expect("re_blockquote: pattern is valid and should always compile")
    })
}

pub(super) fn re_unordered_list() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(\s*)([-*+])\s+(.*)$")
            .expect("re_unordered_list: pattern is valid and should always compile")
    })
}

pub(super) fn re_ordered_list() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(\s*)(\d+[.)])\s+(.*)$")
            .expect("re_ordered_list: pattern is valid and should always compile")
    })
}

pub(super) fn re_horizontal_rule() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(?:-[\s-]*-[\s-]*-[\s-]*|\*[\s*]*\*[\s*]*\*[\s*]*|_[\s_]*_[\s_]*_[\s_]*)$")
            .expect("re_horizontal_rule: pattern is valid and should always compile")
    })
}

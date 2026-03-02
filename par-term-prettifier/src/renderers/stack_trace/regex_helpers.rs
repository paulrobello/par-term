//! Compiled regex patterns for stack trace parsing.

use std::sync::OnceLock;

use regex::Regex;

pub(super) fn re_java_frame() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\s+at\s+([\w.$]+)\(([\w.]+):(\d+)\)")
            .expect("re_java_frame: pattern is valid and should always compile")
    })
}

pub(super) fn re_python_frame() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"^\s+File "([^"]+)", line (\d+)"#)
            .expect("re_python_frame: pattern is valid and should always compile")
    })
}

pub(super) fn re_js_frame() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\s+at\s+\S+\s+\((.+):(\d+):(\d+)\)")
            .expect("re_js_frame: pattern is valid and should always compile")
    })
}

pub(super) fn re_rust_location() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"([\w/\\.-]+\.rs):(\d+)")
            .expect("re_rust_location: pattern is valid and should always compile")
    })
}

pub(super) fn re_go_location() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"([\w/\\.-]+\.go):(\d+)")
            .expect("re_go_location: pattern is valid and should always compile")
    })
}

pub(super) fn re_error_header() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^([\w.]+(?:Error|Exception|Panic)):?\s")
            .expect("re_error_header: pattern is valid and should always compile")
    })
}

pub(super) fn re_caused_by() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^Caused by:").expect("regex pattern is valid and should always compile")
    })
}

pub(super) fn re_python_traceback_header() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^Traceback \(most recent call last\):")
            .expect("re_python_traceback_header: pattern is valid and should always compile")
    })
}

pub(super) fn re_rust_panic() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^thread '.*' panicked at")
            .expect("regex pattern is valid and should always compile")
    })
}

pub(super) fn re_go_panic() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^goroutine \d+ \[").expect("regex pattern is valid and should always compile")
    })
}

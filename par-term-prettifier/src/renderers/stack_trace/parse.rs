//! Stack trace line parsing: frame classification and file path extraction.

use super::regex_helpers::{
    re_caused_by, re_error_header, re_go_location, re_go_panic, re_java_frame, re_js_frame,
    re_python_frame, re_python_traceback_header, re_rust_location, re_rust_panic,
};
use super::types::{FilePath, FrameType, TraceLine};

/// Classify a frame as application or framework based on app package prefixes.
pub(super) fn classify_frame(frame_text: &str, app_packages: &[String]) -> FrameType {
    if app_packages.is_empty() {
        // If no app packages configured, consider indented "at" frames as framework
        // and other frames as application by default
        return FrameType::Application;
    }
    if app_packages
        .iter()
        .any(|pkg| frame_text.contains(pkg.as_str()))
    {
        FrameType::Application
    } else {
        FrameType::Framework
    }
}

/// Extract a file path with line number from a stack frame.
pub(super) fn extract_file_path(line: &str) -> Option<FilePath> {
    // Java: at package.Class(FileName.java:42)
    if let Some(caps) = re_java_frame().captures(line) {
        return Some(FilePath {
            path: caps
                .get(2)
                .expect("re_java_frame capture group 2 (filename) must be present after a match")
                .as_str()
                .to_string(),
            line: caps.get(3).and_then(|m| m.as_str().parse().ok()),
            column: None,
        });
    }

    // Python: File "path/to/file.py", line 42
    if let Some(caps) = re_python_frame().captures(line) {
        return Some(FilePath {
            path: caps
                .get(1)
                .expect("re_python_frame capture group 1 (file path) must be present after a match")
                .as_str()
                .to_string(),
            line: caps.get(2).and_then(|m| m.as_str().parse().ok()),
            column: None,
        });
    }

    // JavaScript/Node.js: at Function (file.js:42:10)
    if let Some(caps) = re_js_frame().captures(line) {
        return Some(FilePath {
            path: caps
                .get(1)
                .expect("re_js_frame capture group 1 (file path) must be present after a match")
                .as_str()
                .to_string(),
            line: caps.get(2).and_then(|m| m.as_str().parse().ok()),
            column: caps.get(3).and_then(|m| m.as_str().parse().ok()),
        });
    }

    // Rust: src/main.rs:42
    if let Some(caps) = re_rust_location().captures(line) {
        return Some(FilePath {
            path: caps
                .get(1)
                .expect(
                    "re_rust_location capture group 1 (file path) must be present after a match",
                )
                .as_str()
                .to_string(),
            line: caps.get(2).and_then(|m| m.as_str().parse().ok()),
            column: None,
        });
    }

    // Go: /home/user/app/main.go:42
    if let Some(caps) = re_go_location().captures(line) {
        return Some(FilePath {
            path: caps
                .get(1)
                .expect("re_go_location capture group 1 (file path) must be present after a match")
                .as_str()
                .to_string(),
            line: caps.get(2).and_then(|m| m.as_str().parse().ok()),
            column: None,
        });
    }

    None
}

/// Parse a line of a stack trace into a classified TraceLine.
pub(super) fn parse_trace_line(line: &str, app_packages: &[String]) -> TraceLine {
    // Check for Caused by:
    if re_caused_by().is_match(line) {
        return TraceLine::CausedBy(line.to_string());
    }

    // Check for error headers
    if re_error_header().is_match(line)
        || re_python_traceback_header().is_match(line)
        || re_rust_panic().is_match(line)
        || re_go_panic().is_match(line)
    {
        return TraceLine::ErrorHeader(line.to_string());
    }

    // Check for stack frames (indented lines with frame patterns)
    let file_path = extract_file_path(line);
    let is_frame = file_path.is_some()
        || line.trim_start().starts_with("at ")
        || (line.starts_with(' ') || line.starts_with('\t'));

    if is_frame {
        let frame_type = classify_frame(line, app_packages);
        return TraceLine::Frame {
            text: line.to_string(),
            frame_type,
            file_path,
        };
    }

    TraceLine::Other(line.to_string())
}

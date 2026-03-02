//! Internal types for stack trace classification.

/// Classification of a stack frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrameType {
    /// User application code — rendered with bright/normal colors.
    Application,
    /// Framework or library code — rendered dimmed.
    Framework,
}

/// A file path with line number extracted from a stack frame.
#[derive(Debug, Clone)]
pub(super) struct FilePath {
    /// The file path as it appeared in the frame.
    pub(super) path: String,
    /// The line number (if found).
    pub(super) line: Option<usize>,
    /// Optional column number.
    pub(super) column: Option<usize>,
}

/// Classification of a line in a stack trace.
#[derive(Debug)]
pub(super) enum TraceLine {
    /// Error/exception header (e.g., "java.lang.NullPointerException: message").
    ErrorHeader(String),
    /// "Caused by:" chain header.
    CausedBy(String),
    /// A stack frame line.
    Frame {
        text: String,
        frame_type: FrameType,
        file_path: Option<FilePath>,
    },
    /// Other text (context, notes, etc.).
    Other(String),
}

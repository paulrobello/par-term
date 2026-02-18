//! Terminal manager for par-term terminal emulator.
//!
//! This crate provides the `TerminalManager` which wraps the core PTY session
//! and provides a high-level API for terminal operations including:
//!
//! - PTY I/O (read/write/paste)
//! - Terminal lifecycle (spawn, resize, kill)
//! - Shell integration (CWD, exit codes, command tracking)
//! - Clipboard management
//! - Inline graphics (Sixel, iTerm2, Kitty)
//! - Search functionality
//! - Scrollback metadata and prompt marks
//! - Recording and screenshots
//! - Coprocess management
//! - tmux control mode

pub mod scrollback_metadata;
pub mod styled_content;
pub mod terminal;

// Re-export main types for convenience
pub use scrollback_metadata::{CommandSnapshot, LineMetadata, ScrollbackMark, ScrollbackMetadata};
pub use styled_content::{StyledSegment, extract_styled_segments, segments_to_plain_text};
pub use terminal::TerminalManager;
pub use terminal::coprocess_env;

// Re-export types from core that are part of our public API
pub use par_term_emu_core_rust::terminal::{ClipboardEntry, ClipboardSlot, HyperlinkInfo};

// Re-export Cell from config crate
pub use par_term_config::Cell;

/// A single search match in the terminal scrollback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchMatch {
    /// Line index in scrollback (0 = oldest line)
    pub line: usize,
    /// Column position in the line (0-indexed)
    pub column: usize,
    /// Length of the match in characters
    pub length: usize,
}

impl SearchMatch {
    /// Create a new search match.
    pub fn new(line: usize, column: usize, length: usize) -> Self {
        Self {
            line,
            column,
            length,
        }
    }
}

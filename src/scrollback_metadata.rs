//! Scrollback metadata for shell integration markers.
//!
//! This module re-exports types from the par-term-terminal crate for backward compatibility.

pub use par_term_terminal::scrollback_metadata::{
    CommandSnapshot, LineMetadata, ScrollbackMark, ScrollbackMetadata,
};

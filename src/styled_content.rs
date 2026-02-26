//! Styled content extraction from terminal grids.
//!
//! This module re-exports types from the par-term-terminal crate for backward compatibility.

pub use par_term_terminal::styled_content::{
    StyledSegment, extract_styled_segments, segments_to_plain_text,
};

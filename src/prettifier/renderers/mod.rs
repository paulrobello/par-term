//! Content renderers for detected formats.

use crate::prettifier::traits::ThemeColors;
use crate::prettifier::types::{SourceLineMapping, StyledLine, StyledSegment};

pub mod csv;
pub mod diagrams;
pub mod diff;
pub mod json;
pub mod log;
pub mod markdown;
pub mod sql_results;
pub mod stack_trace;
pub mod table;
pub mod toml;
pub mod tree_renderer;
pub mod xml;
pub mod yaml;

/// Push a styled line and its source mapping.
///
/// Shared helper used by multiple renderers to add a rendered line and
/// its corresponding source-line mapping in lockstep.
pub fn push_line(
    lines: &mut Vec<StyledLine>,
    line_mapping: &mut Vec<SourceLineMapping>,
    segments: Vec<StyledSegment>,
    source_line: Option<usize>,
) {
    line_mapping.push(SourceLineMapping {
        rendered_line: lines.len(),
        source_line,
    });
    lines.push(StyledLine::new(segments));
}

/// Create a styled segment for tree guide characters (dim grey).
///
/// Shared by the JSON, YAML, TOML, and XML renderers.
pub(super) fn guide_segment(prefix: &str, theme: &ThemeColors) -> StyledSegment {
    StyledSegment {
        text: prefix.to_string(),
        fg: Some(theme.dim_color()),
        ..Default::default()
    }
}

/// Create a plain (unstyled) segment.
///
/// Shared by the JSON, YAML, TOML, and XML renderers.
pub(super) fn plain_segment(text: &str) -> StyledSegment {
    StyledSegment {
        text: text.to_string(),
        ..Default::default()
    }
}

/// Create a dim-styled segment (same dim grey as guide characters).
///
/// Used for XML punctuation and other secondary syntax elements.
/// Shared by the XML renderer and any future renderer that needs dim text.
pub(super) fn dim_segment(text: &str, theme: &ThemeColors) -> StyledSegment {
    StyledSegment {
        text: text.to_string(),
        fg: Some(theme.dim_color()),
        ..Default::default()
    }
}

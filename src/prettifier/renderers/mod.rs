//! Content renderers for detected formats.

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

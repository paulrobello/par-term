//! Helper functions for gutter, line numbers, and string truncation.

use crate::prettifier::traits::ThemeColors;
use crate::prettifier::types::StyledSegment;

/// Create a line number gutter segment for inline mode.
pub(super) fn gutter_segment(
    old: Option<usize>,
    new: Option<usize>,
    theme: &ThemeColors,
) -> StyledSegment {
    let old_str = old
        .map(|n| format!("{n:>4}"))
        .unwrap_or_else(|| "    ".to_string());
    let new_str = new
        .map(|n| format!("{n:>4}"))
        .unwrap_or_else(|| "    ".to_string());
    StyledSegment {
        text: format!("{old_str} {new_str} "),
        fg: Some(theme.palette[8]), // Dim grey
        ..Default::default()
    }
}

/// Create a line number segment for side-by-side mode.
pub(super) fn line_num_segment(
    num: Option<usize>,
    width: usize,
    theme: &ThemeColors,
) -> StyledSegment {
    let text = num
        .map(|n| format!("{n:>width$} ", width = width - 1))
        .unwrap_or_else(|| format!("{:>width$} ", "", width = width - 1));
    StyledSegment {
        text,
        fg: Some(theme.palette[8]),
        ..Default::default()
    }
}

/// Truncate a string to fit within a given width.
pub(super) fn truncate_str(s: &str, max_width: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_width {
        s.to_string()
    } else if max_width > 1 {
        let truncated: String = s.chars().take(max_width - 1).collect();
        format!("{truncated}~")
    } else {
        "~".to_string()
    }
}

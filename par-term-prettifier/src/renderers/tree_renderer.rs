//! Shared tree rendering utilities for structured data formats.
//!
//! Provides tree guide line generation and collapsed-node summaries used by
//! JSON, YAML, TOML, and XML renderers to display hierarchical data with
//! visual indentation guides.

/// Tree guide characters used for indentation.
pub struct TreeGuideChars {
    /// Vertical continuation line: `│`
    pub vertical: &'static str,
    /// Spacer between guide columns.
    pub spacer: &'static str,
}

impl Default for TreeGuideChars {
    fn default() -> Self {
        Self {
            vertical: "│",
            spacer: "  ",
        }
    }
}

/// Generate tree guide prefix string for a given depth.
///
/// Each depth level produces a `│` followed by padding. For example, at depth 2:
/// ```text
/// │  │
/// ```
pub fn tree_guides(depth: usize) -> String {
    let chars = TreeGuideChars::default();
    let mut prefix = String::new();
    for _ in 0..depth {
        prefix.push_str(chars.vertical);
        prefix.push_str(chars.spacer);
    }
    prefix
}

/// Generate a collapsed summary for a container node.
///
/// Returns strings like `{ 3 keys }` or `[ 5 items ]`.
pub fn collapsed_summary(node_type: &str, count: usize) -> String {
    match node_type {
        "object" => format!("{{ {count} keys }}"),
        "array" => format!("[ {count} items ]"),
        _ => format!("({count} entries)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_guides_depth_0() {
        assert_eq!(tree_guides(0), "");
    }

    #[test]
    fn test_tree_guides_depth_1() {
        assert_eq!(tree_guides(1), "│  ");
    }

    #[test]
    fn test_tree_guides_depth_2() {
        assert_eq!(tree_guides(2), "│  │  ");
    }

    #[test]
    fn test_tree_guides_depth_3() {
        assert_eq!(tree_guides(3), "│  │  │  ");
    }

    #[test]
    fn test_collapsed_summary_object() {
        assert_eq!(collapsed_summary("object", 3), "{ 3 keys }");
    }

    #[test]
    fn test_collapsed_summary_array() {
        assert_eq!(collapsed_summary("array", 5), "[ 5 items ]");
    }

    #[test]
    fn test_collapsed_summary_unknown() {
        assert_eq!(collapsed_summary("other", 2), "(2 entries)");
    }

    #[test]
    fn test_collapsed_summary_zero_items() {
        assert_eq!(collapsed_summary("object", 0), "{ 0 keys }");
        assert_eq!(collapsed_summary("array", 0), "[ 0 items ]");
    }
}

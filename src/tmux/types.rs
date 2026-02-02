//! Core types for tmux integration
//!
//! These types map to tmux's internal structures and are used to
//! synchronize state between tmux and par-term.

/// tmux window ID (e.g., @0, @1)
pub type TmuxWindowId = u64;

/// tmux pane ID (e.g., %0, %1)
pub type TmuxPaneId = u64;

/// Information about a tmux session
#[derive(Debug, Clone)]
pub struct TmuxSessionInfo {
    /// Session name
    pub name: String,
    /// Session ID (e.g., $0)
    pub id: u64,
    /// Whether this session is attached
    pub attached: bool,
    /// Number of windows in the session
    pub window_count: usize,
    /// Creation timestamp
    pub created: std::time::SystemTime,
    /// Last activity timestamp
    pub activity: std::time::SystemTime,
}

/// A tmux window (corresponds to a par-term tab)
#[derive(Debug, Clone)]
pub struct TmuxWindow {
    /// Window ID (e.g., @0)
    pub id: TmuxWindowId,
    /// Window name/title
    pub name: String,
    /// Window index (1-based in tmux)
    pub index: usize,
    /// Whether this is the active window
    pub active: bool,
    /// Layout string (e.g., "89x24,0,0{44x24,0,0,1,44x24,45,0,2}")
    pub layout: String,
    /// Panes in this window
    pub panes: Vec<TmuxPane>,
}

impl TmuxWindow {
    /// Create a new window
    pub fn new(id: TmuxWindowId, name: String, index: usize) -> Self {
        Self {
            id,
            name,
            index,
            active: false,
            layout: String::new(),
            panes: Vec::new(),
        }
    }
}

/// A tmux pane (corresponds to a split pane in par-term)
#[derive(Debug, Clone)]
pub struct TmuxPane {
    /// Pane ID (e.g., %0)
    pub id: TmuxPaneId,
    /// Whether this pane is active in its window
    pub active: bool,
    /// Pane width in characters
    pub width: usize,
    /// Pane height in characters
    pub height: usize,
    /// Pane X position in characters
    pub x: usize,
    /// Pane Y position in characters
    pub y: usize,
    /// Current command running in the pane
    pub current_command: String,
    /// Pane title (from OSC sequences)
    pub title: String,
}

impl TmuxPane {
    /// Create a new pane
    pub fn new(id: TmuxPaneId) -> Self {
        Self {
            id,
            active: false,
            width: 80,
            height: 24,
            x: 0,
            y: 0,
            current_command: String::new(),
            title: String::new(),
        }
    }

    /// Set pane geometry
    pub fn set_geometry(&mut self, x: usize, y: usize, width: usize, height: usize) {
        self.x = x;
        self.y = y;
        self.width = width;
        self.height = height;
    }
}

/// Parsed tmux layout information
#[derive(Debug, Clone)]
pub struct TmuxLayout {
    /// Root layout node
    pub root: LayoutNode,
}

/// A node in the tmux layout tree
#[derive(Debug, Clone)]
pub enum LayoutNode {
    /// A leaf pane
    Pane {
        /// Pane ID
        id: TmuxPaneId,
        /// Width in characters
        width: usize,
        /// Height in characters
        height: usize,
        /// X position
        x: usize,
        /// Y position
        y: usize,
    },
    /// Horizontal split (panes stacked vertically)
    HorizontalSplit {
        /// Total width
        width: usize,
        /// Total height
        height: usize,
        /// X position
        x: usize,
        /// Y position
        y: usize,
        /// Child nodes
        children: Vec<LayoutNode>,
    },
    /// Vertical split (panes side by side)
    VerticalSplit {
        /// Total width
        width: usize,
        /// Total height
        height: usize,
        /// X position
        x: usize,
        /// Y position
        y: usize,
        /// Child nodes
        children: Vec<LayoutNode>,
    },
}

impl TmuxLayout {
    /// Parse a tmux layout string
    ///
    /// Format example: "89x24,0,0{44x24,0,0,1,44x24,45,0,2}"
    /// - `89x24,0,0` = dimensions and position
    /// - `{...}` = vertical split (panes side by side)
    /// - `[...]` = horizontal split (panes stacked)
    /// - Number at end = pane ID
    ///
    /// The layout string may be prefixed with a checksum like "f865," which we skip.
    pub fn parse(layout_str: &str) -> Option<Self> {
        let layout_str = layout_str.trim();

        // Skip the checksum prefix if present (format: "xxxx,..." where xxxx is hex)
        let layout_str = if let Some(comma_idx) = layout_str.find(',') {
            // Check if prefix looks like a checksum (4 hex chars before first comma)
            if comma_idx == 4 && layout_str[..4].chars().all(|c| c.is_ascii_hexdigit()) {
                &layout_str[5..] // Skip "xxxx,"
            } else {
                layout_str
            }
        } else {
            layout_str
        };

        if layout_str.is_empty() {
            return None;
        }

        let (node, _) = Self::parse_node(layout_str)?;
        Some(Self { root: node })
    }

    /// Parse a single node from the layout string
    /// Returns the parsed node and the remaining unparsed string
    fn parse_node(s: &str) -> Option<(LayoutNode, &str)> {
        // Parse dimensions: WIDTHxHEIGHT,X,Y
        let (width, s) = Self::parse_number(s)?;
        let s = s.strip_prefix('x')?;
        let (height, s) = Self::parse_number(s)?;
        let s = s.strip_prefix(',')?;
        let (x, s) = Self::parse_number(s)?;
        let s = s.strip_prefix(',')?;
        let (y, s) = Self::parse_number(s)?;

        // Check what follows: either a split or a pane ID
        if let Some(rest) = s.strip_prefix('{') {
            // Vertical split (panes side by side)
            let (children, rest) = Self::parse_children(rest, '}')?;
            Some((
                LayoutNode::VerticalSplit {
                    width,
                    height,
                    x,
                    y,
                    children,
                },
                rest,
            ))
        } else if let Some(rest) = s.strip_prefix('[') {
            // Horizontal split (panes stacked)
            let (children, rest) = Self::parse_children(rest, ']')?;
            Some((
                LayoutNode::HorizontalSplit {
                    width,
                    height,
                    x,
                    y,
                    children,
                },
                rest,
            ))
        } else if let Some(rest) = s.strip_prefix(',') {
            // Pane ID follows
            let (id, rest) = Self::parse_number(rest)?;
            Some((
                LayoutNode::Pane {
                    id: id as TmuxPaneId,
                    width,
                    height,
                    x,
                    y,
                },
                rest,
            ))
        } else if s.is_empty() || s.starts_with(',') || s.starts_with('}') || s.starts_with(']') {
            // End of string or parent container - this shouldn't happen for a valid pane
            // but handle gracefully
            None
        } else {
            None
        }
    }

    /// Parse children of a split container
    fn parse_children(s: &str, end_char: char) -> Option<(Vec<LayoutNode>, &str)> {
        let mut children = Vec::new();
        let mut remaining = s;

        loop {
            // Parse a child node
            let (child, rest) = Self::parse_node(remaining)?;
            children.push(child);
            remaining = rest;

            // Check what follows
            if remaining.starts_with(end_char) {
                // End of this container
                return Some((children, &remaining[1..]));
            } else if remaining.starts_with(',') {
                // More children follow
                remaining = &remaining[1..];
            } else {
                // Unexpected character
                return None;
            }
        }
    }

    /// Parse a number from the start of the string
    fn parse_number(s: &str) -> Option<(usize, &str)> {
        let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        if end == 0 {
            return None;
        }
        let num = s[..end].parse().ok()?;
        Some((num, &s[end..]))
    }

    /// Get all pane IDs in the layout
    pub fn pane_ids(&self) -> Vec<TmuxPaneId> {
        let mut ids = Vec::new();
        Self::collect_pane_ids(&self.root, &mut ids);
        ids
    }

    fn collect_pane_ids(node: &LayoutNode, ids: &mut Vec<TmuxPaneId>) {
        match node {
            LayoutNode::Pane { id, .. } => {
                ids.push(*id);
            }
            LayoutNode::HorizontalSplit { children, .. }
            | LayoutNode::VerticalSplit { children, .. } => {
                for child in children {
                    Self::collect_pane_ids(child, ids);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_pane() {
        let layout = TmuxLayout::parse("89x24,0,0,1").unwrap();
        match layout.root {
            LayoutNode::Pane {
                id,
                width,
                height,
                x,
                y,
            } => {
                assert_eq!(id, 1);
                assert_eq!(width, 89);
                assert_eq!(height, 24);
                assert_eq!(x, 0);
                assert_eq!(y, 0);
            }
            _ => panic!("Expected single pane"),
        }
    }

    #[test]
    fn test_parse_vertical_split() {
        // Two panes side by side
        let layout = TmuxLayout::parse("89x24,0,0{44x24,0,0,1,44x24,45,0,2}").unwrap();
        match layout.root {
            LayoutNode::VerticalSplit {
                width,
                height,
                children,
                ..
            } => {
                assert_eq!(width, 89);
                assert_eq!(height, 24);
                assert_eq!(children.len(), 2);

                // Check first child
                match &children[0] {
                    LayoutNode::Pane { id, width, .. } => {
                        assert_eq!(*id, 1);
                        assert_eq!(*width, 44);
                    }
                    _ => panic!("Expected pane"),
                }

                // Check second child
                match &children[1] {
                    LayoutNode::Pane { id, x, .. } => {
                        assert_eq!(*id, 2);
                        assert_eq!(*x, 45);
                    }
                    _ => panic!("Expected pane"),
                }
            }
            _ => panic!("Expected vertical split"),
        }
    }

    #[test]
    fn test_parse_horizontal_split() {
        // Two panes stacked
        let layout = TmuxLayout::parse("89x24,0,0[89x12,0,0,1,89x11,0,13,2]").unwrap();
        match layout.root {
            LayoutNode::HorizontalSplit {
                width,
                height,
                children,
                ..
            } => {
                assert_eq!(width, 89);
                assert_eq!(height, 24);
                assert_eq!(children.len(), 2);
            }
            _ => panic!("Expected horizontal split"),
        }
    }

    #[test]
    fn test_parse_with_checksum() {
        // tmux often sends a checksum prefix like "f865,"
        let layout = TmuxLayout::parse("f865,89x24,0,0,1").unwrap();
        match layout.root {
            LayoutNode::Pane { id, .. } => {
                assert_eq!(id, 1);
            }
            _ => panic!("Expected pane"),
        }
    }

    #[test]
    fn test_pane_ids() {
        let layout = TmuxLayout::parse("89x24,0,0{44x24,0,0,1,44x24,45,0,2}").unwrap();
        let ids = layout.pane_ids();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn test_nested_splits() {
        // Vertical split with horizontal split inside
        let layout =
            TmuxLayout::parse("89x24,0,0{44x24,0,0[44x12,0,0,1,44x11,0,13,2],44x24,45,0,3}")
                .unwrap();
        match &layout.root {
            LayoutNode::VerticalSplit { children, .. } => {
                assert_eq!(children.len(), 2);

                // First child should be horizontal split
                match &children[0] {
                    LayoutNode::HorizontalSplit { children, .. } => {
                        assert_eq!(children.len(), 2);
                    }
                    _ => panic!("Expected horizontal split"),
                }

                // Second child should be a pane
                match &children[1] {
                    LayoutNode::Pane { id, .. } => {
                        assert_eq!(*id, 3);
                    }
                    _ => panic!("Expected pane"),
                }
            }
            _ => panic!("Expected vertical split"),
        }

        let ids = layout.pane_ids();
        assert_eq!(ids, vec![1, 2, 3]);
    }
}

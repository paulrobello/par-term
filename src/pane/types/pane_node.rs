//! `PaneNode` — binary tree structure for arbitrary pane nesting.

use super::bounds::PaneBounds;
use super::common::{DividerRect, NavigationDirection, PaneId, SplitDirection};
use super::pane::Pane;

/// Tree node for pane layout
///
/// The pane tree is a binary tree where:
/// - Leaf nodes contain actual terminal panes
/// - Split nodes contain two children with a split direction and ratio
pub enum PaneNode {
    /// A leaf node containing a terminal pane
    Leaf(Box<Pane>),
    /// A split containing two child nodes
    Split {
        /// Direction of the split
        direction: SplitDirection,
        /// Split ratio (0.0 to 1.0) - position of divider
        /// For horizontal: ratio is height of first child / total height
        /// For vertical: ratio is width of first child / total width
        ratio: f32,
        /// First child (top for horizontal, left for vertical)
        first: Box<PaneNode>,
        /// Second child (bottom for horizontal, right for vertical)
        second: Box<PaneNode>,
    },
}

impl PaneNode {
    /// Create a new leaf node with a pane
    pub fn leaf(pane: Pane) -> Self {
        PaneNode::Leaf(Box::new(pane))
    }

    /// Create a new split node
    pub fn split(direction: SplitDirection, ratio: f32, first: PaneNode, second: PaneNode) -> Self {
        PaneNode::Split {
            direction,
            ratio: ratio.clamp(0.1, 0.9), // Enforce minimum pane size
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Check if this is a leaf node
    pub fn is_leaf(&self) -> bool {
        matches!(self, PaneNode::Leaf(_))
    }

    /// Get the pane if this is a leaf node
    pub fn as_pane(&self) -> Option<&Pane> {
        match self {
            PaneNode::Leaf(pane) => Some(pane),
            PaneNode::Split { .. } => None,
        }
    }

    /// Get mutable pane if this is a leaf node
    pub fn as_pane_mut(&mut self) -> Option<&mut Pane> {
        match self {
            PaneNode::Leaf(pane) => Some(pane),
            PaneNode::Split { .. } => None,
        }
    }

    /// Find a pane by ID (recursive)
    pub fn find_pane(&self, id: PaneId) -> Option<&Pane> {
        match self {
            PaneNode::Leaf(pane) => {
                if pane.id == id {
                    Some(pane)
                } else {
                    None
                }
            }
            PaneNode::Split { first, second, .. } => {
                first.find_pane(id).or_else(|| second.find_pane(id))
            }
        }
    }

    /// Find a mutable pane by ID (recursive)
    pub fn find_pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        match self {
            PaneNode::Leaf(pane) => {
                if pane.id == id {
                    Some(pane)
                } else {
                    None
                }
            }
            PaneNode::Split { first, second, .. } => first
                .find_pane_mut(id)
                .or_else(move || second.find_pane_mut(id)),
        }
    }

    /// Find the pane at a given pixel position
    pub fn find_pane_at(&self, x: f32, y: f32) -> Option<&Pane> {
        match self {
            PaneNode::Leaf(pane) => {
                if pane.bounds.contains(x, y) {
                    Some(pane)
                } else {
                    None
                }
            }
            PaneNode::Split { first, second, .. } => first
                .find_pane_at(x, y)
                .or_else(|| second.find_pane_at(x, y)),
        }
    }

    /// Get all pane IDs in this subtree
    pub fn all_pane_ids(&self) -> Vec<PaneId> {
        match self {
            PaneNode::Leaf(pane) => vec![pane.id],
            PaneNode::Split { first, second, .. } => {
                let mut ids = first.all_pane_ids();
                ids.extend(second.all_pane_ids());
                ids
            }
        }
    }

    /// Get all panes in this subtree
    pub fn all_panes(&self) -> Vec<&Pane> {
        match self {
            PaneNode::Leaf(pane) => vec![pane],
            PaneNode::Split { first, second, .. } => {
                let mut panes = first.all_panes();
                panes.extend(second.all_panes());
                panes
            }
        }
    }

    /// Get all mutable panes in this subtree
    pub fn all_panes_mut(&mut self) -> Vec<&mut Pane> {
        match self {
            PaneNode::Leaf(pane) => vec![pane],
            PaneNode::Split { first, second, .. } => {
                let mut panes = first.all_panes_mut();
                panes.extend(second.all_panes_mut());
                panes
            }
        }
    }

    /// Count total number of panes
    pub fn pane_count(&self) -> usize {
        match self {
            PaneNode::Leaf(_) => 1,
            PaneNode::Split { first, second, .. } => first.pane_count() + second.pane_count(),
        }
    }

    /// Calculate bounds for all panes given the total available area
    ///
    /// This recursively distributes space according to split ratios
    /// and updates each pane's bounds field.
    pub fn calculate_bounds(&mut self, bounds: PaneBounds, divider_width: f32) {
        match self {
            PaneNode::Leaf(pane) => {
                pane.bounds = bounds;
            }
            PaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let (first_bounds, second_bounds) = match direction {
                    SplitDirection::Horizontal => {
                        // Split vertically (panes stacked top/bottom)
                        let first_height = (bounds.height - divider_width) * *ratio;
                        let second_height = bounds.height - first_height - divider_width;
                        (
                            PaneBounds::new(bounds.x, bounds.y, bounds.width, first_height),
                            PaneBounds::new(
                                bounds.x,
                                bounds.y + first_height + divider_width,
                                bounds.width,
                                second_height,
                            ),
                        )
                    }
                    SplitDirection::Vertical => {
                        // Split horizontally (panes side by side)
                        let first_width = (bounds.width - divider_width) * *ratio;
                        let second_width = bounds.width - first_width - divider_width;
                        (
                            PaneBounds::new(bounds.x, bounds.y, first_width, bounds.height),
                            PaneBounds::new(
                                bounds.x + first_width + divider_width,
                                bounds.y,
                                second_width,
                                bounds.height,
                            ),
                        )
                    }
                };

                first.calculate_bounds(first_bounds, divider_width);
                second.calculate_bounds(second_bounds, divider_width);
            }
        }
    }

    /// Find the closest pane in a given direction from the focused pane
    ///
    /// Returns the pane ID of the closest pane in the specified direction,
    /// or None if there is no pane in that direction.
    pub fn find_pane_in_direction(
        &self,
        from_id: PaneId,
        direction: NavigationDirection,
    ) -> Option<PaneId> {
        // Get the bounds of the source pane
        let from_pane = self.find_pane(from_id)?;
        let from_center = from_pane.bounds.center();

        // Get all other panes
        let all_panes = self.all_panes();

        // Filter panes that are in the correct direction and find the closest
        let mut best: Option<(PaneId, f32)> = None;

        for pane in all_panes {
            if pane.id == from_id {
                continue;
            }

            let pane_center = pane.bounds.center();
            let is_in_direction = match direction {
                NavigationDirection::Left => pane_center.0 < from_center.0,
                NavigationDirection::Right => pane_center.0 > from_center.0,
                NavigationDirection::Up => pane_center.1 < from_center.1,
                NavigationDirection::Down => pane_center.1 > from_center.1,
            };

            if is_in_direction {
                // Calculate distance (Manhattan distance works well for grid-like layouts)
                let dx = (pane_center.0 - from_center.0).abs();
                let dy = (pane_center.1 - from_center.1).abs();

                // Weight the primary direction more heavily
                let distance = match direction {
                    NavigationDirection::Left | NavigationDirection::Right => dx + dy * 2.0,
                    NavigationDirection::Up | NavigationDirection::Down => dy + dx * 2.0,
                };

                if best.is_none_or(|(_, d)| distance < d) {
                    best = Some((pane.id, distance));
                }
            }
        }

        best.map(|(id, _)| id)
    }

    /// Collect all divider rectangles in the pane tree
    ///
    /// Returns a list of DividerRect structures that can be used for:
    /// - Rendering divider lines between panes
    /// - Hit testing for mouse drag resize
    pub fn collect_dividers(&self, bounds: PaneBounds, divider_width: f32) -> Vec<DividerRect> {
        let mut dividers = Vec::new();
        self.collect_dividers_recursive(bounds, divider_width, &mut dividers);
        dividers
    }

    /// Recursive helper for collecting dividers
    fn collect_dividers_recursive(
        &self,
        bounds: PaneBounds,
        divider_width: f32,
        dividers: &mut Vec<DividerRect>,
    ) {
        match self {
            PaneNode::Leaf(_) => {
                // Leaf nodes have no dividers
            }
            PaneNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                // Calculate divider position and child bounds
                let (first_bounds, divider, second_bounds) = match direction {
                    SplitDirection::Horizontal => {
                        // Horizontal split: panes stacked top/bottom, divider is horizontal line
                        let first_height = (bounds.height - divider_width) * *ratio;
                        let second_height = bounds.height - first_height - divider_width;
                        (
                            PaneBounds::new(bounds.x, bounds.y, bounds.width, first_height),
                            DividerRect::new(
                                bounds.x,
                                bounds.y + first_height,
                                bounds.width,
                                divider_width,
                                true, // is_horizontal
                            ),
                            PaneBounds::new(
                                bounds.x,
                                bounds.y + first_height + divider_width,
                                bounds.width,
                                second_height,
                            ),
                        )
                    }
                    SplitDirection::Vertical => {
                        // Vertical split: panes side by side, divider is vertical line
                        let first_width = (bounds.width - divider_width) * *ratio;
                        let second_width = bounds.width - first_width - divider_width;
                        (
                            PaneBounds::new(bounds.x, bounds.y, first_width, bounds.height),
                            DividerRect::new(
                                bounds.x + first_width,
                                bounds.y,
                                divider_width,
                                bounds.height,
                                false, // is_horizontal (it's vertical)
                            ),
                            PaneBounds::new(
                                bounds.x + first_width + divider_width,
                                bounds.y,
                                second_width,
                                bounds.height,
                            ),
                        )
                    }
                };

                // Add this divider
                dividers.push(divider);

                // Recurse into children
                first.collect_dividers_recursive(first_bounds, divider_width, dividers);
                second.collect_dividers_recursive(second_bounds, divider_width, dividers);
            }
        }
    }
}

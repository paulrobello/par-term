//! Status bar sections (Advanced › Status Bar) and agent usage (Assistant &
//! Agents › Agent Usage). Placement is set in [`crate::layout`].
//!
//! - `general`: Enable/disable, position, height
//! - `styling`: Colors, font size, separator
//! - `auto_hide`: Fullscreen and mouse-inactivity auto-hide
//! - `widget_options`: Time format, git status display
//! - `poll_intervals`: System monitor and git branch poll rates
//! - `widgets`: Three-column widget layout with toggle/reorder/move controls
//! - `agent_usage`: Agent usage panel settings

pub(crate) mod agent_usage;
pub(crate) mod auto_hide;
pub(crate) mod general;
pub(crate) mod poll_intervals;
pub(crate) mod styling;
pub(crate) mod widget_options;
pub(crate) mod widgets;

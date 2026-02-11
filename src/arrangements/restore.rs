//! Monitor-aware restore logic for window arrangements

use super::{MonitorInfo, WindowArrangement};
use std::collections::HashMap;
use winit::monitor::MonitorHandle;

/// Build a mapping from saved monitor indices to available monitor indices.
///
/// Matching priority:
/// 1. Match by monitor name (e.g., "DELL U2720Q")
/// 2. Fall back to matching by index
/// 3. Fall back to primary/first monitor (index 0)
pub fn build_monitor_mapping(
    saved_monitors: &[MonitorInfo],
    available: &[MonitorHandle],
) -> HashMap<usize, usize> {
    let mut mapping = HashMap::new();

    for saved in saved_monitors {
        let matched_index = if let Some(ref saved_name) = saved.name {
            // Try matching by name first
            available
                .iter()
                .position(|m| m.name().as_deref() == Some(saved_name.as_str()))
        } else {
            None
        };

        let matched_index = matched_index.unwrap_or({
            // Fall back to index if available
            if saved.index < available.len() {
                saved.index
            } else {
                // Fall back to primary (index 0)
                0
            }
        });

        mapping.insert(saved.index, matched_index);
    }

    mapping
}

/// Clamp a window position and size to ensure it's visible on the target monitor.
///
/// Returns (clamped_x, clamped_y, clamped_width, clamped_height).
pub fn clamp_to_monitor(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    monitor_pos: (i32, i32),
    monitor_size: (u32, u32),
) -> (i32, i32, u32, u32) {
    // Ensure window isn't larger than the monitor
    let clamped_width = width.min(monitor_size.0);
    let clamped_height = height.min(monitor_size.1);

    // Ensure the window is at least partially visible on the monitor
    // Allow a minimum of 100px visible on each axis
    let min_visible = 100i32;
    let clamped_x = x
        .max(monitor_pos.0 - clamped_width as i32 + min_visible)
        .min(monitor_pos.0 + monitor_size.0 as i32 - min_visible);
    let clamped_y = y
        .max(monitor_pos.1 - clamped_height as i32 + min_visible)
        .min(monitor_pos.1 + monitor_size.1 as i32 - min_visible);

    (clamped_x, clamped_y, clamped_width, clamped_height)
}

/// Compute the absolute position and size for a window snapshot,
/// given the monitor mapping and available monitors.
///
/// Returns (absolute_x, absolute_y, width, height) or None if no monitors available.
pub fn compute_restore_position(
    snapshot: &super::WindowSnapshot,
    monitor_mapping: &HashMap<usize, usize>,
    available: &[MonitorHandle],
) -> Option<(i32, i32, u32, u32)> {
    if available.is_empty() {
        return None;
    }

    // Find the target monitor
    let target_index = monitor_mapping
        .get(&snapshot.monitor.index)
        .copied()
        .unwrap_or(0);
    let target_monitor = available.get(target_index).or(available.first())?;

    let monitor_pos = target_monitor.position();
    let monitor_size = target_monitor.size();

    // Convert relative position to absolute
    let abs_x = monitor_pos.x + snapshot.position_relative.0;
    let abs_y = monitor_pos.y + snapshot.position_relative.1;

    // Clamp to ensure visibility
    let (x, y, w, h) = clamp_to_monitor(
        abs_x,
        abs_y,
        snapshot.size.0,
        snapshot.size.1,
        (monitor_pos.x, monitor_pos.y),
        (monitor_size.width, monitor_size.height),
    );

    Some((x, y, w, h))
}

/// Get the list of tab CWDs from an arrangement for creating tabs
pub fn tab_cwds(arrangement: &WindowArrangement, window_index: usize) -> Vec<Option<String>> {
    arrangement
        .windows
        .get(window_index)
        .map(|ws| ws.tabs.iter().map(|t| t.cwd.clone()).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clamp_to_monitor_within_bounds() {
        let (x, y, w, h) = clamp_to_monitor(100, 100, 800, 600, (0, 0), (1920, 1080));
        assert_eq!((x, y, w, h), (100, 100, 800, 600));
    }

    #[test]
    fn test_clamp_to_monitor_too_large() {
        let (_, _, w, h) = clamp_to_monitor(0, 0, 3000, 2000, (0, 0), (1920, 1080));
        assert_eq!(w, 1920);
        assert_eq!(h, 1080);
    }

    #[test]
    fn test_clamp_to_monitor_offscreen_right() {
        let (x, _, _, _) = clamp_to_monitor(2000, 0, 800, 600, (0, 0), (1920, 1080));
        // Window should be clamped so at least 100px visible
        assert!(x <= 1920 - 100);
    }

    #[test]
    fn test_clamp_to_monitor_offscreen_left() {
        let (x, _, _, _) = clamp_to_monitor(-1000, 0, 800, 600, (0, 0), (1920, 1080));
        // Window should be clamped so at least 100px visible
        assert!(x >= -800 + 100);
    }

    #[test]
    fn test_tab_cwds() {
        use super::super::{MonitorInfo, TabSnapshot, WindowArrangement, WindowSnapshot};
        use uuid::Uuid;

        let arrangement = WindowArrangement {
            id: Uuid::new_v4(),
            name: "Test".to_string(),
            monitor_layout: Vec::new(),
            windows: vec![WindowSnapshot {
                monitor: MonitorInfo {
                    name: None,
                    index: 0,
                    position: (0, 0),
                    size: (1920, 1080),
                },
                position_relative: (0, 0),
                size: (800, 600),
                tabs: vec![
                    TabSnapshot {
                        cwd: Some("/home/user".to_string()),
                        title: "tab1".to_string(),
                    },
                    TabSnapshot {
                        cwd: None,
                        title: "tab2".to_string(),
                    },
                ],
                active_tab_index: 0,
            }],
            created_at: String::new(),
            order: 0,
        };

        let cwds = tab_cwds(&arrangement, 0);
        assert_eq!(cwds.len(), 2);
        assert_eq!(cwds[0], Some("/home/user".to_string()));
        assert_eq!(cwds[1], None);

        // Out of bounds window index
        let cwds = tab_cwds(&arrangement, 5);
        assert!(cwds.is_empty());
    }
}

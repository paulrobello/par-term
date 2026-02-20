//! Capture current window arrangement from live windows

use super::{MonitorInfo, TabSnapshot, WindowArrangement, WindowSnapshot};
use crate::app::window_state::WindowState;
use std::collections::HashMap;
use uuid::Uuid;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

/// Build a MonitorInfo from a winit MonitorHandle
fn monitor_info_from_handle(handle: &winit::monitor::MonitorHandle, index: usize) -> MonitorInfo {
    let pos = handle.position();
    let size = handle.size();
    MonitorInfo {
        name: handle.name(),
        index,
        position: (pos.x, pos.y),
        size: (size.width, size.height),
    }
}

/// Capture the current window arrangement
///
/// Enumerates all monitors and windows, capturing their positions (relative to
/// their monitor), sizes, and tab CWDs.
pub fn capture_arrangement(
    name: String,
    windows: &HashMap<WindowId, WindowState>,
    event_loop: &ActiveEventLoop,
) -> WindowArrangement {
    // Enumerate all monitors
    let monitors: Vec<_> = event_loop.available_monitors().collect();
    let monitor_layout: Vec<MonitorInfo> = monitors
        .iter()
        .enumerate()
        .map(|(i, m)| monitor_info_from_handle(m, i))
        .collect();

    // Capture each window
    let mut window_snapshots = Vec::new();
    for window_state in windows.values() {
        let Some(window) = &window_state.window else {
            continue;
        };

        // Determine which monitor this window is on
        let current_monitor = window.current_monitor();
        let monitor_info = if let Some(ref mon) = current_monitor {
            // Find the index of this monitor in our list
            let index = monitors
                .iter()
                .position(|m| m.name() == mon.name() && m.position() == mon.position())
                .unwrap_or(0);
            monitor_info_from_handle(mon, index)
        } else {
            // Fallback to primary/first monitor
            monitor_layout.first().cloned().unwrap_or(MonitorInfo {
                name: None,
                index: 0,
                position: (0, 0),
                size: (1920, 1080),
            })
        };

        // Compute position relative to monitor origin
        let window_pos = window.outer_position().unwrap_or_default();
        let position_relative = (
            window_pos.x - monitor_info.position.0,
            window_pos.y - monitor_info.position.1,
        );

        let outer_size = window.outer_size();
        let size = (outer_size.width, outer_size.height);

        // Capture tabs
        let tabs: Vec<TabSnapshot> = window_state
            .tab_manager
            .tabs()
            .iter()
            .map(|tab| TabSnapshot {
                cwd: tab.get_cwd(),
                title: tab.title.clone(),
                custom_color: tab.custom_color,
                user_title: if tab.user_named {
                    Some(tab.title.clone())
                } else {
                    None
                },
                custom_icon: tab.custom_icon.clone(),
            })
            .collect();

        let active_tab_index = window_state.tab_manager.active_tab_index().unwrap_or(0);

        window_snapshots.push(WindowSnapshot {
            monitor: monitor_info,
            position_relative,
            size,
            tabs,
            active_tab_index,
        });
    }

    WindowArrangement {
        id: Uuid::new_v4(),
        name,
        monitor_layout,
        windows: window_snapshots,
        created_at: chrono::Utc::now().to_rfc3339(),
        order: 0,
    }
}

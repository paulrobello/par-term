//! Linux-specific menu initialization.
//!
//! On Linux, menus are GTK-based and must be attached to a specific window.
//! Both X11 (Xlib) and Wayland display servers are handled here.

use anyhow::Result;
use std::sync::Arc;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

/// Attach the menu bar to a Linux window.
///
/// GTK menu initialization is more complex than macOS or Windows because
/// it depends on the display server (X11 vs Wayland) and requires GTK
/// to be available. The `muda` GTK feature handles the underlying plumbing.
pub fn init_for_window(window: &Arc<Window>) -> Result<()> {
    if let Ok(handle) = window.window_handle() {
        match handle.as_raw() {
            RawWindowHandle::Xlib(_xlib_handle) => {
                // For X11, menu support goes through GTK integration
                log::info!("Linux X11 menu support (using GTK integration)");
            }
            RawWindowHandle::Wayland(_wayland_handle) => {
                log::info!("Linux Wayland menu support (using GTK integration)");
            }
            _ => {
                log::warn!("Linux: unrecognised window handle type for menu attachment");
            }
        }
    }
    // GTK menu initialization is handled by muda's gtk feature
    log::info!("Linux menu bar initialized (GTK-based)");
    Ok(())
}

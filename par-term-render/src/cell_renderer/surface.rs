use super::CellRenderer;
use crate::wgpu_conversions::VsyncModeWgpu;
use std::sync::atomic::{AtomicU64, Ordering};

/// Clamp a surface extent so `Surface::configure` cannot reject it.
///
/// `configure` is a validation boundary: an extent beyond the device's
/// `max_texture_dimension_2d` is an error, and an uncaptured wgpu error
/// aborts the process (see [`install_nonfatal_error_handler`]). A window
/// spanning multiple high-DPI displays can exceed the 8192 default — the
/// crash of 2026-08-21 was a full-screen tile at 10240×2822 against an 8192
/// limit. Clamping trades a slightly soft, compositor-upscaled frame on
/// adapters that genuinely cap at 8192 for not losing every tab.
pub fn clamp_surface_extent(width: u32, height: u32, max_dimension: u32) -> (u32, u32) {
    (
        width.min(max_dimension).max(1),
        height.min(max_dimension).max(1),
    )
}

/// Device limits with `max_texture_dimension_2d` raised to the adapter's real
/// maximum instead of the 8192 of [`wgpu::Limits::default`].
pub fn texture_limits(max_texture_dimension_2d: u32) -> wgpu::Limits {
    wgpu::Limits {
        max_texture_dimension_2d,
        ..wgpu::Limits::default()
    }
}

/// Install a non-fatal handler for uncaptured wgpu errors on `device`.
///
/// wgpu's default turns any error not caught by an error scope into a panic.
/// par-term's wgpu calls happen inside AppKit/SkyLight callbacks — resize and
/// redraw arrive as Objective-C notifications — where a Rust panic cannot
/// unwind and aborts the process. Logging instead keeps the app alive with a
/// degraded frame.
pub fn install_nonfatal_error_handler(device: &wgpu::Device) {
    let seen = AtomicU64::new(0);
    device.on_uncaptured_error(std::sync::Arc::new(move |err: wgpu::Error| {
        let count = seen.fetch_add(1, Ordering::Relaxed) + 1;
        if should_log_uncaptured_error(count) {
            log::error!("wgpu uncaptured error #{count}: {err}");
        }
    }));
}

/// Whether occurrence number `count` (1-based) reaches the log: the first,
/// then every 1000th, so a per-frame error cannot grow the log unbounded.
fn should_log_uncaptured_error(count: u64) -> bool {
    count == 1 || count.is_multiple_of(1_000)
}

impl CellRenderer {
    pub fn reconfigure_surface(&mut self) {
        self.surface.configure(&self.device, &self.config);
    }

    /// Get the list of supported present modes for this surface
    pub fn supported_present_modes(&self) -> &[wgpu::PresentMode] {
        &self.supported_present_modes
    }

    /// Check if a vsync mode is supported
    pub fn is_vsync_mode_supported(&self, mode: par_term_config::VsyncMode) -> bool {
        self.supported_present_modes
            .contains(&mode.to_present_mode())
    }

    /// Update the vsync mode. Returns the actual mode applied (may differ if requested mode unsupported).
    /// Also returns whether the mode was changed.
    pub fn update_vsync_mode(
        &mut self,
        mode: par_term_config::VsyncMode,
    ) -> (par_term_config::VsyncMode, bool) {
        let requested = mode.to_present_mode();
        let current = self.config.present_mode;

        // Determine the actual mode to use
        let actual = if self.supported_present_modes.contains(&requested) {
            requested
        } else {
            log::warn!(
                "Requested present mode {:?} not supported, falling back to Fifo",
                requested
            );
            wgpu::PresentMode::Fifo
        };

        // Only reconfigure if the mode actually changed
        if actual != current {
            self.config.present_mode = actual;
            self.surface.configure(&self.device, &self.config);
            log::info!("VSync mode changed to {:?}", actual);
        }

        // Convert back to VsyncMode for return
        let actual_vsync = match actual {
            wgpu::PresentMode::Immediate => par_term_config::VsyncMode::Immediate,
            wgpu::PresentMode::Mailbox => par_term_config::VsyncMode::Mailbox,
            wgpu::PresentMode::Fifo | wgpu::PresentMode::FifoRelaxed => {
                par_term_config::VsyncMode::Fifo
            }
            _ => par_term_config::VsyncMode::Fifo,
        };

        (actual_vsync, actual != current)
    }

    /// Get the current vsync mode
    pub fn current_vsync_mode(&self) -> par_term_config::VsyncMode {
        match self.config.present_mode {
            wgpu::PresentMode::Immediate => par_term_config::VsyncMode::Immediate,
            wgpu::PresentMode::Mailbox => par_term_config::VsyncMode::Mailbox,
            wgpu::PresentMode::Fifo | wgpu::PresentMode::FifoRelaxed => {
                par_term_config::VsyncMode::Fifo
            }
            _ => par_term_config::VsyncMode::Fifo,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The crash of 2026-08-21: a full-screen tile spanning two 5K displays,
    /// 10240×2822, configured against a device capped at 8192.
    #[test]
    fn oversize_extent_is_clamped_to_the_device_limit() {
        assert_eq!(clamp_surface_extent(10240, 2822, 8192), (8192, 2822));
    }

    #[test]
    fn extent_within_the_limit_is_unchanged() {
        assert_eq!(clamp_surface_extent(3840, 2160, 8192), (3840, 2160));
    }

    #[test]
    fn extent_at_the_limit_is_unchanged() {
        assert_eq!(clamp_surface_extent(8192, 8192, 8192), (8192, 8192));
    }

    /// `configure` rejects a zero extent as well, so the clamp floors at one
    /// and a call site cannot trade one validation failure for another.
    #[test]
    fn zero_extent_floors_to_one() {
        assert_eq!(clamp_surface_extent(0, 0, 8192), (1, 1));
    }

    #[test]
    fn texture_limits_raises_only_the_2d_dimension() {
        assert_eq!(
            texture_limits(16384),
            wgpu::Limits {
                max_texture_dimension_2d: 16384,
                ..wgpu::Limits::default()
            }
        );
    }

    #[test]
    fn uncaptured_errors_log_the_first_then_every_1000th() {
        assert!(should_log_uncaptured_error(1));
        assert!(!should_log_uncaptured_error(2));
        assert!(!should_log_uncaptured_error(999));
        assert!(should_log_uncaptured_error(1000));
        assert!(!should_log_uncaptured_error(1001));
    }
}

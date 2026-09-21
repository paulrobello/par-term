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

/// Whether `mode` throttles presents to the display refresh (the Fifo family).
fn is_vsync_throttled(mode: wgpu::PresentMode) -> bool {
    matches!(
        mode,
        wgpu::PresentMode::Fifo | wgpu::PresentMode::FifoRelaxed
    )
}

/// Pick an alternate supported present mode to configure through before
/// settling back on `target`.
///
/// A display-topology change can leave the CAMetalLayer in a brightness-strobe
/// state that a same-config `Surface::configure` does not heal, while a vsync
/// toggle — a configure with a *different* present mode — heals in either
/// direction (measured 2026-09-21). Prefers a mode from the other vsync family
/// (the drawable-count difference perturbs the layer harder), then any other
/// supported mode. `None` when the surface supports only `target` and there is
/// nothing to cycle through.
pub fn alternate_present_mode(
    target: wgpu::PresentMode,
    supported: &[wgpu::PresentMode],
) -> Option<wgpu::PresentMode> {
    let preferred: &[wgpu::PresentMode] = if is_vsync_throttled(target) {
        &[wgpu::PresentMode::Immediate, wgpu::PresentMode::Mailbox]
    } else {
        &[wgpu::PresentMode::Fifo, wgpu::PresentMode::FifoRelaxed]
    };
    preferred
        .iter()
        .copied()
        .find(|m| *m != target && supported.contains(m))
        .or_else(|| supported.iter().copied().find(|m| *m != target))
}

/// Refresh a surface configuration against capabilities re-queried after a
/// display-topology change.
///
/// The stored configuration was negotiated against the OLD topology. Every
/// field the fresh capabilities still support is kept as-is — a gratuitous
/// format change would invalidate the render pipelines — and only what the new
/// topology dropped is re-picked, mirroring the selection order of
/// `CellRenderer::new` (first non-sRGB format; Fifo; PreMultiplied >
/// PostMultiplied > Auto). The extent is re-clamped from the live window size.
pub fn refresh_config_after_display_change(
    config: &mut wgpu::SurfaceConfiguration,
    formats: &[wgpu::TextureFormat],
    present_modes: &[wgpu::PresentMode],
    alpha_modes: &[wgpu::CompositeAlphaMode],
    width: u32,
    height: u32,
    max_dimension: u32,
) {
    if !formats.contains(&config.format) {
        let fallback = formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or_else(|| formats.first().copied());
        if let Some(fallback) = fallback {
            log::warn!(
                "Surface format {:?} no longer supported after display change; switching to {:?} (render pipelines were built for the old format)",
                config.format,
                fallback
            );
            config.format = fallback;
        }
    }

    if !present_modes.contains(&config.present_mode) {
        let fallback = if present_modes.contains(&wgpu::PresentMode::Fifo) {
            Some(wgpu::PresentMode::Fifo)
        } else {
            present_modes.first().copied()
        };
        if let Some(fallback) = fallback {
            log::warn!(
                "Present mode {:?} no longer supported after display change; falling back to {:?}",
                config.present_mode,
                fallback
            );
            config.present_mode = fallback;
        }
    }

    if !alpha_modes.contains(&config.alpha_mode) {
        let fallback = [
            wgpu::CompositeAlphaMode::PreMultiplied,
            wgpu::CompositeAlphaMode::PostMultiplied,
            wgpu::CompositeAlphaMode::Auto,
        ]
        .into_iter()
        .find(|m| alpha_modes.contains(m))
        .or_else(|| alpha_modes.first().copied());
        if let Some(fallback) = fallback {
            log::warn!(
                "Alpha mode {:?} no longer supported after display change; falling back to {:?}",
                config.alpha_mode,
                fallback
            );
            config.alpha_mode = fallback;
        }
    }

    let (w, h) = clamp_surface_extent(width, height, max_dimension);
    config.width = w;
    config.height = h;
}

impl CellRenderer {
    pub fn reconfigure_surface(&mut self) {
        self.surface.configure(&self.device, &self.config);
    }

    /// Reconfigure the surface after a display-topology change (monitor
    /// attach/detach, resolution or color-space change), healing the
    /// brightness-strobe state such a change can leave behind.
    ///
    /// A plain [`CellRenderer::reconfigure_surface`] reuses the stored config
    /// and does not heal that state (measured 2026-09-21: a vsync toggle heals
    /// in either direction, a same-config configure does not). This variant
    /// therefore
    ///
    /// 1. re-queries surface capabilities — the stored ones were negotiated
    ///    against the old topology — and refreshes `config` and
    ///    `supported_present_modes`, and
    /// 2. cycles the present mode through an alternate supported mode and
    ///    back, reproducing the toggle's drawable-pool rebuild without
    ///    changing the user's vsync setting.
    pub fn reconfigure_after_display_change(&mut self, width: u32, height: u32) {
        let caps = self.surface.get_capabilities(&self.adapter);
        let mut refreshed = self.config.clone();
        refresh_config_after_display_change(
            &mut refreshed,
            &caps.formats,
            &caps.present_modes,
            &caps.alpha_modes,
            width,
            height,
            self.device.limits().max_texture_dimension_2d,
        );
        self.supported_present_modes = caps.present_modes.clone();

        let target = refreshed.present_mode;
        match alternate_present_mode(target, &self.supported_present_modes) {
            Some(alternate) => {
                log::info!(
                    "Display-change heal: cycling present mode {:?} -> {:?}, config {} by fresh capabilities",
                    target,
                    alternate,
                    if refreshed == self.config {
                        "unchanged"
                    } else {
                        "changed"
                    }
                );
                self.config = refreshed;
                self.config.present_mode = alternate;
                self.surface.configure(&self.device, &self.config);
                self.config.present_mode = target;
                self.surface.configure(&self.device, &self.config);
            }
            None if refreshed == self.config => {
                // Nothing to transition to: a same-config configure heals
                // nothing, and wgpu 30 treats configure as an expensive
                // state transition, not idempotent maintenance.
                log::warn!(
                    "Display-change heal: no alternate present mode and fresh capabilities unchanged; skipping configure"
                );
            }
            None => {
                log::info!(
                    "Display-change heal: no alternate present mode; applying refreshed config"
                );
                self.config = refreshed;
                self.surface.configure(&self.device, &self.config);
            }
        }
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

    fn surface_config(present_mode: wgpu::PresentMode) -> wgpu::SurfaceConfiguration {
        wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: 2560,
            height: 1440,
            present_mode,
            alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        }
    }

    #[test]
    fn alternate_mode_prefers_the_other_vsync_family() {
        // The two halves of the measured heal: toggling vsync either way
        // recovers, so the cycle must be able to leave the target's family.
        assert_eq!(
            alternate_present_mode(
                wgpu::PresentMode::Fifo,
                &[wgpu::PresentMode::Fifo, wgpu::PresentMode::Immediate],
            ),
            Some(wgpu::PresentMode::Immediate)
        );
        assert_eq!(
            alternate_present_mode(
                wgpu::PresentMode::Immediate,
                &[wgpu::PresentMode::Fifo, wgpu::PresentMode::Immediate],
            ),
            Some(wgpu::PresentMode::Fifo)
        );
    }

    #[test]
    fn alternate_mode_falls_back_to_any_other_supported_mode() {
        // Metal offers no Mailbox; FifoRelaxed is the only other option.
        assert_eq!(
            alternate_present_mode(
                wgpu::PresentMode::Fifo,
                &[wgpu::PresentMode::Fifo, wgpu::PresentMode::FifoRelaxed],
            ),
            Some(wgpu::PresentMode::FifoRelaxed)
        );
    }

    #[test]
    fn alternate_mode_is_none_when_target_is_the_only_mode() {
        assert_eq!(
            alternate_present_mode(wgpu::PresentMode::Fifo, &[wgpu::PresentMode::Fifo]),
            None
        );
    }

    #[test]
    fn refresh_keeps_fields_the_new_topology_still_supports() {
        let mut config = surface_config(wgpu::PresentMode::Fifo);
        refresh_config_after_display_change(
            &mut config,
            &[
                wgpu::TextureFormat::Bgra8Unorm,
                wgpu::TextureFormat::Rgba8UnormSrgb,
            ],
            &[wgpu::PresentMode::Fifo, wgpu::PresentMode::Immediate],
            &[
                wgpu::CompositeAlphaMode::PreMultiplied,
                wgpu::CompositeAlphaMode::Auto,
            ],
            1920,
            1080,
            8192,
        );
        assert_eq!(config.format, wgpu::TextureFormat::Bgra8Unorm);
        assert_eq!(config.present_mode, wgpu::PresentMode::Fifo);
        assert_eq!(config.alpha_mode, wgpu::CompositeAlphaMode::PreMultiplied);
        assert_eq!((config.width, config.height), (1920, 1080));
    }

    #[test]
    fn refresh_replaces_what_the_new_topology_dropped() {
        let mut config = surface_config(wgpu::PresentMode::Mailbox);
        refresh_config_after_display_change(
            &mut config,
            &[wgpu::TextureFormat::Rgba8Unorm],
            &[wgpu::PresentMode::Fifo],
            &[wgpu::CompositeAlphaMode::Opaque],
            0,
            0,
            8192,
        );
        // Re-picked by the CellRenderer::new preference order.
        assert_eq!(config.format, wgpu::TextureFormat::Rgba8Unorm);
        assert_eq!(config.present_mode, wgpu::PresentMode::Fifo);
        assert_eq!(config.alpha_mode, wgpu::CompositeAlphaMode::Opaque);
        // The extent still passes through the configure-rejection clamp.
        assert_eq!((config.width, config.height), (1, 1));
    }

    #[test]
    fn refresh_clamps_oversize_extent() {
        let mut config = surface_config(wgpu::PresentMode::Fifo);
        let (format, present_mode, alpha_mode) =
            (config.format, config.present_mode, config.alpha_mode);
        refresh_config_after_display_change(
            &mut config,
            &[format],
            &[present_mode],
            &[alpha_mode],
            10240,
            2822,
            8192,
        );
        assert_eq!((config.width, config.height), (8192, 2822));
    }
}

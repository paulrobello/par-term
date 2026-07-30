//! Frame pacing and GPU adapter settings.
//!
//! Extracted from the top-level [`super::Config`] struct via `#[serde(flatten)]`.
//! All fields serialise at the top level of the YAML config file -- existing
//! config files remain 100% compatible.

use crate::types::{PowerPreference, VsyncMode};
use serde::{Deserialize, Serialize};

/// Frame rate target, VSync mode, GPU preference and output batching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderingConfig {
    /// Maximum frames per second (FPS) target
    /// Controls how frequently the terminal requests screen redraws.
    /// Note: On macOS, actual FPS may be lower (~22-25) due to system-level
    /// VSync throttling in wgpu/Metal, regardless of this setting.
    /// Default: 60
    #[serde(default = "crate::defaults::max_fps", alias = "refresh_rate")]
    pub max_fps: u32,

    /// VSync mode - controls GPU frame synchronization
    /// - immediate: No VSync, render as fast as possible (lowest latency, highest power)
    /// - mailbox: Cap at monitor refresh rate with triple buffering (balanced)
    /// - fifo: Strict VSync with double buffering (lowest power, slight input lag)
    ///
    /// Default: fifo (strict VSync — lowest power, most compatible)
    #[serde(default)]
    pub vsync_mode: VsyncMode,

    /// GPU power preference for adapter selection
    /// - none: Let the system decide (default)
    /// - low_power: Prefer integrated GPU (saves battery)
    /// - high_performance: Prefer discrete GPU (maximum performance)
    ///
    /// Note: Requires app restart to take effect.
    #[serde(default)]
    pub power_preference: PowerPreference,

    /// Reduce flicker by delaying redraws while cursor is hidden (DECTCEM off).
    /// Many terminal programs hide cursor during bulk updates to prevent visual artifacts.
    #[serde(default = "crate::defaults::reduce_flicker")]
    pub reduce_flicker: bool,

    /// Maximum delay in milliseconds when reduce_flicker is enabled.
    /// Rendering occurs when cursor becomes visible OR this delay expires.
    /// Range: 1-100ms. Default: 16ms (~1 frame at 60fps).
    #[serde(default = "crate::defaults::reduce_flicker_delay_ms")]
    pub reduce_flicker_delay_ms: u32,

    /// Enable throughput mode to batch rendering during bulk output.
    /// When enabled, rendering is throttled to reduce CPU overhead for large outputs.
    /// Toggle with Cmd+Shift+T (macOS) or Ctrl+Shift+T (other platforms).
    #[serde(default = "crate::defaults::maximize_throughput")]
    pub maximize_throughput: bool,

    /// Render interval in milliseconds when maximize_throughput is enabled.
    /// Higher values = better throughput but delayed display. Range: 50-500ms.
    #[serde(default = "crate::defaults::throughput_render_interval_ms")]
    pub throughput_render_interval_ms: u32,
}

impl Default for RenderingConfig {
    fn default() -> Self {
        Self {
            max_fps: crate::defaults::max_fps(),
            vsync_mode: VsyncMode::default(),
            power_preference: PowerPreference::default(),
            reduce_flicker: crate::defaults::reduce_flicker(),
            reduce_flicker_delay_ms: crate::defaults::reduce_flicker_delay_ms(),
            maximize_throughput: crate::defaults::maximize_throughput(),
            throughput_render_interval_ms: crate::defaults::throughput_render_interval_ms(),
        }
    }
}

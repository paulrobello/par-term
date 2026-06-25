//! wgpu conversion helpers for `par-term-config` rendering enums.
//!
//! These used to live in `par-term-config` behind the `wgpu-types` feature
//! (see AUDIT.md ARC-003). That was a layering violation: a Layer-1 pure-data
//! crate carried an optional dependency on Layer-3 GPU types. The helpers now
//! live here in `par-term-render` — the Layer-3 crate that actually depends on
//! wgpu — and are exposed as narrow extension traits so existing call sites
//! keep the same method names (`vsync.to_present_mode()`,
//! `pref.to_wgpu()`, `scaling.to_filter_mode()`).
//!
//! Call sites must `use crate::wgpu_conversions::{...};` to bring the methods
//! into scope.

use par_term_config::{ImageScalingMode, PowerPreference, VsyncMode};

/// `VsyncMode` → `wgpu::PresentMode`.
pub trait VsyncModeWgpu {
    fn to_present_mode(self) -> wgpu::PresentMode;
}

/// `PowerPreference` → `wgpu::PowerPreference`.
pub trait PowerPreferenceWgpu {
    fn to_wgpu(self) -> wgpu::PowerPreference;
}

/// `ImageScalingMode` → `wgpu::FilterMode`.
pub trait ImageScalingModeWgpu {
    fn to_filter_mode(self) -> wgpu::FilterMode;
}

impl VsyncModeWgpu for VsyncMode {
    fn to_present_mode(self) -> wgpu::PresentMode {
        match self {
            VsyncMode::Immediate => wgpu::PresentMode::Immediate,
            VsyncMode::Mailbox => wgpu::PresentMode::Mailbox,
            VsyncMode::Fifo => wgpu::PresentMode::Fifo,
        }
    }
}

impl PowerPreferenceWgpu for PowerPreference {
    fn to_wgpu(self) -> wgpu::PowerPreference {
        match self {
            PowerPreference::None => wgpu::PowerPreference::None,
            PowerPreference::LowPower => wgpu::PowerPreference::LowPower,
            PowerPreference::HighPerformance => wgpu::PowerPreference::HighPerformance,
        }
    }
}

impl ImageScalingModeWgpu for ImageScalingMode {
    fn to_filter_mode(self) -> wgpu::FilterMode {
        match self {
            ImageScalingMode::Nearest => wgpu::FilterMode::Nearest,
            ImageScalingMode::Linear => wgpu::FilterMode::Linear,
        }
    }
}

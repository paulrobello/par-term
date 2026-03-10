//! Common GPU utilities for texture and sampler creation.
//!
//! This module provides reusable helper functions for common wgpu operations
//! to reduce code duplication across renderer modules.

use wgpu::{AddressMode, Device, FilterMode, Sampler, SamplerDescriptor};

/// Create a sampler with the specified filter mode and ClampToEdge address mode.
///
/// This allows choosing between nearest-neighbor (sharp/pixel art) and
/// linear (smooth) filtering for texture sampling.
pub fn create_sampler_with_filter(
    device: &Device,
    filter: FilterMode,
    label: Option<&str>,
) -> Sampler {
    device.create_sampler(&SamplerDescriptor {
        label,
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        address_mode_w: AddressMode::ClampToEdge,
        mag_filter: filter,
        min_filter: filter,
        mipmap_filter: FilterMode::Nearest,
        ..Default::default()
    })
}

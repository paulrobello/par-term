//! GPU utility re-exports from par-term-render crate.
//!
//! This module re-exports types from the par-term-render crate for backward compatibility.

pub use par_term_render::gpu_utils::{
    create_linear_sampler, create_render_texture, create_repeat_sampler, create_rgba_texture,
    create_sampler_with_filter, write_rgba_texture, write_rgba_texture_region,
};

//! Renderer re-exports from the `par-term-render` sub-crate.
//!
//! Re-exports types from the `par-term-render` sub-crate for backward compatibility.
//! All GPU rendering implementation is defined in `par-term-render`.
//!
//! # Re-exports from `par-term-render`
//!
//! This module is a thin facade so the rest of the main crate can use
//! `crate::renderer::Renderer` rather than depending directly on
//! `par_term_render`. The actual wgpu rendering pipeline lives in `par-term-render`.

pub use par_term_render::renderer::{
    DividerRenderInfo, PaneDividerSettings, PaneRenderInfo, PaneTitleInfo, Renderer,
    RendererParams, compute_visible_separator_marks, graphics, params, shaders,
};

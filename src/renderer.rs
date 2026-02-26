//! Renderer re-exports from par-term-render crate.
//!
//! This module re-exports types from the par-term-render crate for backward compatibility.

pub use par_term_render::renderer::{
    DividerRenderInfo, PaneDividerSettings, PaneRenderInfo, PaneTitleInfo, Renderer,
    RendererParams, compute_visible_separator_marks, graphics, params, shaders,
};

//! GPU-accelerated rendering engine for par-term terminal emulator.
//!
//! This crate provides the rendering pipeline for the terminal emulator,
//! including:
//!
//! - Cell-based GPU rendering with glyph atlas
//! - Sixel/iTerm2/Kitty inline graphics rendering
//! - Custom GLSL shader post-processing (Shadertoy/Ghostty compatible)
//! - Scrollbar rendering with mark overlays
//! - Background image rendering
//! - GPU utility functions

pub mod cell_renderer;
pub mod custom_shader_renderer;
pub mod gpu_utils;
pub mod graphics_renderer;
pub mod renderer;
pub mod scrollbar;

// Re-export main public types
pub use cell_renderer::{Cell, CellRenderer, PaneViewport};
pub use custom_shader_renderer::CustomShaderRenderer;
pub use graphics_renderer::GraphicsRenderer;
pub use renderer::{
    DividerRenderInfo, PaneDividerSettings, PaneRenderInfo, PaneTitleInfo, Renderer,
    RendererParams, compute_visible_separator_marks,
};
pub use scrollbar::Scrollbar;

// Re-export shared types from dependencies for convenience
pub use par_term_config::{ScrollbackMark, SeparatorMark};

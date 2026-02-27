//! Prettifier pipeline: boundary detection → format detection → rendering.
//!
//! `PrettifierPipeline` wires together a `BoundaryDetector`, a `RendererRegistry`,
//! a `RenderCache`, and block tracking into a single flow. Terminal output lines are
//! fed in, content blocks are emitted at boundaries, detected, rendered, and stored
//! for display. Each `PrettifiedBlock` wraps a `DualViewBuffer` for efficient
//! source/rendered toggling and copy operations.
//!
//! Sub-modules:
//! - [`block`] — `PrettifiedBlock` type
//! - [`config`] — `PrettifierConfig` type
//! - [`pipeline_impl`] — `PrettifierPipeline` struct and all methods

mod block;
mod config;
mod pipeline_impl;

#[cfg(test)]
mod tests;

// Re-export the public API.
pub use block::PrettifiedBlock;
pub use config::PrettifierConfig;
pub use pipeline_impl::PrettifierPipeline;

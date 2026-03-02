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
//! - [`pipeline_impl`] — `PrettifierPipeline` struct, constructors, and core methods
//! - [`pipeline_blocks`] — block lifecycle: detection, deduplication, eviction, and query accessors
//! - [`pipeline_claude_code`] — Claude Code session detection and expand event handling
//! - [`render`] — cache-aware rendering helpers

mod block;
mod config;
mod pipeline_blocks;
mod pipeline_claude_code;
mod pipeline_impl;
mod render;

#[cfg(test)]
mod tests;

// Re-export the public API.
pub use block::PrettifiedBlock;
pub use config::PrettifierConfig;
pub use pipeline_impl::PrettifierPipeline;

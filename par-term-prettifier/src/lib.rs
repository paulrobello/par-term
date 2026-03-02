//! Content Prettifier framework.
//!
//! Detects structured content in terminal output (Markdown, JSON, YAML, diffs, etc.)
//! and renders it in a rich, human-readable form. Built on a pluggable trait-based
//! architecture where `ContentDetector` identifies formats and `ContentRenderer`
//! handles display.
//!
//! # Module Structure
//!
//! The prettifier is organized into three functional layers:
//!
//! ## Detection Layer
//! - [`detectors`] — Content type detector implementations (`JsonDetector`,
//!   `MarkdownDetector`, `DiffDetector`, etc.). Each implements [`traits::ContentDetector`].
//! - [`regex_detector`] — Generic regex-based detector for user-configured patterns.
//! - [`boundary`] — Line boundary tracking for multi-line content detection.
//! - [`buffer`] — Output buffer used during progressive detection.
//!
//! ## Rendering Layer
//! - [`renderers`] — Format-specific renderers (Markdown, JSON, YAML, diffs,
//!   stack traces, diagrams). Each implements [`traits::ContentRenderer`].
//! - [`custom_renderers`] — User-configured external command renderers.
//! - [`gutter`] — Gutter indicator manager: renders left-margin marks that flag
//!   prettified content blocks in the terminal view.
//!
//! ## Pipeline / Registry Layer
//! - [`pipeline`] — `PrettifierPipeline`: the top-level coordinator that holds
//!   all detectors and renderers and drives per-line processing.
//! - [`registry`] — `RendererRegistry`: maps content type identifiers to the
//!   renderer implementations at runtime.
//! - [`cache`] — Rendered output cache to avoid re-rendering unchanged content.
//! - [`config_bridge`] — Translates `par-term-config` prettifier settings into
//!   live `PrettifierPipeline` instances; bridges configuration and runtime.
//! - [`claude_code`] — Specialized detector/renderer for Claude Code XML tool-call
//!   output format.
//!
//! ## Shared Types
//! - [`traits`] — `ContentDetector` and `ContentRenderer` trait definitions.
//! - [`types`] — Shared data types: `ContentBlock`, `DetectionResult`, `RenderOutput`, etc.

// Debug macro shims: map crate::debug_*!(category, ...) to log:: equivalents.
// The #[macro_export] makes them accessible as `crate::debug_info!` etc.
// from all sub-modules, matching the pattern used in the main par-term crate.

/// Log at `error` level (maps to `log::error!`).
#[macro_export]
macro_rules! debug_error {
    ($category:expr, $($arg:tt)*) => { log::error!($($arg)*) };
}

/// Log at `info` level (maps to `log::info!`).
#[macro_export]
macro_rules! debug_info {
    ($category:expr, $($arg:tt)*) => { log::info!($($arg)*) };
}

/// Log at `debug` level (maps to `log::debug!`).
#[macro_export]
macro_rules! debug_log {
    ($category:expr, $($arg:tt)*) => { log::debug!($($arg)*) };
}

/// Log at `trace` level (maps to `log::trace!`).
#[macro_export]
macro_rules! debug_trace {
    ($category:expr, $($arg:tt)*) => { log::trace!($($arg)*) };
}

pub mod ansi_colors;
pub mod ansi_parser;
pub mod boundary;
pub mod buffer;
pub mod cache;
pub mod claude_code;
pub mod claude_code_preview;
pub mod config;
pub mod config_bridge;
pub mod custom_renderers;
pub mod detectors;
pub mod gutter;
pub mod pipeline;
pub mod regex_detector;
pub mod registry;
pub mod renderers;
pub mod rule_loader;
#[cfg(test)]
pub mod testing;
pub mod traits;
pub mod types;

pub use boundary::*;
pub use buffer::*;
pub use cache::*;
pub use claude_code::*;
pub use config_bridge::*;
pub use custom_renderers::*;
pub use gutter::*;
pub use pipeline::*;
pub use regex_detector::*;
pub use registry::*;
pub use traits::*;
pub use types::*;

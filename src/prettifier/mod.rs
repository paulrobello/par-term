//! Content Prettifier framework.
//!
//! Detects structured content in terminal output (Markdown, JSON, YAML, diffs, etc.)
//! and renders it in a rich, human-readable form. Built on a pluggable trait-based
//! architecture where `ContentDetector` identifies formats and `ContentRenderer`
//! handles display.

pub mod boundary;
pub mod buffer;
pub mod cache;
pub mod claude_code;
pub mod config_bridge;
pub mod custom_renderers;
pub mod detectors;
pub mod gutter;
pub mod pipeline;
pub mod regex_detector;
pub mod registry;
pub mod renderers;
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

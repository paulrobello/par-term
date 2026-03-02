//! Stack trace renderer with root error highlighting, frame classification,
//! clickable file paths, and collapsible long traces.
//!
//! Parses stack traces from multiple languages (Java, Python, Rust, Go, JS)
//! and renders with:
//!
//! - **Root error highlighting**: error/exception messages bold red
//! - **Frame classification**: application frames bright, framework frames dimmed
//! - **Clickable file paths**: file:line patterns rendered as links
//! - **Collapsible traces**: long traces folded with "... N more frames"
//!
//! Sub-modules:
//! - [`config`]        — `StackTraceRendererConfig`
//! - [`types`]         — `FrameType`, `FilePath`, `TraceLine` (internal)
//! - [`regex_helpers`] — compiled regex patterns (internal)
//! - [`parse`]         — frame classification and line parsing (internal)
//! - [`renderer`]      — `StackTraceRenderer` struct and registration

mod config;
mod parse;
mod regex_helpers;
mod renderer;
mod types;

#[cfg(test)]
mod tests;

// Re-export the public API.
pub use config::StackTraceRendererConfig;
pub use renderer::{StackTraceRenderer, register_stack_trace_renderer};

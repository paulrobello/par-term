//! Diff renderer with green/red coloring, word-level highlighting, line number
//! gutter, file/hunk header styling, and optional side-by-side mode.
//!
//! Parses unified diff format into structured hunks, then renders with:
//! - **Line-level coloring**: green for additions, red for removals
//! - **File headers** (`---`/`+++`): bold with distinct color
//! - **Hunk headers** (`@@`): cyan/blue with line range info
//! - **Word-level diff**: highlights changed words within paired +/- lines
//! - **Line number gutter**: old/new line numbers from hunk headers
//! - **Side-by-side mode**: when terminal is wide enough
//!
//! Sub-modules:
//! - [`config`] — `DiffStyle`, `DiffRendererConfig`, `DiffLineState`
//! - [`renderer`] — `DiffRenderer` struct and `ContentRenderer` implementation
//! - [`side_by_side`] — side-by-side layout helpers
//! - [`helpers`] — gutter, line number, and truncation utilities

mod config;
mod diff_parser;
mod diff_word;
mod helpers;
mod renderer;
mod side_by_side;

#[cfg(test)]
mod tests;

// Re-export the public API.
pub use config::{DiffRendererConfig, DiffStyle};
pub use renderer::{DiffRenderer, register_diff_renderer};

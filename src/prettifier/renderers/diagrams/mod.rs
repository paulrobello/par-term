//! Diagram renderer for fenced code blocks tagged with diagram language identifiers.
//!
//! Converts fenced code blocks tagged with diagram language identifiers
//! (Mermaid, PlantUML, GraphViz, D2, etc.) into rendered output using one of
//! four backends:
//!
//! - **Native**: pure-Rust mermaid rendering via `mermaid-rs-renderer` (mermaid only,
//!   500–1400× faster than mmdc, zero external dependencies).
//! - **Local CLI**: pipes diagram source to a local tool (e.g., `dot`, `mmdc`),
//!   captures PNG output, and stores it as an `InlineGraphic`.
//! - **Kroki API**: sends diagram source via HTTP POST to a Kroki server,
//!   receives PNG output, and stores it as an `InlineGraphic`.
//! - **Text fallback**: syntax-highlighted source display with a format badge.
//!
//! Backend selection follows the `engine` config: `"auto"` tries native (mermaid
//! only) then local CLI then Kroki, `"native"` uses only the native renderer,
//! `"local"` uses only local CLI, `"kroki"` uses only the API, and
//! `"text_fallback"` skips all backends.
//!
//! # Sub-modules
//!
//! - [`languages`] — supported diagram types and their metadata
//! - [`svg_utils`] — SVG→PNG conversion utilities and Mermaid theme helpers
//! - [`renderer`] — `DiagramRenderer` struct and rendering logic

mod languages;
mod renderer;
mod svg_utils;
mod tests;

// Re-export the public API to preserve backward compatibility.
pub use languages::{DiagramLanguage, default_diagram_languages};
pub use renderer::{DiagramRenderer, register_diagram_renderer};
pub use svg_utils::svg_to_png_bytes;

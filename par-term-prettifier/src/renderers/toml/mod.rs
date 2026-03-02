//! TOML renderer with syntax highlighting, section headers, key-value alignment,
//! and tree guide indentation.
//!
//! Uses a line-by-line classifier to parse TOML content without a full parser.
//! Features include:
//!
//! - **Section headers**: `[section]` rendered prominently with bold styling
//! - **Array table headers**: `[[array]]` styled distinctly from regular sections
//! - **Key-value alignment**: `=` signs aligned within sections for readability
//! - **Type-aware value coloring**: strings, integers, floats, booleans, dates
//! - **Comment dimming**: comments styled as dimmed italic text
//! - **Tree guides**: indentation guides for nested section hierarchy

mod parser;
#[cfg(test)]
mod tests;

pub use parser::{TomlRenderer, TomlRendererConfig, register_toml_renderer};

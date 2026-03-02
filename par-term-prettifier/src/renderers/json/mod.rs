//! JSON renderer with syntax highlighting, tree guides, and collapsible nodes.
//!
//! Parses raw JSON text via `serde_json`, then walks the value tree to produce
//! styled terminal output. Features include:
//!
//! - **Syntax highlighting**: distinct colors for keys, strings, numbers, booleans, null
//! - **Tree guide lines**: vertical `│` characters at each indentation level
//! - **Collapsible nodes**: objects/arrays auto-collapse beyond `max_depth_expanded`
//! - **Value type indicators**: optional `(type)` annotations next to values
//! - **Large array truncation**: arrays beyond a threshold show `... and N more items`
//! - **URL detection**: string values containing URLs rendered as OSC 8 hyperlinks
//! - **Key sorting**: optional alphabetical ordering of object keys

mod parser;
#[cfg(test)]
mod tests;

pub use parser::{JsonRenderer, JsonRendererConfig, register_json_renderer};

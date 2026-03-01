//! YAML renderer with syntax highlighting, indentation guides, and collapsible sections.
//!
//! Uses a line-by-line parser that tracks indentation depth rather than a full
//! YAML parser. Features include:
//!
//! - **Syntax highlighting**: distinct colors for keys, string values, numbers,
//!   booleans, anchors, aliases, tags, and comments
//! - **Tree guide lines**: vertical `│` characters at each indentation level
//! - **Collapsible sections**: mapping keys with nested children auto-collapse
//!   beyond `max_depth_expanded`
//! - **Anchor/alias indicators**: `&anchor` and `*alias` in distinct color
//! - **Document separator styling**: `---` rendered as a prominent separator

mod parser;
#[cfg(test)]
mod tests;

pub use parser::{YamlRenderer, YamlRendererConfig, register_yaml_renderer};

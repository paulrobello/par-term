//! Tests for the Markdown renderer, organized by concern.
//!
//! Sub-modules:
//! - [`blocks`]      — block classification and table helper tests
//! - [`inline`]      — inline element rendering (bold, italic, code, links, etc.)
//! - [`code_blocks`] — fenced code block and syntax highlighting tests
//! - [`tables`]      — table rendering tests
//! - [`integration`] — full renderer round-trip tests

#[path = "tests/blocks.rs"]
mod blocks;
#[path = "tests/code_blocks.rs"]
mod code_blocks;
#[path = "tests/inline.rs"]
mod inline;
#[path = "tests/integration.rs"]
mod integration;
#[path = "tests/tables.rs"]
mod tables;

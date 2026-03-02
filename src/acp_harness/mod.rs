//! Reusable components for the `par-term-acp-harness` test binary.
//!
//! This module is intentionally kept **thin** — it re-exports helpers that live
//! in dedicated sub-modules so the binary's `main.rs` stays focused on the
//! top-level event loop.
//!
//! Sub-modules:
//! - [`agent_discovery`]: Agent discovery and custom-agent merging logic
//! - [`binary_resolver`]: `par-term` binary path resolution
//! - [`harness_output`]: Console/transcript output formatting
//! - [`message_handler`]: ACP agent message dispatch and config update application
//! - [`prompt_builder`]: Prompt block construction and preview

pub mod agent_discovery;
pub mod binary_resolver;
pub mod harness_output;
pub mod message_handler;
pub mod prompt_builder;

//! Shader context generation for AI Inspector agent prompts.
//!
//! When users discuss shaders with the ACP agent, this module detects
//! shader-related keywords and builds a rich context block describing the
//! current shader state, available shaders, debug paths, uniforms, and
//! a minimal template so the agent can assist with shader creation, editing,
//! debugging, and management.

mod context_builder;
pub(super) mod helpers;
#[cfg(test)]
mod tests;

pub use context_builder::build_shader_context;
pub use helpers::{is_shader_activation_request, should_inject_shader_context};

//! GLSL-to-WGSL transpiler for custom shaders.
//!
//! Splits the former monolithic `transpiler.rs` (~1,256 lines) into focused sub-modules:
//!
//! - **`glsl_parse`** -- GLSL source preprocessing, Shadertoy fragCoord handling,
//!   and custom shader control (`// control ...`) uniform extraction/replacement.
//! - **`wgsl_emit`** -- GLSL wrapper template, naga-based transpilation, WGSL
//!   post-processing (builtin injection, fragCoord seeding), and the public entry points.

mod glsl_parse;
mod wgsl_emit;

pub(crate) use wgsl_emit::transpile_glsl_to_wgsl;
pub(crate) use wgsl_emit::transpile_glsl_to_wgsl_source;
